# v0.6 Part 2 core — development branch

Implements roadmap items 4 to 7. The two originals remain undecided, and Part 2 is not marked released. No changes are merged into main and no release is published.

## Behavior and compatibility

- New worlds use generation revision **9** for the requested terrain refinements. The original core is revision 8, with revision 7 surface shape/climate. Revisions **0–8** retain their terrain, cave, mod-patch and placement paths. The save format remains version 16; generator options still use the existing packed field. Protocol **44** prevents older clients from silently clamping revision 9 to revision 8.
- The depth-aware cave selector reuses surface temperature, moisture, continentalness and erosion. Deep Dark takes precedence below height 24 under inland rugged land. Damp, mild land has lush caves from height 24; drier, non-frozen inland land has dripstone at heights 10–48. Bedrock and the top eight blocks below the surface are excluded. Existing carving, blocks, geodes and survival mechanics remain.
- Climate-target mods use optional bounded axes, a 0.35 per-axis eligibility radius, normalized mean squared distance and stable-key tie-breaking. Targetless patches retain first-match priority. Built-in shape landmarks and rare variants are protected. Targets are ignored in older worlds and rejected for mod dimensions.
- Surface structures validate their complete bounded footprints, including interiors. Villages check squares, paths, farms and houses; growth sites are filtered too. Mansions remain inside dark forest, and springs require cold, level ground. Overworld mod templates receive supporting foundations, and settling aprons preserve adjacent waterways.
- Existing Stalac-tight, Lush Life and Homebrew advancements remain earnable and are exercised by socket playtests. No unrelated new content or originals are introduced.

## Validation

Validated on Linux x86-64 with **Rust 1.90.0**, locked dependencies and the normal release profile (thin LTO):

- `cargo test --release --locked`: **556 passed, 0 failed, 1 ignored** (the manual map export).
- `cargo clippy --release --all-targets --locked -- -D warnings`: clean.
- `cargo check --release --locked --no-default-features`: passes.
- Audio mixer tests: **2 passed**. This container required the GNU bfd linker for the standalone vendor test binary.
- Six-bot, 60-second CLI socket playtest, seed 1234: everyone agrees (103 blocks placed, 87 broken, 130 chat lines). Includes natural cave generation, village foundations, climate-target mod surfaces, bounded rivers, rocky coasts, host edit/restore propagation and advancement delivery.
- The test suite also runs a two-bot/seed-77 scenario and a six-bot/seed-1234 scenario with base content, without installing a mod registry concurrently.
- Released-generation regression fixtures were captured from merge commit `3ef73d102c6119e4092c2e59b69e47c8c1d4c556` before implementation: seed 424242, chunks (0,0), (-7,11), (31,-29), for every revision 0–7. Revision 8 was captured from commit `59c38343a05c72044b06a08e49f309bafe8feae2` before the revision 9 edits; all nine fixtures pass.
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

Across the ten seeds used for visual comparison (512×512 chunks, 8-block sampling), mean adjacent-sample biome changes declined from 15.08% (revision 8) to 6.80%; sampled oceans changed from 29.61% to 32.70%, rivers from 3.25% to 1.29%, and snow/ice-spike land from 10.26% to 7.86%. These bounded surveys do not measure exact worldwide area or establish equivalence to Minecraft. Surface biome maps do not show rocky material; generated-block tests and coast screenshots cover it.

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
