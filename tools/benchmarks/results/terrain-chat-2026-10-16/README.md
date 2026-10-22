# Terrain Refinement And Authoritative Chat

## Report Status

This report is being assembled during implementation. The before measurement
below was captured from the previous binary. All five workspace checks passed.
The fixed-camera after run was rejected because the new mountain surrounds the
old camera position; **there is no valid paired native FPS comparison**. Native
feature views were inspected separately and are identified below.

## Architecture

The existing terrain generator, section cache, density field, vegetation shapes,
batched block renderer and client/server session path are retained. No dependency
upgrade, replacement renderer, second prop system or individual micro-voxel
entities were introduced.

- Climate still creates smoothly blended terrain profiles. An additional
  climate-driven mountain uplift produces genuine high ranges. Public
  `biome_at` and prepared `SurfaceColumn.biome` suppress Highland below Y80 and
  blend its ecology in from Y80 to Y108. Suppressed cover becomes forest/plains;
  ocean and desert weights are not reclassified. Internal height generation uses
  the original climate sample to avoid recursive biome/height evaluation.
- The new default snow line is Y112. Explicit snow heights in existing world
  manifests/configurations remain authoritative. Snow rendering uses a complete
  snow cube instead of a stone-sided block with a white top.
- Exactly the uppermost solid grass voxel is Grass, accounting for fractional
  and negative surface heights. At least three Dirt voxels lie beneath it.
  Local relief is the height above the lowest of four adjacent columns:
  shallow rock exposure starts at three metres, blending over three to seven
  metres. A coherent regional mask also retains deeper green mountain soils.
  Sandstone deposits apply below actual Sand, not below oasis/transition Grass.
- Existing cave noise varies between narrow passages and fissures. Rare deep
  chambers use deterministic ellipsoidal density combined with the same tunnel
  noise, approximately 74..86 m wide and 44..52 m high for the largest class.
  There is no separate cave mesher. Chamber/profile data is prepared per column;
  the voxel path still uses the existing two 3D noise evaluations. Exterior roofs
  retain five metres of protection. Explicit river/entrance roofs and two-metre
  floor bands prevent an intersecting chamber from removing their support.
- Open river banks blend over 14..64 m instead of narrow cut walls. Bank reach
  is limited at drainage-cell borders, roof erosion varies more gently, and
  exposed fork islands are limited to 2.5 m above water. Existing local water
  levels, covered reaches, waterfalls, fluid cells and supported-roof checks
  remain in use. These changes do not promise that every natural canyon wall
  becomes shallow.
- Giant mushroom variants share existing block IDs and tree/section batching,
  with different cap proportions, asymmetric young caps and extra surface
  detail. Their attached face relief is at most 6 cm deep and bounded to 320
  surface vertices per source face; a selected raised scale adds 24 vertices,
  not a new entity. Cap-scale coverage varies by color from 12% to 46%, reuses
  the existing White Cap material, and protrudes only 1.2..3.5 cm. Undersides use
  short lamella ridges. This geometry follows the existing 32 m micro-detail
  band with 28/36 m hysteresis, not the whole terrain LOD0 region. Spruces are
  about 40% taller, have one-block vertical trunks, and end
  in foliage rather than exposed wood tips. No per-detail entities or GLBs are
  required for these ordinary block structures.
- Tilt-UI chat sends text and `/locate biome <name>` through the existing
  authoritative session. Locate searches run as bounded worker tasks, not as
  unrestricted main-thread terrain scans. Results use structured biome names,
  coordinates and server-issued teleport tokens; the UI colors the name and
  coordinates purple/pink and the action yellow. The server checks permission,
  target freshness, safe landing and loaded world state before teleporting.
  Chat has bounded messages, history, request queues and rate limits. Integrated
  and dedicated servers use this same path; no client-side position spoofing is
  introduced. Teleport is restricted to Creative or existing environment
  permission, and unsafe destinations have no active teleport action.

Height/ecology preparation is shared by vertical section jobs. Normal terrain
meshing does not query `biome_at` for each voxel. Terrain/prop tinting takes four
biome samples per section; public biome queries now include height work where a
Highland climate weight is present. This is an additional cost, not a claimed
free optimization. Final measurements must establish its practical impact.

## Compatibility

World format 1, generator version 5, existing seeds and block IDs remain intact.
Network protocol 11 adds chat and generation-tagged teleport synchronization;
client and server must be updated together. Unedited terrain and vegetation can
change substantially because real mountains, cave forms and grass stratification
changed. Saved block edits retain their original cell coordinates and still take
precedence; no journal entries are discarded or migrated destructively.

Prop IDs include generated support coordinates, so changed surface heights can
produce different candidates above a previously removed prop. Bit-identical
regeneration of earlier v5 landscapes is not claimed. A new world is recommended
for evaluating this terrain update. Existing worlds with an explicit snow line
of 52 retain it; they do not silently receive the new default of 112.

Locate is a bounded search, not a globally nearest-biome guarantee. Its square
search rings reach 6144 m along each horizontal axis on a 96 m grid, then check
nearby four-metre offsets for safe landing. A missing result means no matching
candidate was found within that search, not that the biome does not exist in the
infinite world. Ocean results may lack a safe dry
landing and therefore have no usable teleport action. Chat does not implement an
account provider, operator console or full command framework.

## Before Measurement And Rejected Comparison

Raw source: [before log](native/terra-refinement-before-river.log), captured on 2026-10-16 at
06:41 UTC. The old binary completed the offline river benchmark with exit code 0.
Adapter: AMD Radeon RX 6950 XT, RADV NAVI21, Mesa 26.2.4-arch3.1, Vulkan.
The actual physical window was 1920x1200. The recorded fixed camera position is
`[102.5, 52.364502, 416.5]`, orientation quaternion
`[-0.21340242, 0.2545232, 0.057693165, 0.9414611]`.

The benchmark used an 18-second frame sample window, offline mode and no
controller calls. Screenshot-adjacent frames are excluded by the harness. An
after run must retain the same settings, fixed camera and sample window. Because
terrain itself changes, a same-coordinate view may contain different geometry
or even become obstructed; such a result must not be presented as a pure renderer
speed comparison. Native feature views are separate from this fixed benchmark.

The after run completed but its screenshot showed the camera inside the new
massif, looking through back faces instead of the original river landscape.
It represented zero props instead of 64599. Its 146.589 FPS, 8.448 ms p95 and
10.043 ms p99 are **discarded observations**, not an improvement, regression or
usable after baseline. The raw run is retained under `discarded/`. A valid paired
renderer comparison needs frozen saved terrain or a new baseline with matched
visible geometry. The table below therefore has no valid after values.

The retained `configs/` directory freezes the river benchmark settings, including
the original snow line of 52, to match the valid initial baseline. Native feature
captures instead exercise the new default snow line of 112. A later attempt to
rerun the old binary against the edited live assets failed before rendering:
the old executable did not recognize the new `mushroom` surface type or
`chat-panel` component. That failed run is excluded completely from timing
comparisons; it is not a slow/fast sample. The valid 142.635 FPS before result
above was recorded before those edits, with its matching assets.

| Metric | Before | Valid After |
|---|---:|---:|
| Sampled frames | 2567 | Unavailable |
| Mean FPS | 142.635 | Unavailable |
| p95 frame interval ms | 8.594 | Unavailable |
| p99 frame interval ms | 11.102 | Unavailable |
| Initial load s | 3.115 | Unavailable |
| Application sampled RSS KiB | 848692 | Unavailable |
| Wrapper peak RSS KiB | 845020 | Unavailable |
| Loaded sections | 7643 | Unavailable |
| Rendered sections | 4286 | Unavailable |
| Mesh assets | 9292 | Unavailable |
| Visible mesh entities | 4837 | Unavailable |
| Loaded active props | 64599 | Unavailable |
| Represented props | 64599 | Unavailable |
| Prop batches | 1221 | Unavailable |
| Water tiles | 45 | Unavailable |
| Terrain composition jobs | 4286 | Unavailable |
| Terrain worker sum s | 0.563 | Unavailable |
| Maximum terrain composition job ms | 9.291 | Unavailable |
| Prop composition jobs | 1221 | Unavailable |
| Prop worker sum s | 0.185 | Unavailable |
| Whole process elapsed s | 23.822 | Unavailable |
| Whole process user CPU s | 41.566 | Unavailable |
| Whole process system CPU s | 7.382 | Unavailable |

The RSS values come from different sampling/accounting paths and are not
interchangeable. CPU seconds include the complete process run, whereas the
frame sample covers only the steady measurement window. Parallel worker sums
are not elapsed generation latency. Frame intervals measured on the CPU include
scheduling/presentation behavior and do not measure pure CPU execution time.

Selected instrumented GPU passes, 2563 samples before:

| GPU Pass | Before Mean ms | Before p95 ms | Before p99 ms | After Mean ms |
|---|---:|---:|---:|---:|
| Early prepass | 0.605 | 0.664 | 0.968 | Unavailable |
| Main opaque | 1.162 | 1.246 | 1.541 | Unavailable |
| Main transmissive | 0.300 | 0.314 | 0.331 | Unavailable |

These are individual instrumented pass durations, **not total GPU frame time**.
No summed total GPU latency or exact draw-call count is inferred. Mesh assets,
entities, batches, primitive counts and shader invocation counts are not draw
calls. Total GPU frame time and exact draw calls remain unavailable.

## CPU Streaming

Sources: [CPU before](native/terra-refinement-cpu-before.log) and
[CPU after](native/terra-refinement-cpu-after.log). Both runs completed the existing
`streaming_benchmark` example. It advances the application at a 60 Hz frame
budget and uses the configured job budgets without a GPU or GPU uploads.

| Phase | Before Sections | Before Frames / s | After Sections | After Frames / s |
|---|---:|---:|---:|---:|
| Cold | 4375 | 158 / 2.626 | 4375 | 158 / 2.626 |
| One-column move | 4375 | 8 / 0.117 | 4375 | 8 / 0.117 |
| Distant eviction | 5303 | 298 / 4.968 | 5385 | 300 / 5.001 |
| Reload | 4375 | 158 / 2.626 | 4375 | 158 / 2.626 |

The changed distant landscape needs 82 more active sections and two additional
scheduled frames, an observed increase of 0.033 s. Cold loading, one-column
movement and reload match at this measurement resolution. These are frame-paced
streaming latencies, not unthrottled generation throughput or GPU-inclusive load
times. The example constructs `GameConfig::default()` in each executable;
therefore this comparison includes the changed terrain and snow-line default,
unlike the separately frozen settings for the native river benchmark.

## Validation And Native Evidence

Validation status:

| Check | Status |
|---|---|
| `cargo fmt --all --check` | Passed |
| `cargo check --workspace` | Passed |
| `cargo test --workspace` | 443 passed; zero failed; one existing ignored soak |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo build --workspace` | Passed |
| Standalone dedicated-server build and headless start | Passed; no display variables, 27 sections ready |
| Real UDP chat, locate, permission and teleport integration | Passed within 37 network tests; included in workspace total |
| Native Tilt-UI chat and clickable result | Passed; wide/compact captures and real server teleport |
| Native green Highland, rock/soil slopes and full snow blocks | Passed; final native captures inspected |
| Native mushroom variants and taller single-column spruces | Passed; final native captures inspected |
| Native cave entrance and giant chamber | Passed; two inspected captures per scene |
| Native river transitions and forks | Pending inspection |

Passing regression coverage includes the Y80 height boundary and normalized biome
weights; one Grass voxel followed by Dirt at negative/fractional heights;
oasis/transition Grass over Dirt; large chamber dimensions; cave density at
negative cell boundaries; protected water beds, explicit passage floors and
roofs; low open fork islands; and continuous drainage-cell edges. Final command
logs are retained under `native/`: [format](native/terra-refinement-fmt.log),
[check](native/terra-refinement-check.log), [tests](native/terra-refinement-tests.log),
[Clippy](native/terra-refinement-clippy.log), and [build](native/terra-refinement-build.log).
The ignored long-running soak is not counted as a pass.

### Completed Native Checks

The standalone `terra-server` build and smoke start with display variables unset
completed successfully. Its smoke run loaded 27 sections and selected solid dry
spawn `[8,112,24]`. The standalone dependency tree was checked and contains no
hidden mandatory renderer/window/GPU dependencies. Sources:
[headless build](native/terra-refinement-headless-build.log) and
[headless start](native/terra-refinement-headless.log).

The native Tilt-UI chat run used a real integrated server over UDP. The wide
1280x800 and compact 960x640 captures were inspected: biome/coordinate text is
pink-purple, the action is yellow, and both layouts remain readable. Keyboard
input, `/locate`, Escape handling and the clickable result were exercised.
The server confirmed a real teleport from `[8.5,113.91008,24.5]` to
`[-1815.5,30.910006,-1799.5]`, generation 1. Three screenshot callbacks completed:
`chat-locate-wide.png`, `chat-locate-compact.png` and
`chat-desert-destination.png`. Source: [chat log](native/terra-refinement-chat.log);
captures: [wide](captures/chat-locate-wide.png),
[compact](captures/chat-locate-compact.png), and
[destination](captures/chat-desert-destination.png). This is evidence for the
actual client/server action, not a simulated message or locally assigned camera
position. Permission-denial and malformed-message coverage also passed in the
separate network integration tests.

The entrance and giant-chamber native runs both completed successfully and
their overview/close screenshots were inspected as nonblank production scenes.
The giant chamber uses existing lighting, not an added test lamp, and validates
a natural 75-voxel air span with solid floor at `[98,-49,-92]`. Its rendered
anchor is `[98.5,-24.5,-91.5]`; 10956 sections were active at the overview view.
The entrance capture uses the updated natural mouth near
`[-2828,35.8,-4078]`. Sources: [entrance log](native/terra-refinement-cave.log),
[giant chamber log](native/terra-refinement-giant-cave.log); captures:
[entrance overview](captures/cave-overview.png), [entrance close](captures/cave-close.png),
[chamber overview](captures/giant-cave-overview.png),
[chamber close](captures/giant-cave-close.png). These are focused visual checks, not exhaustive
manual exploration of every seed or a cave-specific FPS benchmark.

Candidate native landmarks for seed 42042:

- Highland region X/Z 512/-512: approximately Y88..129 over the surrounding area,
  spanning green foothills and the new snow line.
- Higher ridge X/Z -256/256: approximately Y112..137.
- Giant chamber center X/Y/Z 98/-25/-92; approximate ellipsoid radii 38/22.8/31.16.
- A second giant chamber center is 485/-59/101, horizontal radius 43 m.
- Updated natural cave mouths include -2828/35.7/-4078 and -2307/122.3/-4119.

Final mushroom and Highland overview/close captures were also inspected after
the reduced mushroom-relief density and detail budget were applied. These use
the new snow-line default and are feature checks, not fixed-scene FPS results.
Only the scenes explicitly marked completed above are visually validated so
far. The remaining river views require their own readiness, nonblank capture and
geometry checks. No extra cave lighting or fake world edits are used for the
completed cave screenshots.

## Reproduction

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo build -p terra-server
env -u DISPLAY -u WAYLAND_DISPLAY target/debug/terra-server --bind 127.0.0.1:0 --no-save --smoke-test
cargo run --locked -p terra-world --example terrain_regions
cargo run --locked -p terra-world --features bevy --example streaming_benchmark
TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/terrain-chat-2026-10-16/configs" TERRA_RIVER_BASELINE_VIEW=1 TERRA_BENCH_SECONDS=18 TERRA_RENDER_DIAGNOSTICS=1 TERRA_BENCH_MODE=offline TERRA_BENCH_FULL_MICRO=0 python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-river-benchmark
TERRA_CAPTURE_DIR=/tmp/terra-refinement-native target/debug/terra-client --smoke-test-chat
TERRA_BIOME_SCENE=cave TERRA_CAPTURE_DIR=/tmp/terra-refinement-native target/debug/terra-client --smoke-test-biomes
TERRA_BIOME_SCENE=giant-cave TERRA_CAPTURE_DIR=/tmp/terra-refinement-native target/debug/terra-client --smoke-test-biomes
```

Keep compilation, a second Terra client and other GPU-heavy workloads out of the
timed benchmark. Add final native commands, retained logs/captures and measured
regressions when available; pending entries must not be replaced by assumptions.
