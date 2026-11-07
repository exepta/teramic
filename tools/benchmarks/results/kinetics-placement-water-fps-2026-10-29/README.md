# Kinetic Placement, Gearboxes, Water Sources And Streaming

## Implemented Changes

- Mechanical placement uses the clicked face and its adjacent cell. An axial
  port determines orientation before other neighbors; a side click does not
  teleport the new part to a different port. Preview and authority share this
  rule. Tunnel/Funnel can stand alone; inventory attachment is optional and is
  established only when an inventory is actually clicked. Their short feet can
  enter the supporting full floor, without allowing their body through walls.
- Joined windows no longer collide with their own validated group during
  placement. Door clearance uses the proposed primary pose for both halves.
  Unrelated extended machinery still blocks intersecting openings.
  Cherry doors/windows also use the shared opening classification when placing
  and recording their object state; the missing classification caused them to
  lose their render/interaction metadata.
- Straight belts support all three shaft-perpendicular planes, including the
  diagonal arrangement with vertical shafts shown in the reference. Collinear
  intermediate shafts remain real shafts. A free belt cell is their controller;
  removal, dye, save/reload and section indexing preserve the guide shafts.
- Water-wheel base capacity is 128 instead of 32. River/fall/ocean factors remain
  1.0/1.2/1.1, rising to 1.2/1.35/1.4 at full storm. Still lakes and dry wheels
  provide no power. Gearing still conserves power and can overload a network.
- Horizontal gearbox 154 exposes four horizontal ports. Vertical gearbox 155
  exposes two horizontal and two vertical ports. Both cost 2 and transfer at
  1:1; four separately animated ends match the solver's signed port rotations.
- Revised Tunnel, Funnel, belt and gearbox models use actual plank joints,
  metal bands and rivets. Three new kinetic-only bitmap textures are documented
  in `assets/terra/blocks/textures/machinery-materials.md`. Shared rotor meshes
  and belt UV animation avoid per-frame asset generation.
- TMI has a Technic category for machinery, ports, gearboxes, shafts/gears,
  conveyors, Wrench and Bellows, including wooden variants. Explicit custom
  category overrides retain precedence.
- Removing a block above a lake no longer creates a simulated dry tombstone
  that hides the intact source underneath. Old erroneous air tombstones outside
  a natural reservoir are ignored by surface masking; real bucket withdrawals
  inside the reservoir remain dry and persistent.
- New worlds use river revision 3: regional tributaries also work with the
  normal landscape revision, and brooks have additional humid plains/forest
  habitats. Existing drainage, catchment limits and bounded caches are reused.
  Previously saved river revisions 0, 1 and 2 are unchanged.

Existing block IDs and save field layouts remain unchanged. IDs 154/155 are
appended. Client and server use protocol 83 together.

### Watercourse Density

The deterministic centerline test counts only open, actually wet channel length
above ocean level within the same 4096 x 4096 m window, not bank influence or
ocean water. The production generator is queried for both revisions.

| Seed | Revision 2 wet length | Revision 3 wet length | Brooks before/after | Tributaries before/after |
|---|---:|---:|---:|---:|
| 42042 | 13,849.86 m | 15,949.65 m | 31 / 36 | 0 / 4 |
| 7 | 1,537.85 m | 2,325.82 m | 8 / 8 | 0 / 4 |
| 99173 | 2,839.44 m | 3,857.46 m | 5 / 9 | 0 / 0 |

Combined length increases from 18,227.15 to 22,132.93 m: **21.43%**. These are
three reproducible samples, not a claim that every seed gains the same amount.
The original catchment counts and bounded generation remain unchanged.

## Mesh Upload Changes

Wind data previously caused the worker to skip normal/tangent packing even
though the existing forward/prepass vertex shaders accept normalized inputs.
The worker now packs those frames too, preserving positions, UVs, HDR colors,
wind data and tangent handedness. When both frames are present this saves
20 bytes per vertex. It does not imply every material saves that amount.

Terrain tints that still need floating-point range now use Bevy's built-in
Float16 color quantization instead of Float32x4: 8 rather than 16 color bytes
per vertex. HDR RGB, sign and semantic alpha are retained; no Unorm clamping
or shader rewrite is used. Nonfinite/out-of-range batches and already packed
attributes are left unchanged. Tests cover actual snow/evergreen temperature
tags and their decoded error, plus unchanged other geometry attributes.
Quantization runs after splitting in the existing worker, not in the main-thread
upload. The uploader moves the completed Half buffer without copying it. Byte
budgets and subsequent splits account for and preserve the actual Half payload.

These changes do not reduce render distance, geometry detail, vegetation,
textures or shadows. Smoke-only `TERRA_PACKED_ROOTED=0 TERRA_HALF_TINTS=0`
provide an old-layout control in the same binary. The optional mesh telemetry
now records the largest mesh's attribute formats, not just its total bytes.

The prop worker previously inherited `PROP_GROWTH` data section-wide whenever
one flower was present. Even independent static pieces after the final split
could carry 8-byte timing and 12-byte growth anchors per vertex. Those two
buffers are now released only for un-staged pieces with no growth ranges,
complete finite geometry, exact neutral timing `[0, 1]` and a single fully opaque
color stream. Real flowers, crops and regrowing grass keep their animation data.
Wind still uses unchanged UV0, and no batches, vertices, indices or entities are
added or removed. An eligible packed prop mesh needs 28 instead of 48 vertex
bytes. `TERRA_STATIC_PROP_LAYOUT=0` restores the old layout in smoke tests.

### Rejected Cache Experiment

A bounded 96 MiB / 1024-entry LOD cache was implemented, compiled and
measured, then removed rather than shipped as an unsupported FPS fix. The
first version retained active variants and had no hits. Retired-only admission
produced only 57 hits against roughly 16,800 compositions in the control flight.
Neither improved FPS convincingly; the retired-only version also retained more
RAM and mesh assets. The final source has no added LOD cache, cache ownership
layer, cache settings or cache resources.

| Attempt | Average FPS | Minimum FPS | p99 | RSS |
|---|---:|---:|---:|---:|
| Original baseline | 51.08 | 26.49 | 33.80 ms | 2,498,812 KiB |
| Active-variant cache, discarded | 51.98 | 26.82 | 32.26 ms | 2,470,524 KiB |
| Retired-only cache, discarded | 50.74 | 24.62 | 33.70 ms | 2,547,144 KiB |

These historical attempt values are not results for the final upload changes.

## Reproducible Short Measurements

Baseline: native RX 6950 XT, Vulkan/RADV, X11, 1280 x 800, render distance 20,
seed 42042, landscape revision 1 and river revision 1. The historical graphics
and world configuration are copied from `streaming-river-villager-2026-10-28`.
The offline LOD fixture flies out and back 224 m for a 32-second sampling window.
It is camera-only: integrated-server physics, input latency and multiplayer are
not measured by this fixture. No Cargo process runs during native measurements.

```bash
env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET WINIT_X11_SCALE_FACTOR=1 \
  TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/kinetics-placement-water-fps-2026-10-29/configs/clear" \
  TERRA_WORLD_DIR=/tmp/teramic154-replay-world \
  TERRA_BENCH_GRAPHICS=1 TERRA_BENCH_SECONDS=32 TERRA_BENCH_LOAD_SECONDS=120 \
  TERRA_CPU_SCOPES=1 WGPU_BACKEND=vulkan \
  target/debug/terra-client --smoke-test-lod
```

For the rain sample use `configs/rain7` and `TERRA_BENCH_WEATHER=rain`.
The old-payload control sets `TERRA_PACKED_ROOTED=0 TERRA_HALF_TINTS=0`;
the final comparison also disables the static-prop cleanup with
`TERRA_STATIC_PROP_LAYOUT=0`. All three controls are smoke-only.

The following is the final binary, with all three smoke controls off/on. The
historical baseline above is not substituted for the same-binary control.

| Measurement, clear seed 42042 | Old-payload control | Final |
|---|---:|---:|
| Average FPS | 52.40 | 50.81 |
| Minimum FPS | 27.43 | 27.73 |
| p95 frame time | 29.06 ms | 29.50 ms |
| p99 frame time | 32.59 ms | 32.84 ms |
| Frames below 32 FPS | 36 / 1677 | 41 / 1626 |
| Initial load | 30.45 s | 30.55 s |
| Final process RSS | 2,470,856 KiB | 2,605,768 KiB |

| Measurement, rain seed 7 | Old-payload control | Final |
|---|---:|---:|
| Average FPS | 58.17 | 59.78 |
| Minimum FPS | 28.82 | 30.89 |
| p95 frame time | 26.94 ms | 25.63 ms |
| p99 frame time | 29.66 ms | 28.28 ms |
| Frames below 32 FPS | 7 / 1861 | 2 / 1913 |
| Initial load | 24.74 s | 24.66 s |
| Final process RSS | 2,044,380 KiB | 1,991,208 KiB |

These values are actual short-run observations, not a minimum-FPS guarantee or
a long-term acceptance result. No 10/15/30-minute soak was requested or run.
The clear scene does not show a general FPS gain; the rain scene improves in
this sample, but neither clears the 32-FPS floor. The reported extreme FPS
problem therefore remains open. Smaller individual GPU payloads do not imply
lower total process RSS, as these actual measurements demonstrate.

### Intermediate Upload Measurements

These are experiments before the final static-prop layout cleanup. All are
32-second samples at identical settings; no Cargo work runs concurrently.

| Variant / seed / weather | AVG FPS | Min FPS | p95 | p99 | Frames below 32 |
|---|---:|---:|---:|---:|---:|
| Main-thread Half / 42042 / clear | 51.03 | 26.70 | 29.41 ms | 31.81 ms | 28 / 1633 |
| Same-binary control / 42042 / clear | 51.60 | 23.64 | 29.71 ms | 34.22 ms | 44 / 1651 |
| Worker Half / 42042 / clear | 50.68 | 24.81 | 30.28 ms | 33.98 ms | 56 / 1621 |
| Worker-binary control / 42042 / clear | 51.46 | 27.42 | 28.98 ms | 32.41 ms | 29 / 1647 |
| Main-thread Half / 7 / rain | 58.93 | 30.57 | 26.17 ms | 28.72 ms | 2 / 1885 |
| Rain control / 7 / rain | 58.62 | 31.25 | 26.78 ms | 29.42 ms | 3 / 1876 |
| Worker Half / 7 / rain | 58.61 | 32.42 | 26.21 ms | 28.87 ms | 0 / 1875 |

Moving the color conversion out of the main thread lowers `terrain.upload`
from 0.965 to 0.571 ms/frame in the clear fixture. The corresponding old-layout
controls are 0.499 and 0.517 ms/frame. The final format reduces a real 64,000-
vertex terrain mesh from 3,328,000 to 2,816,000 vertex bytes, but these samples
do **not** establish a general AVG/minimum-FPS improvement. Resource packing is
not being represented as a completed fix for the reported FPS collapse.

### Separate GPU Diagnostic Run

An additional identical flight enabled the existing
`TERRA_RENDER_DIAGNOSTICS=1 TERRA_GPU_DRAW_DIAGNOSTICS=1`. These instrumented
FPS are not used in the A/B tables. There are 26 GPU timing series; selected
individual pass measurements are:

| Pass | Mean | p99 |
|---|---:|---:|
| Main opaque | 1.917 ms | 2.830 ms |
| Early prepass | 1.076 ms | 1.762 ms |
| Uniform allocation | 0.767 ms | 1.315 ms |
| Irradiance lightprobe | 0.602 ms | 0.785 ms |
| Bin unpacking | 0.470 ms | 0.780 ms |

Opaque and early prepass respectively average 10.119 and 10.004 million
vertex-shader invocations. These are repeated invocations, not unique scene
triangles. The last complete, delayed GPU draw snapshot (frame 4321, no unread
submissions) contains 5599 opaque and 851 masked color subdraws, the same counts
in prepass, and 23,540 shadow subdraws. This is one snapshot, not a window mean.

CPU wall scopes in the same diagnostic run average 3.769 ms for the stock mesh
allocator, 1.789 ms for preprocess bind groups and 1.066 ms for shadow-bin
uploads; terrain upload is 0.523 ms. Scopes can overlap and include waits.
The remaining investigation should distinguish actual Render-thread allocator/
submission work from waits. Neither entity counts nor these individual GPU
passes establish a total GPU frame time.

The reported shadow timing is **not** a reliable total: Bevy's early and late
per-view shadow systems share a diagnostic path, and this reporter sees only
the latest same-time measurement. The small reported value therefore cannot
be used to declare 23,540 shadow subdraws cheap. No renderer patch or invented
combined shadow/GPU timing was used to fill that measurement gap.

### Userspace CPU Sampling

The existing `target/sample-cpu` successfully attached to a separate native
40-second fixture with CPU scopes/GPU query diagnostics disabled. Its actual
recorded interval is **18.010 s**, ending when the client exited: 12,264 samples,
zero lost, 94 discovered threads. The existing symbolizer resolves 989 samples
with callers; this optimized build therefore has limited caller-stack coverage.
Raw mappings/samples/thread names and the text summary are retained in `logs`.

| Sampled thread group | Samples | Share of sampled userspace CPU |
|---|---:|---:|
| Compute task pool | 8101 | 66.06% |
| Async compute pool | 2276 | 18.56% |
| Render thread | 1238 | 10.09% |
| Main client thread | 638 | 5.20% |

`AssetChangeCheck<Mesh3d>::has_changed` is the largest resolved enclosing
function, with 798 samples (6.51% of all sampled CPU, 9.85% of the compute pool).
Frustum intersection has 331 samples overall. The async pool includes Perlin
sampling and `terra_render::meshing::lighting::sample`; Render-thread samples
include allocator/free-list lookups and libc freeing. These measurements locate
real work but are not frame-time percentages, a proof of one causal stall, or
permission to replace Bevy internals. No vendor/framework patch was added.

GPU indirect/frustum culling already runs, with camera occlusion culling at
view distance 20. The CPU frustum filter remains an additional prefilter.
Changing that with `NoCpuCulling` needs explicit capability/view fallback and
does not remove material specialization's `AssetChanged<Mesh3d>` checks. The
existing isolated experiment in `grass-palette-streaming-2026-10-27` was slower,
so no unvalidated default flip was made here.

The sampled 40-second fixture is diagnostic, not an A/B gain result: it has a
different flight speed and no scope overhead. Its reported 55.07 AVG / 29.75
minimum FPS must not be substituted into the 32-second comparison tables.

## Validation

- `cargo fmt --all --check`: passed.
- `cargo check --locked --workspace --all-targets -j1`: passed.
- `cargo test --locked --workspace --no-fail-fast -j1 -- --test-threads=2`:
  **3010 passed, 0 failed, 7 existing ignored**.
- `cargo clippy --locked --workspace --all-targets -j1 -- -D warnings`: passed.
- `cargo build --locked --workspace -j1`: passed, including client and headless
  dedicated server. No dependency upgrades were made for this change.
- Native kinetic gallery: 41 cached shared mechanical meshes, 6 scene captures
  and 11 actual rendered inventory icons passed. Models/connections were
  inspected from multiple angles. Captures precede only the later worker-side
  resource-packing changes; kinetic art/solver source was unchanged afterwards.
- Native joined-window gallery: Oak/Birch/Spruce, closed/open and group splitting
  passed; screenshots inspected. Cherry and all other opening species are
  additionally covered by authoritative placement/toggle/break/journal tests.
- Native terrain overview passed and was visually inspected. Final clear/rain
  flights additionally exercised Half vertex layouts and static prop variants.
- Final native flights/galleries produced no logged warning, panic or error.
  These fixture tests do not replace manual interaction in an existing save.

Regression coverage includes standalone ports/floor clearance, clicked-axis
placement, horizontal/vertical gearboxes, actual flowing-water wheel to a 2:1
Crusher, preserved guides/controllers across save/unload, legacy item-carrying
belts, all opening species at negative boundaries, lake block-removal versus
bucket withdrawal across save/load, river density, HDR/temperature precision,
Half-payload splitting and unchanged zero-copy upload, plus real flower/crop/
regrowing-grass shader inputs after static-prop cleanup.

## Limits

- Belts are straight spans, not curved paths or arbitrary three-axis diagonals.
  Up to 32 intermediate cells and at least one free controller cell are required.
  A standing XZ ribbon on vertical shafts transmits rotation but carries no items.
- New river placement applies to new revision-3 terrain, not silent retrogen.
- A standalone port is allowed but does not automatically acquire a later
  adjacent inventory; place it against the intended inventory for attachment.
- Art follows the supplied structural reference, without claiming an exact copy.
- A short native control flight cannot certify that every gameplay scene stays
  above 32 FPS. Any remaining low frames must remain visible in the results.
