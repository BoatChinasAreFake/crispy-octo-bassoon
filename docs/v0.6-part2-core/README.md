# v0.6 Part 2 core — development branch

Implements roadmap items 4 to 10. The two originals remain undecided, and Part 2 is not marked released. No changes are merged into main and no release is published.

## Behavior and compatibility

- New worlds use generation revision **12** with narrower shorelines, retaining revision 11 for beach finds and sheltered tide pools, on revision 10 terrain. Revision 9 retains the previous terrain refinements. The original core is revision 8, with revision 7 surface shape/climate. Revisions **0–11** retain their terrain, cave, mod-patch and placement paths. The save format remains version 16; generator options still use the existing packed field. Protocol **49** protects revision 12 and the appended beach block IDs.
- The depth-aware cave selector reuses surface temperature, moisture, continentalness and erosion. Deep Dark takes precedence below height 24 under inland rugged land. Damp, mild land has lush caves from height 24; drier, non-frozen inland land has dripstone at heights 10–48. Bedrock and the top eight blocks below the surface are excluded. Existing carving, blocks, geodes and survival mechanics remain.
- Climate-target mods use optional bounded axes, a 0.35 per-axis eligibility radius, normalized mean squared distance and stable-key tie-breaking. Targetless patches retain first-match priority. Built-in shape landmarks and rare variants are protected. Targets are ignored in older worlds and rejected for mod dimensions.
- Surface structures validate their complete bounded footprints, including interiors. Villages check squares, paths, farms and houses; growth sites are filtered too. Mansions remain inside dark forest, and springs require cold, level ground. Overworld mod templates receive supporting foundations, and settling aprons preserve adjacent waterways.
- Existing Stalac-tight, Lush Life and Homebrew advancements remain earnable and are exercised by socket playtests. No unrelated new content or originals are introduced.

## Validation

Validated on Linux x86-64 with **Rust 1.90.0**, locked dependencies and the normal release profile (thin LTO):

- `cargo test --release --locked`: **570 passed, 0 failed, 1 ignored** (the manual map export).
- `cargo clippy --release --all-targets --locked -- -D warnings`: clean.
- `cargo check --release --locked --no-default-features`: passes.
- Audio mixer tests: **2 passed**. This container required the GNU bfd linker for the standalone vendor test binary.
- Six-bot, 60-second CLI socket playtest, seed 1234: everyone agrees (121 blocks placed, 106 broken, 129 chat lines). Includes natural cave generation, village foundations, climate-target mod surfaces, bounded rivers, rocky coasts, islands, beach ecology, tide pools/finds, all nine coastal species, marine mob snapshots, host edit/restore propagation and advancement delivery.
- The test suite also runs a two-bot/seed-77 scenario and a six-bot/seed-1234 scenario with base content, without installing a mod registry concurrently.
- Released-generation regression fixtures were captured from merge commit `3ef73d102c6119e4092c2e59b69e47c8c1d4c556` before implementation: seed 424242, chunks (0,0), (-7,11), (31,-29), for every revision 0–7. Revision 8 was captured from commit `59c38343a05c72044b06a08e49f309bafe8feae2` before the revision 9 edits; all ten fixtures pass.
- Climate parser tests cover finite bounds, omitted axes, old defaults and unsupported dimensions. Integration tests check generated mod-template blocks, foundations, chest locators and a template spanning a chunk boundary.
- Twelve-seed automated surveys check variety, sea/land balance, river/beach height constraints and absence of directly adjacent hot/frozen biomes. Revision 8 surface maps, structure overlays and cave slices were visually reviewed; revision 9 surface exports passed for all twelve seeds at 512×512 chunks, with the same ten comparison seeds rendered and inspected.
- The original seven natural scenes were inspected for revision 8. Six new scenes were rendered and inspected for revision 9: river, rocky coast, beach, mountains, meadow and forest/plains border, using software OpenGL. Audio output was unavailable in this container; audio playback was not auditioned. Windows and macOS builds were not run locally.

The container's newer Rust 1.99 toolchain reports existing lints outside this slice; the clean Clippy result above is specifically for Rust 1.90.0. No broad lint refactor is bundled into the terrain change.

## Review corrections

Four regression tests first failed against the initial implementation and pass after correction:

1. Revision 8 cave decorators now start at height 3 and let the depth selector reserve the Deep Dark. This removes the inherited two-layer decoration gap at heights 24 and 25.
2. Mod-template reach calculations include the four-block settling apron, so the apron is also generated in neighbouring chunks that contain no template blocks. Older revisions retain their original reach calculation.
3. Climate targets with misspelled `in` restrictions are rejected, rather than treating an empty parsed list as an unrestricted target. Targetless mod parsing is unchanged.
4. Worlds using revisions 0–7 retain the released column-based cave advancement rule. Revisions 8+ use depth-aware advancement selection.

## Requested terrain refinements (revision 9)

- Continentalness, erosion and weirdness sampling scales grow 3×; temperature/moisture scales grow 2×. Existing biome-size options remain available, and local surface detail/warp scales stay unchanged.
- River valleys remain broad, but channel width uses the local weirdness gradient and a 5–11-block target half-width. Near-zero gradients cannot open arbitrarily wide channels. The derivative is a local approximation rather than a global distance field.
- Coastal noise adds bays/headlands. Sand is limited to gentle slopes, with a smaller elevation band on steeper beaches. Rocky shores use stone/gravel and reject tree roots without introducing a new biome identifier.
- Mountain shoulders, local saddles, worn upland plateaus and occasional sharper summits vary the expanded ranges. Existing soft height limits remain.
- Cold lowlands and frozen seas use temperature below −0.38; alpine snow uses temperature plus elevation cooling. Warm mountains remain exposed rock rather than universally snowy.
- Ordinary inland ground, vegetation density and tree species share a deterministic border blend extending roughly 24 blocks. Rivers, seas, shores, mountains, rare variants and wetland identities remain protected.
- Common structure slots reserve two chunks around mansion candidates, preventing common buildings/dungeons from intruding into mansion basements and settling aprons. Cave depth rules and existing whole-footprint checks continue to operate on the new terrain.
- New natural screenshot modes and multiplayer river/coast regeneration checks exercise the newest generation. The map exporter accepts chunk dimensions and a surface-only mode.

Compatibility fixtures include a revision 8 chunk hash captured from the unmodified branch before these changes. Revision 3's historical biome-size comparison explicitly uses revision 3; separate newest-generator surveys verify continuity and variety.

In the initial revision 9 survey, before the bank-profile correction, across the ten comparison seeds (512×512 chunks, 8-block sampling), mean adjacent-sample biome changes declined from 15.08% (revision 8) to 6.80%; sampled oceans changed from 29.61% to 32.70%, rivers from 3.25% to 1.29%, and snow/ice-spike land from 10.26% to 7.86%. These bounded surveys do not measure exact worldwide area or establish equivalence to Minecraft. Surface biome maps do not show rocky material; generated-block tests and coast screenshots cover it.

Screenshot and map PNGs were rendered and visually inspected locally. Automatic approval review rejected uploading these binary payloads to GitHub; they are omitted from this branch. The source harnesses and reproduction commands remain available below.

## Reproduce

```sh
cargo test --release --locked
cargo clippy --release --all-targets --locked -- -D warnings
cargo check --release --locked --no-default-features
cargo test --release --lib --manifest-path vendor/quad-snd/Cargo.toml
cargo run --release --locked -- --playtest --bots 6 --seconds 60 --seed 1234
MINCERAFT_BIOME_MAPS=out/maps cargo test --release --locked biome_maps -- --ignored
cargo run --release --locked -- --screenshot out/spring.png --mode settled_spring --frames 150 --distance 3
```

Other scenes: `depth_dripstone`, `depth_lush`, `depth_dark`, `settled_village`, `settled_mansion`, `climate_mod`. Install `example-mods/climate` as `mods/climate` for the last scene. Raw map palettes and dimensions are documented in the root README. Mod maps from a vanilla test registry contain zero; the CLI playtest and mod integration tests separately exercise climate-target content.

## River-bank correction

The initial revision 9 river cut stopped at the water biome and left a ledge up to the valley floor. Its replacement blends from four blocks below sea level through an underwater shelf to the dry valley floor over twice the channel half-width, with matching slopes at the shelf and bank joins. Revisions 0–8 keep their original river profiles. Profile tests limit adjacent one-block samples to at most one block of rise at normal channel widths; generated-bank tests inspect edge jumps across three seeds. The natural river screenshot is rechecked for this correction. This refines the unreleased revision 9; protocol 45 rejects clients running the previous bank profile.

While rechecking multiplayer, the playtest loading helper was corrected to wait for the 5×5 neighbourhood around the bot, rather than counting distant cached chunks. A regression test preloads a distant area and verifies that the requested local area is still loaded. This fixes premature reads after moves between cached regions.

## Relief, islands and beach ecology (revision 10)

Medium-scale inland hills add relief without shrinking continents or climate regions. Valley flattening ends at weirdness 0.19 instead of 0.32; the smooth river shelf/bank profile remains. Wetland detail blending is softer to keep swamp/jungle boundaries gentle.

Ordinary islands use a separate 1,024-block grid with a 24% candidate roll and 120–250-block shaping radii. Mushroom candidates move from a 640-block/40% grid to 4,096 blocks/18%, roughly 91 times fewer candidates per unit area before offshore filtering; shaping radii grow from 25–50 to 360–520 blocks. These are shaping radii, not exposed land diameters. Candidate centres and cardinal shores must lie in deep ocean. A regression checks hundreds of blocks of exposed mycelium and separating water.

Gentle beaches extend up to sea level +4 and gain gravel patches, sparse scrub, shared-anchor fallen log clusters and rare warm-shore turtle nests, using existing blocks and mechanics. No new originals are chosen.

A dedicated marine spawning pass tries four water habitats near a player per attempt. Local caps are 24 fish and 6 dolphins within 96 blocks; world limits range from 64 to 128 fish and 12 to 24 dolphins by player count. Each spawn checks water clearance. Marine populations no longer consume land-animal caps, and river fish use local limits. Existing hostile sea life and dark-water Glow Squid remain.

The socket playtest now checks island and decorated-beach regeneration, edit restoration, and delivery of newly spawned marine mobs to the joined player. Revision 9 chunk fixtures were captured from commit `abee1142af7856fb050548c39acec79e4b50f3ca` before this slice.

The revision 10 terrain slice passed 563 release tests, including local marine density, water clearance, hard caps, recycling of distant wild schools, beach support, island isolation/size, increased normalized inland relief, compatibility through revision 9 and socket delivery of marine mobs. Kept marine animals are not recycled. Wild fish and dolphins farther than 192 blocks from every player are removed to free cap space at newly explored coasts.

Ten matched 512×512-chunk surface maps, sampled every eight blocks, contained 203 dry mushroom-biome components in revision 9 and two in revision 10. The largest sampled dry mushroom component grew from 56 to 496 blocks wide. Across 4,016,492 matched dry inland samples, mean height change over 32 blocks increased from 3.07 to 3.61 blocks (about 18%). These are bounded survey statistics, not exact worldwide rates.

## Coastal species

The requested nine species have separate appended base-mob IDs, procedural textures, collision dimensions and models. Starfish and sea snails crawl slowly on shallow floors; clams, mussels and oysters remain on supported seabeds; crabs walk sandy beaches and shallow water; sardines spawn in schools of five to eight; tuna need at least twelve blocks of ocean depth; eels swim near the bottom. Their runtime spawning also works in older worlds without changing saved generator options or terrain. Frozen oceans are excluded.

The spawning pass samples actual loaded blocks and each species' full collision box, then rechecks every school/colony member's depth and support. Local limits within 96 blocks are twenty sardines, four tuna, six eels, eight crabs and six of each other species. The separate world cap is 80–160 coastal creatures by player count; kept animals count against limits, and distant wild populations recycle beyond 192 blocks of every player. These creatures consume neither land-animal nor hostile caps.

Five raw seafood items cook into Cooked Seafood, and integrate with existing Cooking Pot fish ingredients. Shellfish also leave seashells, with a three-shell to two-Bone-Dust recipe. Starfish have no food drop. Life at the Shore rewards a close encounter, and Shore Supper rewards obtaining raw seafood. Existing base item/mob IDs stay fixed; mod mob saves retain name keys and mod item saves use their palette, so their shifted runtime offsets are remapped on load. Save format 16 and terrain revision 10 remain. Protocol 47 prevents mixed old/new wire IDs.

Four new tests cover actual depth/support, Tuna body clearance and dry-cell rejection, natural Sardine/Tuna population limits, kept/wild cap behavior, bottom movement and stationary colonies, food/cooking, advancement and save round trips. Both socket playtests check delivery of all nine species and the encounter advancement. Controlled `coastal_floor` and `coastal_fish` aquarium scenes support model review; they use held review poses and the existing Night Vision effect, and are explicitly staged scenes rather than evidence of natural spawning.

The coastal art refinement replaces the shared diagonal-striping pattern with separate species/material textures and explicit top, underside and side faces. Starfish arms are fivefold symmetric, tapered and connected to a low raised centre; their existing collision box is unchanged. Shellfish show growth bands, the snail has a spiral shell and soft foot, crabs have mottled plates and segmented legs, and swimmers have scale rows, countershading, heads and fin rays. The Tuna's head remains fixed with its eyes while its tail/fins animate. `coastal_starfish` provides a close, elevated view for checking the silhouette. These changes affect presentation only; protocol, saves, spawning and terrain rules stay unchanged.

Art validation: three existing atlas tests and the coastal species/save/food round-trip test pass; release build and Clippy with warnings denied pass. The starfish close-up, coastal floor group and swimmer side views were rendered and inspected using software OpenGL and Night Vision.

## Beach finds and sheltered tide pools (revision 11)

New generation replaces dry log clusters with low, oriented Driftwood models and adds low decorative shell patches, waterlogged shallow shell beds and Dune Grass clumps. Driftwood crafts into two Planks or burns for ten seconds. Two seashells craft a placeable patch, which returns a seashell when harvested; the earlier three-shell/two-Bone-Dust recipe remains. Wet patches retain water after harvest/support loss. Dune Grass can be replanted above water on supported sand, red sand, gravel, dirt or grass.

Tide pools use 64-block candidate cells, a 28% candidate roll and two- or three-block radii, then require a complete level, mild-climate Beach footprint and rim with no mod-biome override. A sandstone support layer stops their sand floors falling into caves. Each pool is two water blocks deep and may contain shell beds. All placement decisions use global coordinates across chunk boundaries. Coastal habitats now measure actual local water height, letting existing shallow-water creatures inhabit the basins.

The save format remains 16, generation compatibility is checked through revision 10, and protocol 48 rejects older block/gen layouts. The new base blocks append after the village blocks; mod block palettes remap their shifted runtime IDs. The revision 10 fixture was captured from unmodified commit `d915575cf3fa27cc60d960958cce500414cb2064` before the beach changes.

Tests cover natural beach finds, closed and supported cross-chunk pools, water retention through simulation, shellfish habitat, placement/support, waterlogged harvest, resources, recipes and advancements. Socket playtests compare generated pool chunks, exercise every new block ID and restoration, and check joined-player placement/payment and both new advancements. `tide_pool` is a natural scene; `beach_showcase` explicitly stages Driftwood, Shell Patch and Dune Grass beside a generated basin.

Beach validation: 570 release tests pass, Clippy is clean, the reduced-feature build passes, and the six-bot/mod-enabled CLI playtest agrees (121 placed, 106 broken, 129 chat lines). The natural pool and staged decoration scenes were rendered and inspected with software OpenGL.

## Narrower beaches (revision 12)

The beach candidate rule now also requires sea water within a smoothly varied 16–28 block reach, sampled in eight directions at three distances. River channels do not qualify as sea. Low coastal plains farther from water use their inland climate biome; terrain elevation is unchanged. This keeps decorations and sheltered pool generation while removing broad sandy interiors. Revision 12 also permits a pool centre at sea level; the complete dry rim and supported floor checks remain, allowing pools on lower, narrower shores. The ocean and river rules, rocky headlands and mushroom islands retain their earlier identities. Desert coasts may still have sand inland because their climate is desert.

Ten seeds sampled across 8192 × 8192 blocks at 16-block intervals showed 41–53% less beach biome area. Regression coverage checks reduced area, unchanged heights and non-beach biomes, and a revision 11 coastal surface hash captured from `9aee2d6` before the change. Existing contained-pool, collectible-find and joined-player tests run against revision 12. New worlds default to revision 12; saved revisions 0–11 stay unchanged. Protocol 49 prevents mismatched shoreline regeneration.

## Larger parody example packs

The optional `half_baked`, `dubiety`, `ad_absurdum` and `dusk` folders add 53 declared blocks, 53 items, 58 recipes, 40 original text-defined textures, 15 mobs, six dimensions, three biomes and ten structures. Their individual READMEs document survival routes, crafting, commands, persistence and limits. No source code, textures or training text were copied from the inspiration mods.

Nuclear loops include refining, timed reactor batches, cooling-loss meltdowns, suited/shielded radiation, two armed/disarmable nukes and bounded, dimension-specific blasts/fallout. Space includes four rocket tiers, fuel, equipment/oxygen/heat checks, five separately saved worlds, ore progression, reusable launch equipment and a custom supply rover. Dusk confines all terrain and scripted wild spawns to its own dimension, with three biomes and four cooperative, progression-gated guardian encounters. Dubiety embeds a 4,681-byte original corpus and generates replies with 525 trigram states and 558 weighted transitions; private notes and short prompt history are bounded and saved.

The script API now exposes stable dimension keys, worn equipment, realm-specific reads/actions and host-owned safe landing travel. Mod dimensions can set bounded, finite player gravity; old definitions default to one. Protocol 50 prevents old clients using Earth gravity for these planets. Save format 16 and Overworld revision 12 are unchanged.

Four native advancements accompany the packs. Controlled real-game QA scenes are `half_baked_lab`, `astra_rocket`, `dusk_guardians` and `dubiety_companion`; all use the latest generator and require the packs installed. The examples are deliberately compact: command-operated machines, staged rocket departures and realm landings, a supply companion rover, template ruins, per-player oxygen, and word-transition language generation.

Validation for the parody slice: 584 release tests passed (one map-export test ignored); the eleven pack tests also passed after the final script population guards. Clippy on all targets with warnings denied and the no-default-features check are clean. The real socket test verifies a joined player assembling and paying for a rocket, arriving in the Moon realm with the host remaining in the Overworld, recovered launch gear and local assistant replies. A game-file save/reload test verifies private notes, a pending reactor batch and a planet realm. All four native showcase scenes were rendered and visually checked.

Rocket model refinement: three mod shape presets provide a faceted fuselage, tapered nose and a bell nozzle with four swept fins. Four new metal/paint textures replace the original pillar-like appearance. Existing rocket block keys and five-block assembly scripts are unchanged. Protocol 51 rejects clients without the shape presets; model tests verify taper, four fin directions, collision bounds and inventory geometry.

Rocket refinement validation: 585 release tests passed (one map export ignored), Clippy with all targets and warnings denied is clean, and the updated `astra_rocket` scene was rendered and visually checked.

Dusk custom trees: Twisted Duskwood, Gloam Pine and Moonwillow replace its generic tree silhouettes. They remain exclusively in the forest dimension, have cutout foliage, and provide three craftable wood sets (planks, slabs and stairs). Optional biome tree styles are deterministic and bounded to a six-block halo; the old four-field tree path is unchanged. Protocol 52 protects custom-tree regeneration. `dusk_trees` stages all three shapes with the latest generator.

Dubiety now has a separately summoned Obesity variant with a larger orange companion model and a dedicated beacon. It accepts all offered item types and swallows living cows, pigs, sheep and chickens inside a five-block sphere without death loot. Hunger demands repeat every eight seconds; feeding, realm isolation and client inventory/removal synchronization are verified independently of the ordinary word-model assistant. Protocol 53 protects interaction precedence.

Rocket trips now show the shaped hull moving with its pilot, a first-person cockpit, full-bright engine flames and moving exhaust. Ascent lasts four seconds and destination descent lasts three; gear is recovered after touchdown. Native flight snapshots are realm-local and host-authoritative, and saved Rhai phase records restore animation after loading. Protocol 54 protects the new snapshot message.


## Glutton swallowing animation

Obesity now opens a toothy mouth on its existing face and pulls a temporary model of nearby livestock through a hopping, shrinking arc. At 1.05 seconds the meal disappears into the mouth; a belly squash/bounce and a synthesized burp finish the 1.8-second sequence. Babies and sheared sheep keep their appearance. Only the cosmetic copy remains after the host consumes livestock, so it cannot be attacked, bred, milked or saved again, and creates no loot or kill rewards.

Each realm owns at most 64 visual meals. Periodic host snapshots include the eater/prey IDs, appearance, origin and current phase; new arrivals receive the same state. The effect expires without replaying on save/load. Protocol 55 protects the snapshot and appended sound ID; generation revision 12 and save format 16 stay unchanged. `glutton_swallow`, `glutton_gulp` and `glutton_belly` stage the actual effect for render inspection.

Validation: 591 release tests passed (one manual map export ignored); all-target release Clippy with warnings denied and the no-default-features check passed on Rust 1.90.0. Focused lifecycle/protocol checks were rerun after visual alignment. All three native animation stages were rendered and inspected; synthesized audio tests pass, but playback could not be auditioned without a sound device. Existing Bottomless Appetite feeding progression remains available.


## Player animation polish

The approved follow-up adds a cosmetic pose controller shared by built-in/custom player skins, armour and trims. Knees and elbows articulate while split cuboids retain the original side UV ranges. Exponential pose blending connects crouching, moving/stationary swimming, creative hover and directional flight, gliding, jumping and a short landing compression; Glider wings blend with the glide pose. Physics, camera height, collisions and food/potion application timing retain their existing rules.

Successful consumption triggers a bounded 1.25-second hand-to-mouth follow-through using the actual consumed item, including the final item in a stack. Food has crumbs and drinks have a distinct tilt. First-person hands use the selected custom skin; third-person items follow the same hand bone as the skin and armour. The host validates cosmetic item types, limits request frequency, replaces supplied player IDs with the sender, and filters creative-flight flags from survival peers. Swimming is derived from water in the correct realm. Late-arriving viewers receive active meal elapsed times; death, beds and rocket seats suppress local gestures.

Obesity now focuses on the nearest claimed meal, crosses its hungry pupils and leans back for a 0.45-second inhale. The cosmetic animal waits during anticipation, then takes the same shrinking arc into the mouth. Gulp and burp occur at 1.45 seconds; the complete effect lasts 2.2 seconds. Host burps now forward to realm observers. Protocol 56 protects the flight flag, meal gesture message and changed glutton timing; generator revision 12 and save format 16 are unchanged.

Native QA modes: `pose_swim`, `pose_tread`, `pose_hover`, `pose_fly_up`, `pose_fly_down`, `pose_glide`, `pose_crouch`, `pose_eat`, `pose_drink`, `pose_eat_first`, `pose_drink_first`, `pose_landing`, `pose_transition` and `glutton_inhale`. Player displays compare built-in, diagnostic custom and armoured skins through the real controller and renderer.

Animation validation on Rust 1.90.0: 600 release tests passed, zero failed, one manual map export ignored. All-target release Clippy with warnings denied and the no-default-features check passed. Native swimming/treading, hover/ascent/descent, glide, crouch, consumption in both views, transition/landing and glutton anticipation scenes were rendered and visually inspected across built-in/custom/armoured models. Socket tests cover gestures, host-filtered flight flags, late-join meal ordering, and the glutton burp reaching observers. Audio synthesis tests pass; playback was not auditioned in this environment.


## Action and machine interaction polish

The user approved item 13 for the current development scope: third-person main/offhand equipment, tool and combat gestures, climbing, mob locomotion transitions, and native controls for the existing nuclear/space machinery. Held sprites use the existing extruded atlas geometry; block items use their normal miniature model. Both hands follow the forearm bones shared by skins and armour. Shields raise from either hand, bows show a two-hand draw while use is held and fire on release, spears thrust, attacks follow through and hurt players recoil. Menus and item changes cancel a bow draw.

Ladder, rope, cave-vine and scaffold poses follow the same world contact check as physics. Hands and knees alternate with vertical travel; hanging holds the current cycle. Mob swimming and flying add blended pitch/bank/bob, and sitting fades into folded legs and a lowered body; host and client interpolation use the same cosmetic update without changing collision boxes or AI.

Machine panels cover Half Baked's centrifuge, reactor and cooling tower, plus Ad Absurdum's refinery, oxygen generator, solar panel, rocket workbench and launch pad. They show ingredient counts, missing ingredients, available controls, and host-reported reactor/recharge progress. Buttons send bounded action indices; the host checks the actual block, player life state, reach and request rate, then invokes the existing recipe script under the sender's name in their realm. No recipe or arbitrary command is accepted from the client. Controls close on removal, death, leaving reach or changing realm. Commands remain available and retain inventory payment, cooling, ownership, cooldown and save behavior. Opening a panel awards a cosmetic advancement.

Protocol 57 extends player state with offhand and combat presentation and adds machine requests/replies. Save format 16 and generator revision 12 are unchanged. Native QA modes are `pose_tool`, `pose_shield`, `pose_bow`, `pose_spear`, `pose_climb`, `pose_hang`, `pose_hurt`, `mob_movement`, `machine_reactor`, and `machine_rocket`; all use the latest generator.

Verification: `cargo test --release` passes 607 tests (one manual export test ignored); `cargo clippy --all-targets -- -D warnings`, `cargo check --no-default-features`, and the release build pass. New real-socket checks cover equipment/shield/bow presentation and machine recipe payment/refused requests. Regression checks cover bow release/cancellation, both-hand poses, climbing/hanging, finite mob blending, reactor/solar progress, invalid machine indices/coordinates, reach, death and block removal. Ten native screenshot scenes were rendered and reviewed, including live reactor progress and the missing-ingredient rocket controls.
