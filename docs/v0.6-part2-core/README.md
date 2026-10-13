# v0.6 Part 2 core — development branch

Implements roadmap items 4 to 9. The two originals remain undecided, and Part 2 is not marked released. No changes are merged into main and no release is published.

## Behavior and compatibility

- New worlds use generation revision **10** for relief, islands and beach ecology. Revision 9 retains the previous terrain refinements. The original core is revision 8, with revision 7 surface shape/climate. Revisions **0–9** retain their terrain, cave, mod-patch and placement paths. The save format remains version 16; generator options still use the existing packed field. Protocol **47** protects revision 10, the appended coastal mob IDs and the new item IDs.
- The depth-aware cave selector reuses surface temperature, moisture, continentalness and erosion. Deep Dark takes precedence below height 24 under inland rugged land. Damp, mild land has lush caves from height 24; drier, non-frozen inland land has dripstone at heights 10–48. Bedrock and the top eight blocks below the surface are excluded. Existing carving, blocks, geodes and survival mechanics remain.
- Climate-target mods use optional bounded axes, a 0.35 per-axis eligibility radius, normalized mean squared distance and stable-key tie-breaking. Targetless patches retain first-match priority. Built-in shape landmarks and rare variants are protected. Targets are ignored in older worlds and rejected for mod dimensions.
- Surface structures validate their complete bounded footprints, including interiors. Villages check squares, paths, farms and houses; growth sites are filtered too. Mansions remain inside dark forest, and springs require cold, level ground. Overworld mod templates receive supporting foundations, and settling aprons preserve adjacent waterways.
- Existing Stalac-tight, Lush Life and Homebrew advancements remain earnable and are exercised by socket playtests. No unrelated new content or originals are introduced.

## Validation

Validated on Linux x86-64 with **Rust 1.90.0**, locked dependencies and the normal release profile (thin LTO):

- `cargo test --release --locked`: **567 passed, 0 failed, 1 ignored** (the manual map export).
- `cargo clippy --release --all-targets --locked -- -D warnings`: clean.
- `cargo check --release --locked --no-default-features`: passes.
- Audio mixer tests: **2 passed**. This container required the GNU bfd linker for the standalone vendor test binary.
- Six-bot, 60-second CLI socket playtest, seed 1234: everyone agrees (122 blocks placed, 106 broken, 129 chat lines). Includes natural cave generation, village foundations, climate-target mod surfaces, bounded rivers, rocky coasts, islands, beach ecology, all nine coastal species, marine mob snapshots, host edit/restore propagation and advancement delivery.
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
