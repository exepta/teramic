# Kinetic Reference Models, Conveyor Ports And Frame-Time Investigation

## Reference Models And Dye

The nine reusable wooden/stone/iron small gear, large gear and shaft models now
have solid segmented bodies, iron hubs, end collars, plank joints and bolts.
Small/large gears retain 8/16 teeth for the actual 2:1 mechanical ratio rather
than copying an inconsistent tooth ratio from an illustrative reference.
The original collision hulls and block IDs are unchanged. There are still only
two material groups per model, shared by all placed instances. Wood variants
continue using the existing machinery textures. Model vertex counts are 4336
(small), 6960 (large) and 1184 (shaft).

The existing conveyor dye route colors `kinetic_belt`, not wood or metal. Tests
exercise all 14 ribbon colors across all four wood variants without duplicating
meshes. Authority tests apply dye in Survival through controller, endpoint,
intermediate ribbon and guide shaft, consume one dye, and replay the journal.
The route already existed; this is verification, not a fabricated production fix.

The native kinetic gallery passed and produced six world captures plus eleven
inventory icons. Selected captures are in `captures/`. The new gear bodies and
dyed ribbon were inspected in the actual Vulkan client. This is not a claim of
pixel-identical art or an exhaustive temporal flicker test.

## Red Branch Network Failure

An inner stretched-belt ribbon previously advertised an axial shaft port. A
short reverse-geared branch could then reconnect through that fictitious port
and impose contradictory directions on a valid network. Material 148 ribbons
now transfer power only along their explicit span; real endpoint/guide shafts
and legacy individual belts retain their axial ports. Placement consults those
same span links. Actual opposed sources still correctly cause an overload.

Regressions include three water-driven sources, the short counter-rotating
Crusher branch, compatible alternate routes and a genuinely incompatible pair.
They exercise both the core graph and loaded world/journal topology. No new
metadata schema, liquid engine, block ID or save migration was introduced.

## Short Native FPS Experiments

Hardware: RX 6950 XT, Ryzen 5900X, Vulkan/RADV, X11, 1280 x 800, render distance
20, high graphics. Each sample uses the existing offline LOD out-and-back flight
for 32 seconds after loading. It crosses streaming/LOD boundaries but does not
measure integrated-server physics or multiplayer. No Cargo process ran during
these measurements. No new 10/15/30-minute soak was run.

### Allocator Comparison

These process-only `LD_PRELOAD` experiments use the same pre-probe binary and
seed 42042 / clear. No allocator dependency or machine-wide setting was changed.

| Allocator | AVG FPS | Min FPS | p95 ms | p99 ms | Frames below 32 | Final RSS KiB |
|---|---:|---:|---:|---:|---:|---:|
| System | 53.32 | 26.38 | 28.59 | 31.79 | 28 / 1706 | 2,479,568 |
| jemalloc | 54.25 | 11.65 | 27.90 | 31.43 | 18 / 1736 | 2,010,976 |
| tcmalloc | 54.09 | 25.93 | 28.43 | 32.34 | 30 / 1731 | 2,109,084 |

Rejected as a general FPS fix: no convincing frame-tail improvement, including
a much worse isolated jemalloc spike. Reduced RSS alone is not a frame-rate fix.

### Compute Worker Comparison

The smoke-only `TERRA_COMPUTE_THREADS` caps just the frame Compute pool. Actual
logged counts are 16/8/4 Compute workers with four Async and four IO workers in
each case; world-generation/IO policies are unchanged. Production retains the
stock default. Seed 42042 / clear, same binary:

| Compute workers | AVG FPS | Min FPS | p95 ms | p99 ms | Frames below 32 |
|---|---:|---:|---:|---:|---:|
| 16 | 52.50 | 27.44 | 27.99 | 31.95 | 22 / 1680 |
| 8 | 53.69 | 28.58 | 27.48 | 30.80 | 12 / 1718 |
| 4 | 49.13 | 24.59 | 31.62 | 35.73 | 88 / 1572 |

The rain / seed 7 sample with eight workers gives 58.09 AVG / 26.51 minimum,
p99 30.11 ms, 11 / 1859 frames below 32, RSS 1,963,180 KiB. Eight workers is
not proven universally better; four is clearly worse in this sample. This
does not justify changing all production pools to an arbitrary fixed size.
The probe is intended for fresh offline smoke starts. An integrated server may
initialize the global pools before DefaultPlugins, so the actual logged counts
must be checked rather than assuming a requested cap took effect.

### Render Schedule And Validation Comparison

The entire serial Render schedule experiment was slower and its experimental
code was removed. The existing Core3d view executor remains unchanged. The
indirect validation experiment changed only the public process setting
`WGPU_VALIDATION_INDIRECT_CALL=0`, not general validation or geometry.

| Variant, seed 42042 / clear | AVG FPS | Min FPS | p95 ms | p99 ms | Frames below 32 |
|---|---:|---:|---:|---:|---:|
| Parallel Render, fresh control | 52.40 | 25.97 | 29.26 | 32.97 | 38 / 1677 |
| Serial Render, discarded | 44.47 | 22.59 | 35.06 | 39.12 | 184 / 1423 |
| Indirect validation off | 52.42 | 25.30 | 29.05 | 32.53 | 29 / 1677 |

Neither experiment is shipped as an FPS optimization. Validation remains on;
there is no vendored engine or suppressed allocator error.

## Rejected Live Terrain Batch Reuse Experiment

**This experiment has been removed from the final source**, including its
modules, ownership hooks, metadata, control flag, telemetry and direct BLAKE3
dependency. The following describes discarded versions and actual measurements,
not a shipped feature or a completed FPS fix.

The temporary `meshing/reuse.rs` fingerprinted the final worker payload, after geometry,
lighting, splitting, index compression and color packing. BLAKE3 hashed the
attribute formats and bytes, material, shadow metadata and indices. The temporary
direct dependency used existing version 1.8.7 with Bevy's existing
`default-features = false`; no dependency version was upgraded.

Each live section stored only those small descriptors, template identity and
already-owned mesh handles. An identical replacement batch could retain its
handle before `prepare_batch`/`Meshes.add`. This was not a retired-LOD cache:
it held no old geometry and introduced no retained region. The worker still
composed the requested geometry.

Partial staging and publication revalidated ownership. Stale jobs did not
delete borrowed assets, and unloading still removed owned meshes. Water,
wind/growth batches and the opt-in far-batching experiment were excluded.
Replacement entities recomputed material bindings and shadow-caster flags.

The discarded tests covered repeated LOD swaps with one live asset, changed attributes/index
formats, stale/canceled jobs, unloading, real GPU-only extraction without
reading old CPU vertex buffers, and shadow Off/On descriptor invalidation.
`TERRA_REUSE_TERRAIN_BATCHES=0` was the smoke-only same-binary old-path control.
Reported `reused_terrain_bytes` were avoided CPU batch payload bytes, not an
exact PCIe-transfer or VRAM counter.

### Initial Reuse And Vulkan Block Experiments

The first, unrestricted hashing version avoided thousands of tiny allocations
but did **not** improve FPS. These results precede the final 64-KiB candidate
limit and authored-cube visibility correction and are not final performance
results. Controls and unrestricted reuse use the same binary and 16 workers.

| Seed / weather / variant | AVG FPS | Min FPS | p95 ms | p99 ms | Frames below 32 | RSS KiB |
|---|---:|---:|---:|---:|---:|---:|
| 42042 / clear / reuse off | 54.85 | 26.42 | 27.43 | 30.84 | 13 / 1755 | 2,441,948 |
| 42042 / clear / unrestricted reuse | 53.95 | 26.36 | 28.33 | 31.70 | 21 / 1726 | 2,567,576 |
| 7 / rain / reuse off | 60.47 | 30.51 | 25.73 | 28.23 | 2 / 1935 | 2,027,184 |
| 7 / rain / unrestricted reuse | 59.84 | 28.00 | 26.28 | 28.93 | 1 / 1915 | 1,975,320 |
| 42042 / clear / reuse off, manual Vulkan block hints | 53.65 | 26.91 | 28.19 | 30.97 | 11 / 1717 | 2,561,896 |

Clear unrestricted reuse records 16,221 hits / 124,898,668 avoided CPU payload
bytes; rain records 19,409 / 130,991,832. The clear hashing scope costs 1.422 s
of aggregate worker wall time within the sample, rain 0.347 s. That is not
main-thread frame time or a guaranteed GPU transfer saving. The first version
hashed some large/ineligible batches that the upload path later rejected.
The refined version rejected batches above 64 KiB in O(1) and excluded water,
foliage, flame and fibre materials before hashing. Its clear / 42042 sample
measured 53.51 AVG / 28.75 minimum FPS, p95 28.89 ms, p99 31.82 ms,
26 / 1712 frames below 32 and RSS 2,367,128 KiB. It avoided 97,806,968 payload
bytes and spent 0.314 s hashing during the sample. Even bounded hashing did
not produce a convincing overall FPS gain, so the entire experiment was removed.

The separate smoke-only `TERRA_WGPU_MEMORY_BLOCK_MIB=256` uses the public
MemoryHints API, requests 256..512 MiB device blocks and leaves mesh slab sizes
unchanged. Vulkan reports 402,653,184 startup allocator reserved bytes. This is
not total VRAM and shows no convincing FPS gain, so production keeps stock hints.

## Portal Visibility Corrections

The visibility cache now recognizes authored, fully opaque face-cube models,
including the new grass block. This requires the loader's existing proof of
six canonical opaque faces and an opaque coarse cube; partial models, glass,
leaves and unknown templates are not incorrectly treated as solid walls.

More importantly, an open face mask alone did not say whether different open
faces belonged to the same air pocket. The bounded section flood now records
face-to-face connectivity. The region traversal follows actual entry/exit
ports and permits re-entry through a second reachable face, preserving curved
entrances and looping passages without connecting unrelated cavities.

Crossing a section previously cleared hidden terrain even when the new focus
already lay in the same fully traversed region. Cached reachability is now
reused only when all 27 focus seeds and all their open ports were actually
reached, with unchanged graph/opening epochs. Unknown cells, topology edits,
unloading and warps still invalidate conservatively. The near field, boundary
seeds and complete wall/shadow ring remain intact.

Boundary traversal also previously activated every air pocket of a boundary
section, even when its unknown-facing wall was closed. It now starts only
through the connectivity of genuinely unknown-facing ports. Closed boundary
walls remain visible roots with their complete shadow ring; this is not removal
of the conservative boundary seeds. Two additional regressions cover separated
pockets, bent real entrances, multiple unknown faces and unchanged focus seeds.

The opacity-only preliminary A/B was 53.03 / 21.53 AVG/minimum versus
54.65 / 28.84, but both retained exactly the same eight hidden sections and
6,361 rendered sections. That is not sufficient evidence of a structural FPS
gain, and is not presented as one. The native cave-portal lighting fixture
passed four camera stages after that opacity correction. The complete
connectivity/focus correction is validated separately below when measured.

Before the boundary-port refinement, a fresh same-binary all-legacy control was
53.23 AVG / 26.93 minimum FPS, p95 28.64 ms, p99 32.26 ms,
28 / 1703 frames below 32. Component connectivity and focus reuse measured
53.93 / 29.22, p95 26.80 ms, p99 29.92 ms, 7 / 1726 frames below 32.
Both still retained eight hidden and 6,361 rendered sections. Improved tails
in that one pair do not establish an overall FPS fix, and do not meet the
requested minimum of 32 FPS.

Smoke-only legacy controls allow the final same-binary comparison without
changing normal gameplay: `TERRA_LEGACY_PORTAL_OPACITY=1`,
`TERRA_LEGACY_PORTAL_CONNECTIVITY=1`, `TERRA_LEGACY_PORTAL_FOCUS=1`.

## Workspace Validation

The source passed `cargo fmt --all --check`,
`cargo check --locked --workspace --all-targets -j1` and
`cargo clippy --locked --workspace --all-targets -j1 -- -D warnings`.
The final `cargo build --locked --workspace -j1` also passed for client and
dedicated server. The kinetic asset generator passed `node --check`.
The complete `cargo test --locked --workspace --no-fail-fast -j1 --
--test-threads=1` run passed 3,051 tests, with seven explicitly ignored tests.
These include the authored opacity, disconnected air pockets, legitimate
curved entrances, second-face traversal, focus reuse, stale results, negative
coordinates and graph invalidation regressions.

The preceding two-test-thread run hit the existing five-second GLTF-loading
deadline in two kinetic fixture tests. It was not counted as a pass. The
subsequent complete serial run passed without changing the assets or extending
that deadline; both logs are retained.

The first full run after the material change also exposed two new test-harness
mistakes: the stock prepass fragment uses a shader handle, not `ShaderRef::Default`,
and a bare WESL virtual resolver needs its explicit `constants` module. The
regressions now compare all shader-reference variants against the actual
ExtendedMaterial contract and provide the binding-group constant to their
isolated compiler. No production shader or material change was needed for
these two corrections; both the failed and successful full-suite logs remain.

## Exact Static LOD Shadow Batches

Near terrain keeps its existing shadow shells. For other detail levels, complete
compatible opaque batches now share exact depth-only geometry per section.
Positions and indices are copied without simplifying, recessing or reconstructing
them. Wind, growth, water, glass, masked foliage, flames and special ore materials
stay unchanged. A material group is eligible only if every batch of its slot is
compatible, because the existing upload contract replaces casters by material
slot. Normal vertex/byte splitting still applies, and a proxy is only published
when it reduces the number of casters. Section bounds, atomic publication,
Shadow Off/On and unloading retain their existing ownership rules.

This is enabled normally; `TERRA_EXACT_LOD_SHADOWS=0` is a smoke-only old-path
control. The existing full-shadow reference also disables it. Six regressions
cover geometry equivalence, complete-group eligibility, splitting, material
proof, publication and resource lifetime. Native cave-portal inspection passed
the four actual camera stages 18..21, with bright exterior terrain, sheltered
torch lighting and underwater exposure. The fixture's final stage counter of
22 is not a claim that 22 views ran in this inspection mode.

Same-binary short flights, before the subsequent Wet binding / batching fixes:

| Seed / weather / exact shadows | AVG FPS | Min FPS | p95 ms | p99 ms | Frames below 32 | RSS KiB | Mesh assets |
|---|---:|---:|---:|---:|---:|---:|---:|
| 42042 / clear / off | 53.10 | 26.94 | 28.31 | 32.18 | 24 / 1699 | 2,515,556 | 30,117 |
| 42042 / clear / on | 56.35 | 30.23 | 26.73 | 29.85 | 4 / 1803 | 2,418,704 | 34,705 |
| 7 / rain / off | 59.50 | 29.20 | 26.44 | 29.03 | 4 / 1904 | 1,972,372 | 18,650 |
| 7 / rain / on | 61.50 | 29.07 | 25.42 | 27.76 | 1 / 1968 | 1,970,204 | 21,280 |

The extra worker copy costs 0.0098 ms per call in clear and 0.0079 ms in rain.
This improvement is modest, adds bounded owned proxy meshes, and does not
establish a minimum of 32 FPS. In particular, the rain minimum did not improve.

## Chunk Animation Isolation And Material Fix

A diagnostic-only config changes just `chunk_animation` from `rise` to `none`.
It is archived as `configs/no-animation-diagnostic`; this is not the production
solution or a proposed quality reduction. Clear / 42042 with that config gives
68.93 AVG / 33.93 minimum, p95 22.33 ms, p99 25.60 ms, zero / 2206 frames below
32 and 13.25 s initial load. The same binary with Rise and exact shadows off
gives 53.10 / 26.94 and 30.29 s initial load. This isolates the cost of the
animation path as a whole, not exclusively its material allocations.

The old WetExtension was non-bindless. During alpha fades its modified
non-bindless resources went through Bevy's free/allocate and new uniform/bind
group preparation rather than the bindless POD update path. This is an actual
code-path finding, but the previously measured mesh allocator and mesh
preprocessing-bind-group scopes must not be mislabelled as material timings.

Wet materials now use the public bindless index-table and compact-data contract:
the stock StandardMaterial table remains on binding 0, the extension table is
on 100 for indices 101..103, weather buffers use 101 and 16-byte parameter data
uses 102. A small owned material wrapper delegates the original shader, alpha,
renderer, bias, transmission and specialization contract. It forces both parts
to fall back together when either lacks device support. The public descriptor
provides a correctly sized 1120-byte weather dummy for unused buffer-array
entries. The weather buffer handle and GPU allocation remain stable. There
are no engine, registry or vendor changes. `TERRA_LEGACY_WET_BINDINGS=1` is the
smoke-only same-binary old-path control.

The transition's temporary NoAutomaticBatching override is also released after
that specific child has its confirmed first draw. Unready siblings stay isolated;
outgoing fades do not need this one-shot readiness opt-out. Existing caller-owned
opt-outs are never removed. Opacity and motion formulas are unchanged.
`TERRA_LEGACY_FADE_BATCHING=1` restores the old smoke path independently.

### Final Combined Native Comparison

These are fresh same-binary controls after all changes, with Rise animation
enabled in every row, unchanged high graphics, render distance 20 and 1280 x 800.
The old control sets the three legacy portal flags, `TERRA_EXACT_LOD_SHADOWS=0`,
`TERRA_LEGACY_WET_BINDINGS=1` and `TERRA_LEGACY_FADE_BATCHING=1`. The new path
sets none of them. Both controls and new paths take an initial screenshot;
screenshot-adjacent frames are excluded. The startup logs confirm actual
non-bindless control and bindless new rendering on the RX 6950 XT.

| Seed / weather / path | AVG FPS | Min FPS | p95 ms | p99 ms | Frames below 32 | RSS KiB |
|---|---:|---:|---:|---:|---:|---:|
| 42042 / clear / old control | 53.15 | 21.15 | 27.72 | 31.77 | 24 / 1690 | 2,459,816 |
| 42042 / clear / shipped fixes | 73.47 | 27.25 | 21.76 | 24.59 | 2 / 2341 | 2,476,352 |
| 7 / rain / old control | 61.49 | 27.99 | 25.35 | 27.73 | 4 / 1958 | 1,958,800 |
| 7 / rain / shipped fixes | 86.28 | 33.55 | 19.19 | 21.49 | 0 / 2751 | 2,038,720 |

Average FPS improves by 38.2% / 40.3% in these two samples, and p99 frame time
falls by 22.6% / 22.5%. Loaded sections and represented props are identical
within each pair: 12,157 / 164,430 for clear, 15,831 / 27,522 for rain. The
extra exact proxy meshes are bounded by those sections, not a growing cache.
There is no reduction in rendering distance, vegetation density, water quality,
shadow quality or animation. Initial load remains approximately 30 s / 24 s;
the diagnostic no-animation 13.25 s loading result is **not** the shipped result.

Two extra clear flights isolate the changes without changing that geometry:

| Independent old-path control, other fixes enabled | AVG FPS | Min FPS | p95 ms | p99 ms | Frames below 32 |
|---|---:|---:|---:|---:|---:|
| Old Wet bindings only | 56.97 | 30.47 | 25.40 | 28.17 | 2 / 1823 |
| Old fade batching only | 73.54 | 31.01 | 21.73 | 24.55 | 1 / 2353 |

This identifies the Wet binding change as the dominant measured gain. Earlier
batching release is a one-shot ownership/lifecycle correction, not a separately
proven average-FPS improvement. Individual worst frames still fluctuate; the
clear all-new sample has two frames below 32, so neither a universal 32-FPS floor
nor completion of A05 is claimed.

The final native kinetic gallery passed again with actual bindless rendering
(six world views and eleven icons). Clear/rain old/new captures were visually
compared. The final native cave inspection also passed the four camera stages
18..21, including exterior sunlight, enclosed torch lighting and underwater
exposure. No shader validation errors or allocator errors appeared in these
completed native runs. Offline flights do not validate authoritative-server
physics, multiplayer or long-term behavior. No new long soak was run.

## Reproduction

```bash
config_dir="$(mktemp -d /tmp/teramic155-replay-config.XXXXXX)"
cp tools/benchmarks/results/kinetic-reference-cpu-2026-10-29/configs/clear/{client,graphics,world}.toml "$config_dir/"
env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET WINIT_X11_SCALE_FACTOR=1 \
  TERRA_CONFIG_DIR="$config_dir" \
  TERRA_WORLD_DIR=/tmp/teramic155-replay-world \
  TERRA_BENCH_GRAPHICS=1 TERRA_BENCH_SECONDS=32 TERRA_BENCH_LOAD_SECONDS=120 \
  TERRA_CPU_SCOPES=1 TERRA_COMPUTE_THREADS=16 WGPU_BACKEND=vulkan \
  target/debug/terra-client --smoke-test-lod
```

For the second seed, copy `configs/rain7` instead and set `TERRA_BENCH_WEATHER=rain`.
Only the three TOML inputs are archived; generated map caches and profiles stay
outside this report directory.
For the gallery use `TERRA_CAPTURE_DIR=/tmp/teramic155-kinetic-art` and
`--smoke-test-kinetics` instead. The archived short measurements are observations,
not a guaranteed minimum frame rate or a completed long-term acceptance test.
