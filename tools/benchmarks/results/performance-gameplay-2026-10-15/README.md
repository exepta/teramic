# Performance And Gameplay - 15.10.2026

## Scope And Compatibility

No dependency upgrades, renderer rewrite or second world/prop/save system.
World format 1, generator 5, seed, logical resolution, IDs and biomes remain intact.
Network protocol **4** carries snow/fire conditions and per-player permissions;
restart both endpoints together. Old TOML surface conditions default snow/fire to
zero. Old controls gain Camera on an unused F5..F9 binding without replacing
existing custom keys.

## Rendering And CPU Work

Fine terrain inset walls previously remained active throughout the 96 m terrain
near LOD. Their subpixel geometry costs vertex processing in both depth and main
passes. A separate visual-only detail selector now uses 32 m from section bounds,
with 28/36 m hysteresis. It only omits `BlockTemplate.surface` in one shared cached
template variant. Cube faces, bevels, top caps, textures, tree geometry, grass
materials, terrain/prop LOD distances and vegetation density are unchanged.
Geometry composition stays asynchronous; epochs, LOD and micro-detail are checked
again before atomic replacement. No opaque-terrain dithering or overlapping roots.

Grass uses tapered, gently bent voxel prisms rather than stacked stem/tip boxes.
The same variants, 14/8/4 LOD blade counts, distribution, biome colors and picking
bounds remain. A blade has 20 vertices/10 triangles and no hidden ground-facing
bottom. No extra entities or materials. This is an incremental art pass, not a
claim of reference-image parity or changed logical terrain/tree generation.

Empty world job queues no longer mark ChunkStore changed every frame. Settled
terrain/prop/water dispatch skips full-world scans until actual inputs change.
Terrain collision searches the small build neighborhood around each simulation
center instead of every loaded render section. Hidden menu views are not rebuilt
each gameplay frame. Controller CPU instrumentation records call count, mean and
maximum elapsed CPU wall time for the shared authority/prediction step. It is not
a sampled call-stack profiler; `perf` is unavailable on this machine.

## Controller And Camera

The shared Avian controller steps onto one-meter obstacles only when grounded,
forward movement is blocked, the raised capsule path is clear, collision data is
ready and a leading-foot downcast finds a walkable surface. Low ceilings and tall
walls remain blocking. The leading-foot probe avoids rejecting rounded capsule
contacts at the step lip. Jump/ordinary wall collision remain covered.

Swimming uses the existing world sea level and procedural water-bed check, with
buoyancy, drag, held Jump for ascent and Fly Down for descent. Dry caves above/below
the water bed do not become swimming zones. This is sea/terrain swimming, not a
dynamic water-volume solver. No flight permission is required to swim.

Camera Toggle defaults to F5 and uses persistent rebinding. Third person has a
3.5 m sphere-cast-limited boom and the same shared six-part player model as remote
players. Picking starts at the body's eye, preserving server reach validation.
Switching back removes the local avatar. No new character assets or draw per
micro voxel.

## Dedicated Saves And Permissions

`config/server.toml` defaults to `save_directory = "saves/dedicated"`.
`--save-dir PATH` selects another directory; `--no-save` explicitly disables it.
Relative paths use the config root's parent. Existing manifests must match the
complete world configuration; corrupt/mismatched saves and orphaned edit files
are rejected, never silently regenerated under another seed. A retained OS file
lock prevents concurrent writers and is released automatically after process exit.

The existing sparse EditJournal is restored before startup, snapshotted to an
async worker every ten seconds when dirty, and finally saved after waiting for
any previous snapshot during graceful shutdown. Atomic TOML storage is reused.
Errors are logged; final-save failure yields an error exit. Back up the directory
while stopped. No retention/rotation or administrative in-game save browser is
claimed. Explicit render/server smoke fixtures do not write production saves.

`ServerConfig.player_permissions` maps stable UUIDs to explicit `build`, `flight`
and `environment` booleans. Defaults preserve existing behavior: building enabled,
flight/environment follow legacy `allow_flight`. Rights are captured at admission,
sent to the client, enforced on the server and respected by gameplay/menu controls.
Changing server configuration requires reconnect/restart, not an undocumented
live reload. Local UUID rights are for trusted LANs; UUID ownership is not verified.

## Weather, Snow And Fire

Rain/snow reuse the existing local weather graphics options. One bounded dynamic
mesh holds up to 512 particles, with cached/budgeted roof checks. No particle ECS
explosion or new rendering pass. Snow cover is sparse authoritative state on the
existing journal, not a new block layer: selected top faces use the existing Snow
material at every terrain LOD, including micro-leaf tops. Neighbors stay unchanged.
The logical block still has its original material/occupancy and collision.

Authority advances active fire once per second, capped at 256 cells. Dry exposed
grass/wood/leaves can spread; wetness/snow extinguish; fire decays and leaves
persistent char. Unloaded cells pause. Clients consume confirmed deltas instead
of independently evolving fire. Nearby flames use one reusable mesh/material
batch (maximum 64 visible cells), updated at 10 Hz, not one entity per flame.
Creative wet/extinguish, snow, ignite and reset commands are available from the
existing TiltUI graphics options when a block is targeted and permission allows.

Boundaries: no global weather authority, automatic snow accumulation, melting,
health damage, resource drops or destructive burning. The implementation does
not pretend those larger gameplay systems exist.

## Validation

Workspace fmt/check/test/Clippy (`-D warnings`)/build passed. **234 tests** pass in
the normal suite; the separately executed ignored UDP soak adds one passing test.
Separate dedicated build passes and its feature tree contains no `bevy_render`,
`bevy_winit`, `wgpu` or TiltUI. Native no-display server startup/reload/final saves
and concurrent-save rejection were exercised against an isolated temporary save.

The real UDP soak ran 180.01 seconds, 26,975 host updates and 8,992 newly received
snapshots per client. Identity, connection state and snapshot progress were checked
continuously. This is a short local soak, not a multi-hour/Internet certification.

Native Vulkan fixtures passed for third-person avatar/boom/first-person return;
snow cover, rain, snowfall, ignition/extinction; physics fall/grounding/jump/wall/
support mining; and production TiltUI session lifecycle with world CRUD, rebound
movement, authoritative editing, explicit save, host restart and journal reload.
Third-person, snow cover and snowfall captures were visually inspected. Rain and
fire captures were also inspected, with roof occlusion and small stylized flames.
Unit coverage includes low ceilings/one-block steps/swimming, permission isolation,
snow across all LODs, geometry bounds/indices, fire authority/replicas, old save
conditions, final save ordering and exclusive dedicated-save access.

## Benchmarks

AMD RX 6950 XT, RADV Mesa 26.2.4-arch3.1, Vulkan, Wayland, dev profile with optimized
dependencies. View radius 16, vertical radius 3, terrain LOD 96/160, props 80/144,
full vegetation density. Exact camera, physical resolution and counts are logged.
`tools/benchmarks/run-render-suite.sh` performs sequential native runs without
parallel clients/builds. `measure.py` records process CPU time and peak RSS.

Three 18-second forest pairs compare full micro-relief with optimized detail
distance on the same executable. Two 18-second controller pairs compare offline
and real Integrated Server. Two 24-second moving LOD runs cross section boundaries
and return. `TERRA_BENCH_FULL_MICRO=1` only affects explicit smoke fixtures.
Frame intervals use Instant; screenshots are not taken during this measured suite.
GPU values are per instrumented pass, not total GPU frame latency or draw calls.

An initial repeated series was discarded for FPS inference: unfocused Bevy
windows were background-throttled near 60 Hz despite VSync being disabled.
Explicit benchmark fixtures now use WinitSettings::continuous; normal gameplay
keeps its existing background power behavior. Intermediate pre-series single
runs are preserved separately and are not the primary comparison.

### Stationary Forest

1920x1200 physical, identical camera and scene in every row. Full = the same final
executable with micro-relief throughout terrain near LOD; optimized = normal
32 m micro-detail selection. Both already include the new grass/CPU cleanup.

| Variant / run | FPS | p95 ms | p99 ms | Initial ready s | VmRSS KiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| Full 1 | 87.167 | 13.610 | 23.625 | 4.326 | 742480 |
| Optimized 1 | 118.710 | 10.624 | 14.535 | 3.170 | 712920 |
| Full 2 | 87.134 | 13.178 | 20.667 | 4.255 | 741712 |
| Optimized 2 | 117.352 | 11.119 | 14.651 | 3.119 | 739300 |
| Full 3 | 87.222 | 13.348 | 20.176 | 4.316 | 751376 |
| Optimized 3 | 118.157 | 10.808 | 14.263 | 3.271 | 735980 |
| Median full | 87.167 | 13.348 | 20.667 | 4.316 | 742480 |
| Median optimized | 118.157 | 10.808 | 14.535 | 3.170 | 735980 |

Median FPS +35.6%, p95 -19.0%, p99 -29.7%. Percentiles here are medians of
per-run percentiles, not a fabricated pooled distribution. The first full run's
23.625 ms p99 is retained. Every forest run has 7704 loaded sections, 3531 rendered
sections, 9171 mesh assets, 4764 visible mesh entities, 89866 represented props and
1277 prop batches. No density/draw-distance reduction accounts for this result.

Run 1 GPU main-pass vertex invocations fall from 12,605,227 to 8,624,177 (-31.6%).
Early prepass mean: 1.367 -> 0.950 ms; opaque pass mean: 2.120 -> 1.515 ms.
These are separate pass measurements, not total GPU frame latency. Mesh/material
batch counts did not fall; no exact draw-call reduction is claimed.

The original pre-edit single run is also archived: 85.158 FPS, p95 13.544 ms,
p99 15.572 ms, initial 4.423 s, VmRSS 758972 KiB. The CPU-only intermediate
run was 85.468 FPS / 13.561 / 19.896 ms: no FPS/p95 win and worse p99. The first
micro-detail trial was 113.574 FPS / 11.172 / 14.818 ms. Those single runs depended
on desktop focus and are context, not the controlled final A/B evidence.

### Moving LOD

24-second out-and-back path, same ending camera, all jobs completed, no opaque
VisibilityRange/dither entities permitted by the fixture.

| Variant | FPS | p95 / p99 ms | Terrain jobs | Composition worker spans s | Final VmRSS KiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| Full | 94.062 | 14.615 / 18.985 | 19481 | 5.201 | 841336 |
| Optimized | 120.212 | 11.942 / 15.132 | 21197 | 5.355 | 804184 |

Tradeoff: independent micro-detail swaps add 8.8% terrain composition jobs and
3.0% summed composition worker wall spans on this path. Whole-process user CPU
time increases 64.826 -> 74.199 s while delivering more frames in the same window;
this is not reported as a CPU reduction. Both paths completed ~12,981 terrain-LOD
swaps. Peak RSS from the process wrapper: 838196 -> 801704 KiB. Single moving pair,
not a statistical guarantee against every possible hitch. No universal 60+ FPS
guarantee is made for other machines, dense edits, weather or arbitrary view settings.

### Offline And Integrated Server

Identical walk controller, camera rotation, 7623 loaded / 4515 rendered sections,
9235 mesh assets, 86727 represented props and 1248 prop batches, 1920x1200.
Settled eye heights differ by 0.000238 m due to reconciliation, not a scene change.

| Mode / run | FPS | p95 / p99 ms | Initial ready s | VmRSS KiB | Controller mean / max ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Offline 1 | 145.225 | 8.939 / 11.215 | 3.096 | 766144 | 0.0512 / 0.2210 |
| Integrated 1 | 145.289 | 8.920 / 10.503 | 3.117 | 766392 | 0.0327 / 0.6360 |
| Offline 2 | 145.568 | 8.824 / 10.483 | 3.214 | 739536 | 0.0508 / 0.2491 |
| Integrated 2 | 145.368 | 8.609 / 10.234 | 3.255 | 754476 | 0.0357 / 0.3603 |

No relevant FPS loss is measurable in this stationary scene. This is not proof
of zero network cost under movement, packet loss, many peers or a different GPU.
Instrumentation covers the client's controller calls, including prediction/replay;
the integrated authority owns a separate profile. Process CPU/RSS includes its
thread. Controller maxima below 0.64 ms here do not explain multi-millisecond
whole-frame tails; no claim of a complete CPU call-stack profile is made.

### CPU Generation Baseline

Renderer-free, radius 12/3, four workers, 60-Hz ECS pacing: cold 4375 sections /
158 frames / **2.626 s**, one-column move 8 frames / 0.117 s, eviction 5273 sections /
296 frames / 4.934 s, reload 158 frames / 2.626 s. This preserves the archived
2.626 s baseline. Process peak RSS 189832 KiB, user/system CPU 4.443/0.139 s.
This benchmark excludes GPU upload and is not interchangeable with native initial
ready time. Compared with the historical 165772 KiB run, peak RSS is higher; no
memory improvement is inferred from that non-isolated comparison.

Five warm worker runs without frame pacing: generation median 0.209081 s,
render preparation median 0.375663 s. Same checksum 22955883, 4375 sections,
50506 props and 450427 prepared quads as archived. Historical medians were
0.203734/0.368670 s: current numbers are about 2.6%/1.9% slower, not a claimed
generation speedup. No generator algorithm changed; cross-run machine variance
is not isolated. Render preparation includes padding/meshing/water and is not
an isolated meshing time.

Raw native logs are in `benchmarks/`, CPU baselines in `logs/`, native acceptance
captures in `captures/`, and the discarded focus-throttled series in
`discarded-focus-capped/`. The latter is retained only to explain the measurement
correction, not to support a speedup claim.

## External Blockers

No account provider, session-validation API, trust key or login-proof format is
available. Public unverified joins remain closed; `require_verified_accounts`
fails closed without a verifier. Integrate real proof verification before
`gateway::respond(Request::Join)` issues credentials and in `validate_claim`.
Do not solve authentication by merely trusting a caller-supplied UUID.

No second physical machine or Internet test endpoint is available. Cross-machine
LAN/multicast/firewall/router and Internet behavior remains unverified. Reference
art approval remains an explicit art task, not an automated test result.
