# Rivers, Water Environment And Movement - 15.10.2026

## Compatibility

No dependency upgrades, changed block IDs, new prop registry or new save system.
World format 1 and generator version 5 remain. A saved `WorldConfig.rivers` flag
enables the new terrain feature; missing flags deserialize as false, preserving
existing terrain and edited worlds. Create World enables rivers for new worlds.
The generic storage library preserves its supplied template, and never changes
old saves on load. A fresh dedicated world can enable `rivers = true` before its
first save. Existing dedicated manifests still reject mismatched configurations.

Protocol **5** carries the additional world configuration and matching movement
semantics. Rebuild/restart client and server together. Integrated Server and
Dedicated Server use the same generator and controller.

## Rivers And Waterfalls

The existing heightfield/spalten generator now optionally carves deterministic
meandering channels. Seeded 512 m catchments contain 384 m reaches with rounded
sources, end pools and sea-level outlets where applicable. A sampled running
minimum gives each reach a non-increasing water profile; discrete height drops
produce waterfalls. Riparian banks transition back to the existing terrain.
Biome surface materials, vegetation clearance and tree support use local water
levels, rather than treating every river as ocean level.

Immutable path data is shared across generator clones/jobs in a bounded 256-entry
cache. It contains procedural data only, never player edits. World-generation
jobs still cache horizontal columns across vertical sections. No generation or
large meshing operation is added to the rendering main thread.

The existing WaterPatch pipeline gains per-vertex normals/flow and supports
multiple water heights. Flat surfaces and falling sheets share one material and
one mesh per wet section, not entities for individual fluid cells. Falling sheets
are owned by the lower cell and clipped to section height, including negative
coordinates and vertical boundaries. The old sea-only path retains its shared
17x17 grid. Uploads remain bounded at four wet patches per frame; dry sections do
not allocate water meshes or render records.

The existing WGSL water shader handles horizontal current animation and vertical
fall normals, moving foam and refraction. Shared exact surface edges plus a small
material depth bias avoid opening geometric gaps to offset falling sheets.
Existing depth absorption, SSR/sky fallback and Low/Medium/High quality remain.
No additional water rendering pass is introduced.

Boundaries: static procedural streams, not a dynamic fluid/erosion solver. Digging
or building does not recompute whole drainage basins, redirect rivers or flood
mined caves. No waterfall force, soundscape or particle spray is claimed.

## Underwater Environment

The camera samples the same local water columns as swimming, checking loaded air
and the bed. Water-line hysteresis prevents rapid toggling. A short temporal blend
adjusts the existing DistanceFog, built-in ColorGrading and camera AmbientLight.
Visibility decreases with depth (roughly 9..24 m), the scene becomes cooler and
ambient light darkens. Above-water fog/color/light return on exit. This reuses
existing PBR/tonemapping rather than adding a full-screen UI overlay or custom
postprocessing renderer. Stable states do not rewrite light/fog settings each
frame; legacy worlds above sea level avoid unnecessary bed sampling.

## Movement Fixes

Walk/sprint are 5.4/8.2 m/s. The shared character controller uses stronger effective
gravity and a 7.8 m/s jump impulse for a roughly 0.65 s arc, retaining about one
block of jump height. Other Avian dynamic bodies keep their existing 9.81 gravity.

Inputs are read before the fixed loop, eliminating the previous extra render-frame
delay. CharacterPresentation interpolates the local camera between fixed poses;
physics remains fixed-timestep. Small reconciliation errors are smoothed only for
display (bounded 0.6 m); large corrections/teleports snap. Authoritative body
position, collision, command reach and world streaming are not smoothed away.
A 144 Hz render / 60 Hz physics regression test checks actual per-frame motion.

Swimming uses local river/ocean water height and capsule immersion, not only sea
level and body-center height. Forward swimming follows look pitch; Fly Down
(default Shift) explicitly dives, held Jump ascends, buoyancy returns an idle
swimmer to the surface. Held swim input survives multiple fixed steps and server
ticks without a fresh packet. A brief surface boost and water-enabled step checks
allow leaving onto a one-meter bank. Ceiling, tall-wall and missing-collider
checks remain. No underwater camera teleport or unrestricted flight permission.

The shore-exit fixture uses journal-consistent collider epochs. The earlier low-
ceiling fixture was also corrected: it now proves forward movement up to the
obstacle instead of accidentally passing while waiting for a stale collider.

## Props Are World Objects

Vegetation Density is removed from TiltUI Graphics and its command handler.
Legacy graphics/client TOML fields remain readable, but are ignored by rendering/
picking and omitted on the next save. All non-removed logical props remain
represented at every existing prop LOD. Client/server targeting cannot skip a
resource prop through a density request. Existing IDs, individual removal,
journal persistence, batching and shared materials are unchanged. No drops or
inventory are introduced by this change.

## Validation

All five workspace commands pass: fmt --check, check, **240 regular tests**,
Clippy --all-targets with -D warnings, and build. The existing 180-second soak is
still explicitly ignored by the regular suite; it was not rerun for this change.
The separate server build and display-free startup pass. Its feature tree has no
bevy_render, bevy_winit, wgpu or TiltUI.

New/extended tests cover deterministic downhill paths/default-seed waterfalls,
negative catchments, valid outward water geometry and vertical section clipping,
short jump duration, actual bank exit, swimming/descent, interpolated camera motion,
and full prop representation/picking even with a legacy zero-density setting.
The existing two-client replication, rate/authority, collision, save and seam
tests also pass. Legacy worlds without rivers retain their generation path.

Native Vulkan acceptance fixtures passed:

- `--smoke-test-rivers`: generated fall near (62.5, 19.8, 24.5), upper reach 22.8;
  river/waterfall rendering, underwater fog, surfacing and environment restoration.
- `--smoke-test-physics`: falling, landing, 1.203 m jump, wall collision, blocked
  menu input and support mining, using production inputs/controller/colliders.
- `--smoke-test-menu-session`: production TiltUI, world/server CRUD, controls,
  integrated authority, movement/edit, save, host restart and journal reload.
  The created manifest explicitly contains `rivers = true`.

Native river/waterfall, underwater/surface and Graphics captures were visually
inspected. Shader validation uses actual Vulkan compilation/rendering, not a
claim based on cargo check alone. Hardware: RX 6950 XT, RADV Mesa 26.2.4-arch3.1,
Wayland. No cross-machine or subjective play-feel approval is inferred.

## Performance

`tools/benchmarks/run-hydrology-suite.sh` performs isolated sequential 18-second
samples at 1920x1200 physical, VSync off, benchmark background throttling disabled,
radius 16/3, full logical prop representation, existing 32 m micro detail and
96/160 m terrain LOD. No parallel builds/clients or benchmark screenshots.
Raw logs include camera, counts, controller timings, individual GPU passes,
process CPU/RSS and initial ready time. Different scenes are not speedup pairs.

| Final scene / run | FPS | p95 / p99 ms | Initial ready s | VmRSS KiB |
| --- | ---: | ---: | ---: | ---: |
| Existing forest 1 | 119.448 | 9.993 / 13.360 | 3.161 | 742908 |
| Existing forest 2 | 119.194 | 9.924 / 11.033 | 3.161 | 758852 |
| Controller offline | 147.598 | 8.517 / 9.558 | 3.083 | 743212 |
| Controller integrated | 147.265 | 8.372 / 9.390 | 3.184 | 765456 |
| New river/fall 1 | 131.769 | 9.549 / 12.175 | 4.049 | 821144 |
| New river/fall 2 | 131.373 | 9.625 / 12.423 | 4.113 | 811932 |

The previous phase's matched forest camera had 117.352..118.710 FPS,
p95 10.624..11.119 ms and p99 14.263..14.651 ms. Final forest counts are unchanged:
7704 loaded / 3531 rendered sections, 9171 mesh assets, 89866 represented props,
1277 prop batches. This provides no evidence of an FPS regression in that scene;
it is not a same-executable causal speedup proof for the movement change.
Memory varies and is slightly higher in some runs; no memory-saving claim.

Offline/integrated use the same settled camera and 7623 loaded sections, 4515
rendered sections, 9235 mesh assets and 86727 props / 1248 batches. Client controller
mean/max CPU wall spans are 0.0471/0.1164 ms offline and 0.0346/0.1962 ms integrated.
Stationary numbers do not measure subjective input latency or prove every moving
network scenario is hitch-free; the interpolation/input regression tests address
the identified frame-quantization issue separately.

The new river scene has 7644 loaded sections, 5674 rendered sections, 10496 mesh
assets, 82105 represented props / 1302 batches and **51 water tiles**. Its terrain
and camera differ from the forest, so 131 FPS is a capacity observation, not a
water speedup. Run 1 transmissive GPU pass mean/p95/p99: 0.325/0.329/0.339 ms.
GPU values describe individual passes; total GPU frame latency and exact draw
calls remain unavailable.

Whole-process user/system CPU time for final forest 1: 34.637/5.823 s over
23.762 s wall; river 1: 40.735/6.464 s over 23.796 s. The preceding phase's matched
optimized forest runs used 31.626/5.656 and 33.644/5.574 s; final forest 2 uses
33.377/5.611 s. Thus this is **not** a CPU utilization improvement claim, despite
stable/better frame measurements. Process totals include startup, rendering
workers and scheduling, not just the controller; cross-run differences were not
causally isolated. A preliminary
suite before stable underwater-state early-outs is retained in `intermediate/`;
that cleanup likewise is not claimed as a measured FPS/CPU win.

Renderer-free legacy generation baseline remains 4375 sections / 158 frames /
**2.626 s** cold and reload. One-column move 0.118 s, eviction 4.935 s. The process
used 195248 KiB peak RSS and 4.672/0.125 s user/system CPU. The old generator path
is preserved, but column water metadata has a memory cost; no CPU-generation
speedup is claimed. GPU uploads are excluded from this benchmark, unlike native
initial ready time.

Raw final logs are in `benchmarks/`, earlier non-final runs in `intermediate/`,
verification logs in `logs/`, and selected native images in `captures/`.
