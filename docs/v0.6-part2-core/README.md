# v0.6 Part 2 core — development branch

Implements roadmap items 4, 5 and 6. The two originals remain undecided, and Part 2 is not marked released. No changes are merged into main and no release is published.

## Behavior and compatibility

- New worlds use generation revision **8**, retaining revision 7 surface shapes and climate. Revisions **0–7** retain their terrain, cave, mod-patch and placement paths. The save format remains version 16; generator options still use the existing packed field. Protocol **43** prevents older clients from silently clamping revision 8 to revision 7.
- The depth-aware cave selector reuses surface temperature, moisture, continentalness and erosion. Deep Dark takes precedence below height 24 under inland rugged land. Damp, mild land has lush caves from height 24; drier, non-frozen inland land has dripstone at heights 10–48. Bedrock and the top eight blocks below the surface are excluded. Existing carving, blocks, geodes and survival mechanics remain.
- Climate-target mods use optional bounded axes, a 0.35 per-axis eligibility radius, normalized mean squared distance and stable-key tie-breaking. Targetless patches retain first-match priority. Built-in shape landmarks and rare variants are protected. Targets are ignored in older worlds and rejected for mod dimensions.
- Surface structures validate their complete bounded footprints, including interiors. Villages check squares, paths, farms and houses; growth sites are filtered too. Mansions remain inside dark forest, and springs require cold, level ground. Overworld mod templates receive supporting foundations, and settling aprons preserve adjacent waterways.
- Existing Stalac-tight, Lush Life and Homebrew advancements remain earnable and are exercised by socket playtests. No unrelated new content or originals are introduced.

## Validation

Validated on Linux x86-64 with **Rust 1.90.0**, locked dependencies and the normal release profile (thin LTO):

- `cargo test --release --locked`: **549 passed, 0 failed, 1 ignored** (the manual map export).
- `cargo clippy --release --all-targets --locked -- -D warnings`: clean.
- `cargo check --release --locked --no-default-features`: passes.
- Audio mixer tests: **2 passed**. This container required the GNU bfd linker for the standalone vendor test binary.
- Six-bot, 60-second CLI socket playtest, seed 1234: everyone agrees. Includes natural cave generation, village foundations, climate-target mod surfaces, host edit/restore propagation and advancement delivery.
- The test suite also runs a two-bot/seed-77 scenario and a six-bot/seed-1234 scenario with base content, without installing a mod registry concurrently.
- Released-generation regression fixtures were captured from merge commit `3ef73d102c6119e4092c2e59b69e47c8c1d4c556` before implementation: seed 424242, chunks (0,0), (-7,11), (31,-29), for every revision 0–7.
- Climate parser tests cover finite bounds, omitted axes, old defaults and unsupported dimensions. Integration tests check generated mod-template blocks, foundations, chest locators and a template spanning a chunk boundary.
- Twelve-seed automated surveys check variety, sea/land balance, river/beach height constraints and absence of directly adjacent hot/frozen biomes. Surface maps and structure overlays for all twelve seeds, plus cave slices, were visually reviewed.
- Seven newest-generation screenshot scenes were rendered and inspected using software OpenGL. Audio output was unavailable in this container; audio playback was not auditioned. Windows and macOS builds were not run locally.

The container's newer Rust 1.99 toolchain reports existing lints outside this slice; the clean Clippy result above is specifically for Rust 1.90.0. No broad lint refactor is bundled into the terrain change.

## Review corrections

Four regression tests first failed against the initial implementation and pass after correction:

1. Revision 8 cave decorators now start at height 3 and let the depth selector reserve the Deep Dark. This removes the inherited two-layer decoration gap at heights 24 and 25.
2. Mod-template reach calculations include the four-block settling apron, so the apron is also generated in neighbouring chunks that contain no template blocks. Older revisions retain their original reach calculation.
3. Climate targets with misspelled `in` restrictions are rejected, rather than treating an empty parsed list as an unrestricted target. Targetless mod parsing is unchanged.
4. Worlds using revisions 0–7 retain the released column-based cave advancement rule. Only revision 8 uses depth-aware advancement selection.

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
