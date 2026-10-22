# Caves, Desert Oases And Mushroom Park

## Scope

The existing generator, section mesher, tree primitives, prop batching and water
columns were extended. No dependency versions, world IDs, seeds or existing
block IDs changed. The dedicated server still uses the same generator without
rendering. No parallel ecology, storage, block registry or renderer was added.

- Natural hillside entrances descend into variable passages and larger chambers
  connected to the original 3D cave field. Floors remain walkable and supported.
  Thin or disconnected roof spans are opened instead of leaving floating blocks.
- A bounded near-player prefetch adds at most 18 deep sections per focus; it uses
  prepared columns without synchronous terrain sampling on the main thread.
- Desert sand covers sandstone and then stone. Dead bushes occupy dry regions;
  straw-colored grass and curved palms grow along suitable oasis banks.
- Rare irregular oases use the lowest surrounding ground to choose an enclosed
  local water level. Rendering, fluid sources, collision queries and audio share
  the existing water representation. Desert river selection retains roughly one
  seventh of whole candidate paths, without truncating a river at a biome edge.
- Mushroom Park is a sixth smooth climate weight. Giant mushrooms and six small
  mushroom colors use the existing deterministic vegetation placement system.
- New IDs 20..26 are a cream/brown stem and purple, red, yellow, green, blue,
  white caps. Caps share a pale gill underside. These are ordinary solid cubes,
  with seven JSON definitions, eight shared procedural texture patterns and no
  GLBs. Inventory, mining, placement, physics, saves and UDP replication support
  them. There are 26 cube types, 28 base materials and 44 including props; these
  are asset counts, not draw-call counts.
- Spruces, grouped forest species, fern understory, meadow color patches and
  slightly more varied deep ocean beds retain the original biome transitions.

## Compatibility And Boundaries

World format 1 and generator version 5 remain. Network protocol 10 requires both
client and server to update. New biome profiles change unedited terrain and
vegetation even in old v5 worlds; saved edits still win at their original cells.
Prop IDs include support height, so changed terrain can generate a different
candidate above an old removed prop. No old journal entry is deleted. New worlds
are recommended for exploring this landscape update; bit-identical regeneration
of the previous terrain is not claimed.

Whole-tree felling and logical leaf decay are designed in README, not implemented
as automatic gameplay. General stacked underground aquifers were assessed:
WaterColumn currently represents one vertical water body per X/Z. Multiple
intervals need a coordinated fluid/mesh/physics/audio/network extension. Existing
river galleries and the new isolated oases do work; arbitrary aquifers do not.

## Native Views

The Vulkan client on AMD RX 6950 XT generated the actual seed-42042 landscape,
with no artificial biome edits. Overview and close screenshots are in captures/:

- Oasis: X/Z -2682/-153, local level 26.3783, radius about 23 m.
- Mushroom Park: region X/Z -640/640, genuine stem/cap structures.
- Cave: mouth about X/Y/Z -901/16/-999, view into the descending passage.

The biome fixture waits for generation, render and water readiness, checks loaded
feature blocks, and waits for the actual screenshot callback before exiting.
These are inspected native screenshots, not complete manual world exploration or
art-direction acceptance. The existing distant atmosphere/horizon band remains
an open visual issue. An offline material fixture separately places all 26
palette entries through the existing EditJournal/section path and requires
genuine nonblank GPU previews. Authoritative placement/mining is validated
separately by the real UDP integration tests, not by that offline screenshot.

The first mushroom run overlapped compilation and exposed an existing overly
short frame-count preview timeout for foliage. Readiness now uses a bounded
10-second wall-clock grace, 200 ms between retries and a 3-second readback timeout.
It still rejects blank images rather than supplying placeholders. One audio
underrun during that overloaded exploratory run is not an audio quality result.
Final oasis, mushroom, cave and material logs contain no WARN/ERROR messages;
the exploratory mushroom log is retained separately.

## Validation And Measurements

All five workspace commands passed: fmt --check, check, test, Clippy --all-targets
with -D warnings, and build. **419 tests passed, zero failed; one existing long
UDP soak remains ignored.** Coverage includes nine walkable cave portals across
three seeds, roof connectivity, finite endpoint densities, negative section
boundaries, cache/reload determinism, real oasis/palm/park placement, all six
connected giant mushroom caps, local lake levels, supported shores, desert
strata, sand outside the oasis grass margin, bounded underground prefetch,
shared prop batches, every block ID through
real UDP placement/mining, saves and genuine preview source geometry.

During integration, two old assertions needed intentional updates: terrain
height snapshots now include the new mixed biome, and the prop-color test now
uses the purple mushroom palette slot. The expanded UDP palette loop now respects
the unchanged production rate budget. One exploratory parallel UDP run failed at
server startup before the gameplay assertion; the failure message was improved,
and the final full parallel run passed. No network limits were weakened.

Measurements use the same seed, config, 1920x1200, FOV90, VSync off, radius16/3,
offline mode, fixed camera transforms and 18-second sample windows. Before logs
were captured from the previous binary before compilation. No Cargo process or
second Terra client ran during the benchmarks. One before sample and two after
samples per scene were captured; the table uses the final source build after
the oasis sand correction. This is a short comparison, not a statistical
performance guarantee. Earlier after runs remain in the raw logs.

| Metric | Old Forest Before | Same Position After (Park) | River Before | River After |
|---|---:|---:|---:|---:|
| Average FPS | 110.023 | 168.144 | 145.490 | 144.067 |
| p95 frame ms | 10.582 | 7.346 | 8.519 | 8.408 |
| p99 frame ms | 12.429 | 8.631 | 10.491 | 10.538 |
| Initial load s | 3.502 | 2.600 | 3.096 | 3.108 |
| Process RSS KiB | 876652 | 825592 | 857676 | 839300 |
| Loaded sections | 7704 | 7774 | 7635 | 7643 |
| Rendered sections | 3531 | 3219 | 4345 | 4286 |
| Mesh assets | 9315 | 8417 | 9336 | 9292 |
| Visible mesh entities | 4848 | 4173 | 4833 | 4837 |
| Represented props | 89866 | 91784 | 63116 | 64599 |
| Prop batches | 1277 | 1238 | 1228 | 1221 |
| Terrain composition worker sum s | 0.721 | 0.477 | 0.553 | 0.557 |
| Prop composition worker sum s | 0.270 | 0.236 | 0.176 | 0.178 |
| Main opaque GPU pass mean ms | 1.943 | 1.047 | 1.145 | 1.157 |

The old forest camera now sees Mushroom Park: fewer complex foliage surfaces
explain why this is **not** a pure rendering optimization comparison. The river
scene changes less; final average FPS differs by -0.98%, p95 is slightly lower
and p99 increases by 0.047 ms. The earlier after run measured 144.887 FPS and
10.390 ms p99. Terrain composition worker sum in the final run increases by
about 0.004 s; this is explicitly
retained rather than described as an across-the-board speedup. Existing foliage
costs and long-session/streaming frame-tail work remain open in TODO.

Worker sums are not wall latency. Individual GPU pass times are not total GPU
frame time; mesh/entity counts are not exact draw calls. Exact draw calls and
total GPU frame time were not measured. The wrapper also records peak RSS;
that accounting differs from the application's sampled /proc RSS.

The frame-paced CPU-only streaming example, with GPU uploads excluded:

| Phase | Before | After |
|---|---:|---:|
| Cold | 4375 sections / 2.626 s | 4375 sections / 2.626 s |
| One-column move | 4375 / 0.117 s | 4375 / 0.117 s |
| Distant eviction | 5273 / 4.934 s | 5303 / 4.968 s |
| Reload | 4375 / 2.626 s | 4375 / 2.626 s |

The distant region now contains 30 more active sections and takes two extra
scheduled frames. This is a frame/job-budgeted workload, not unthrottled CPU
generation throughput. Cold-load timing has not regressed in this workload.

The separate `cargo build -p terra-server` and smoke start with DISPLAY and
WAYLAND_DISPLAY unset both passed. Its standalone dependency tree contains no
terra-render, bevy_render, bevy_pbr, bevy_winit, TiltUI, rodio or bevy_audio.

## Reproduction

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo build -p terra-server
env -u DISPLAY -u WAYLAND_DISPLAY target/debug/terra-server --bind 127.0.0.1:0 --no-save --smoke-test
cargo run --locked -p terra-world --example oasis_regions
cargo run --locked -p terra-world --example terrain_regions
cargo run --locked -p terra-world --features bevy --example streaming_benchmark
TERRA_BIOME_SCENE=oasis TERRA_CAPTURE_DIR=/tmp/biomes target/debug/terra-client --smoke-test-biomes
TERRA_BIOME_SCENE=mushroom TERRA_CAPTURE_DIR=/tmp/biomes target/debug/terra-client --smoke-test-biomes
TERRA_BIOME_SCENE=cave TERRA_CAPTURE_DIR=/tmp/biomes target/debug/terra-client --smoke-test-biomes
TERRA_SCREENSHOT=/tmp/materials.png target/debug/terra-client --smoke-test-materials
TERRA_FOREST_BASELINE_VIEW=1 TERRA_BENCH_SECONDS=18 TERRA_RENDER_DIAGNOSTICS=1 TERRA_BENCH_MODE=offline TERRA_BENCH_FULL_MICRO=0 python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-forest
TERRA_RIVER_BASELINE_VIEW=1 TERRA_BENCH_SECONDS=18 TERRA_RENDER_DIAGNOSTICS=1 TERRA_BENCH_MODE=offline TERRA_BENCH_FULL_MICRO=0 python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-river-benchmark
```
