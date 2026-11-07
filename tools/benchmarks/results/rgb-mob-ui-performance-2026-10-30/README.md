# RGB Shapes, Mob Snapshot Recovery And Runtime Work

Phase 159. Linux, Rust 1.97.1, locked workspace dependencies. Existing IDs,
world generation, save formats, view distance and graphic quality are retained.
No vendor patches or dependency upgrades.

## Implemented Corrections

### RGB And Visual Contacts

- Wool stairs (104) and slabs (105) support the existing exact RGB metadata,
  crafting, held/drop/icon presentation, placement, pick and individual mining.
  Cutting dyed wool into shapes retains its color.
- The normal Use action dyes wool, its stairs/slabs and carpets. Authority checks
  the actual target, inventory revision, held dye, reach, permissions and ray.
- Belt recoloring no longer aborts when its geometry commit returns unchanged:
  a real tint change commits independently and consumes exactly one dye. A repeated
  or invalid request remains atomic and does not consume another item.
- In mixed slabs, only the wool half is tinted, even if stone is the primary
  material. Different-colored wool halves cannot share one cell with the existing
  single-color sidecar; these merges are rejected atomically. This limitation is
  deliberate, not an implicit recoloring of the other half.
- Canonical undyed white and explicitly RGB-white are distinct item metadata,
  even where their displayed colors match. Merging must not erase that distinction.
  Removing the wool half from a mixed stone-owned cell also clears its now-invalid
  tint, preserving valid network deltas and saved region data.
- Funnel feet now meet a loaded flat supporting belt as well as solid floor.
  The shared support predicate also drives collision capture. Air, unknown floor
  and sloped/vertical belts do not acquire floating support feet.
  Real job capture respects loaded neighbors even when the edit journal contains
  persisted cells there. Neighbor load/unload refreshes only affected funnel
  snapshots; a local appearance revision rejects already-running stale workers.
- Merger contact pads close the small gaps at all four belt contacts while the
  item corridors remain open. No new block IDs or per-detail entities.
- Client and dedicated server now use protocol 88 for DyeFabric and the appended
  creature snapshot channel. Both must be updated together; save formats are unchanged.

The latest request did not contain an image attachment. These are verified
connection defects, not a claim to reproduce the missing image exactly.

### Why Local Mobs Could Stop Together

Two independent defects were found. The real two-client UDP regression exposed
the more general transport problem: PlayerStates (20 Hz) and CreatureStates
(10 Hz) shared SnapshotChannel, a SequencedUnreliable channel. Lightyear 0.31
keeps one ready latest message per such channel, not per registered Rust message
type. A later player state could discard a complete creature state before the
typed application receiver drained it. Local integrated servers use this same
transport and were affected too.

CreatureSnapshotChannel now gives creatures their own latest-only sequence.
Player transport is unchanged, and their existing distinct tick counters remain
distinct. Protocol 88 identifies the appended channel mapping. A real authority
test checks changing cow age and continuing player ticks on both UDP clients;
it does not inject its valid population snapshots or disable player traffic.
The malformed-payload recovery fixture sends after ordinary creature production,
before the normal network flush, so its exact payload is not accidentally
superseded by a newer valid same-family snapshot. No timeout was increased.

Separately, bee ecology permits naturally born wild bees without a player owner. The client snapshot
validator still required every born creature to have an owner. After a wild bee
birth entered a full snapshot, the client silently rejected the entire snapshot,
including otherwise valid cows, spiders and other moving creatures. The server
could keep simulating while client playback ran out of new positions.

The validator now accepts only the legitimate exception: a bee with valid colony
ecology and its parent/family metadata. Other ownerless newborn species remain
invalid. The server prioritizes visible bee metadata within the existing bounded
ecology payload. Rejected snapshots retain the last valid poses, record a bounded
reason/counter and warn at most once per five seconds. A later valid packet can
recover normally. No interpolation timeout was stretched to hide missing data.

### Runtime Work Removed

1. **Machines:** the renderer previously rebuilt a five-neighbor hash map from
   every loaded section every 100 ms, even with no machines. With 16,059 loaded
   sections this visited 80,295 section/epoch entries per scan. It now visits only
   the existing sparse kinetic section index, probing at most five loaded keys
   for each genuinely occupied section.
2. **Chests/openings:** 20 Hz / 10 Hz candidate scans no longer walk all loaded
   terrain sections. A runtime-only object-section set is maintained at the
   existing central metadata mutation/import/generated-object entry points.
   Partial double chests, vertical doors and adjacent-section machine visibility
   retain the previous semantics. No alternate persistence registry or assets cache.
3. **TiltUI:** changing one shared view previously re-evaluated unrelated property,
   text and flow expressions. The existing expression parser now captures root
   dependencies, including inactive branches, and shared aliases have revisions.
   Minimap changes no longer reevaluate cold inventory/menu expressions. Local
   scopes, calls, imports, Store/Fluent changes and malformed expressions retain
   conservative evaluation. New scopes and parked/restored UI invalidate correctly.
   Changes are in the original `../../tilt-us/tilt-ui` repository, not a local fork.
4. **Footprints:** static marks previously replaced the shared mesh every 50 ms.
   The first 65% of their lifetime has identical geometry/alpha. A persistent dirty
   flag and immutable payload comparison now suppress identical Mesh modifications,
   which otherwise trigger Bevy's global Mesh3d asset-change checks. Actual new
   marks, fade and expiry still update the same shared mesh.
5. **Retirement:** immediate removal previously despawned every retired hierarchy
   and removed its mesh assets in the same Update. Immediate hiding is now separate
   from final reclamation. Explicitly visible mesh/shadow children are hidden too;
   inherited visibility still follows the root. Ownership stays in one generation-
   keyed queue until the PostUpdate drain. The drain admits at most 64 roots,
   256 weighted hierarchy/asset units and about 2 ms per frame, with at least one
   root of progress. This is a soft time bound between indivisible hierarchies,
   not a guarantee that an arbitrarily large single hierarchy takes under 2 ms.
   Pending roots/meshes/materials have hard caps (4,096 / 32,768 / 32,768); overflow
   reclaims oldest entries synchronously rather than allowing unlimited memory.
   Existing visible fade-outs remain unchanged. Standalone callers without an
   installed drainer retain the original synchronous semantics.
6. **Prop invalidation:** when many sections change and few spider-web or support
   owners exist, query from the smaller owner set against deduplicated changes.
   The same 75-cell web / 27-cell support neighborhoods are retained. The sparse
   regression with 1,606 changes and two owners requires at most 150 / 54 probes,
   rather than 120,450 / 43,362. Density, job budgets and loading delays are unchanged.
7. **Exposure:** absent or empty sections skip the final redundant roof/voxel
   branch. The earlier actual-roof, snow-roof and coastal waterline invalidations
   remain intact. A 1,300-case comparison against the legacy predicate covers
   positive/negative section heights and removal/air/water/solid publications.

Opt-in CPU scopes now bracket the six main schedules without replacing them.
Candidate refreshes have separate scopes. Section LOD and mesh ownership counts
are reported only in the opt-in soak telemetry.

## Native Measurement Conditions

Actual hardware: Ryzen 9 5900X, Radeon RX 6950 XT, RADV/Vulkan. Optimized dev
workspace build, integrated authoritative server, real TiltUI HUD, seed 42042,
view distance 20, high shadows/water/material details, real collision-aware input.
The route finds a loaded path out from beneath the spawn canopy, ascends and
travels through new sections. It does not teleport or bypass Creative collision.
Route speed is at most 2.5 blocks/s under the canopy and 12 blocks/s on the open
flight leg; this is not a test at the user's saved 20 blocks/s maximum fly speed.
No simultaneous Cargo job runs during a native sample.

The user's saved graphics configuration requests 3840x2160. A decorated window
was constrained to 3840x2124 by the window manager; that attempt correctly failed
validation and produced no FPS result. The 4K counterchecks use borderless mode
at the verified actual 3840x2160 size, with the other saved graphics settings.

Only short native windows are used. The earlier route that remained under a tree
is not counted as flight. A separate earlier route wrote fly speed every frame,
invalidating GameConfig; it is excluded from comparisons. Identical speed no
longer changes the config tick. Gradual speed changes at canopy corners remain
intentional. A `terrain.refresh.queue.full` count is a budgeted sweep slice, not
a full-world scan: one complete sweep takes multiple slices.

### Rejected Experiment

Bevy's NoCpuCulling path was tested only on static non-shadow-casting meshes,
with capability gating. Same binary, same 1280x800 viewport, Rise animation,
80-second flight, high quality and real HUD:

| Variant | Average FPS | Minimum FPS | p95 ms | p99 ms | PostUpdate mean ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| CPU/GPU existing culling | 88.10 | 36.51 | 18.17 | 22.72 | 4.33 |
| GPU-only noncaster experiment | 85.05 | 35.72 | 18.41 | 21.34 | 4.28 |

There was no demonstrated average/low FPS improvement. Review also found risks
for first-draw transitions and changing Fullmap camera layers. The experiment was
removed instead of being shipped as an unmeasured new rendering mode.

### Production Fix Comparison

4K, ChunkAnimation None, 48-second window, same seed/route/quality. This pair is
not comparable to the differently sized 80-second culling experiment above.

| Build | Average FPS | Minimum FPS | p95 ms | p99 ms | Frames under 32 FPS |
| --- | ---: | ---: | ---: | ---: | ---: |
| Control before sparse-object/TiltUI/footprint fixes | 63.97 | 26.28 | 22.46 | 26.97 | 6 |
| After sparse-object/TiltUI/footprint fixes | 69.54 | 24.03 | 17.80 | 20.80 | 2 |

The controlled pair uses separate fresh configuration/map directories and the
same fixed route. CPU Update falls from 6.27 to 4.99 ms on average; PostUpdate
remains about 7.2 ms. Candidate scans for all three object-render paths are now
below 0.001 ms per call. RSS at the final snapshot is 2,343,972 / 2,396,308 KiB;
this pair does not show a memory reduction. Startup load is 26.96 / 26.10 seconds.

The higher average and better p95/p99 do not prove that the remaining low-FPS
problem is resolved: the single worst frame is worse in the after run. Two
frames remain below 32 FPS, at vertical streaming transitions. These outliers
are investigated separately rather than reported as a hard minimum guarantee.

A separate initial baseline replication measured 66.73 FPS average, 23.24 minimum,
21.40 / 25.35 ms p95/p99, and three frames below 32 FPS. It is retained as
`teramic159-before-4k-borderless.log`, not substituted for the controlled pair.
Its main opaque GPU pass averages 2.64 ms, so that pass alone is not the
explanation for the wall-frame lows. CPU schedules, render preparation, GPU
passes and presentation overlap; their timings must not be added as a serial
frame budget.

### Reproduced Streaming Tail

The follow-up trace `teramic159-exposure.log` uses the same short 4K flight and
adds per-name totals for closed CPU scopes. The old longest-individual-scope
view hid the cumulative cost of thousands of small retirement operations.

| Sample time | Wall frame ms | Retired hierarchies in frame | Retirement scopes ms | Exposure-removal scope ms |
| --- | ---: | ---: | ---: | ---: |
| 5.051 s | 39.71 | 1,606 | 10.50 | 4.06 |
| 6.382 s | 35.69 | 1,141 | 7.37 | 3.27 |

These are vertical streaming transitions, not a measured 20 ms stall in the
async visibility worker. Retirement runs as deferred ECS commands and was not
bounded by the existing mesh-upload admission limit. Its longest individual
scope in the first frame was only 0.118 ms. The main Update schedule took
29.52 ms there. Nested/parallel schedule and worker durations are not additive.

A second seed (7), real rendered rain (0.78, 2,432 precipitation quads), and
1,193 water tiles reproduced the same mechanism in a 32-second flight:
69.30 FPS average, 29.29 minimum, 18.57 / 21.94 ms p95/p99. The worst frame
retired 804 hierarchies in 5.90 ms. This is a pre-retirement-fix diagnostic,
not the final after measurement.

The instrument-free control (`teramic159-clean-control.log`) still reproduced
the low: 67.59 FPS average, 26.69 minimum, 22.07 / 25.62 ms p95/p99, two frames
below 32 FPS. The issue is therefore not merely the opt-in profiler overhead.

### Final Build Counterchecks

Same final protocol-88 binary, two fresh map directories, the same 48-second
instrumented 4K route. The smoke-only `TERRA_BENCH_LEGACY_RETIRE=1` control
restores immediate retirement without reverting the other fixes. Normal client
startup ignores this variable.

| Retirement variant | Average FPS | Minimum FPS | p95 ms | p99 ms | Frames under 32 FPS |
| --- | ---: | ---: | ---: | ---: | ---: |
| Immediate legacy release | 73.49 | 28.58 | 16.96 | 20.23 | 1 |
| Budgeted release | 72.08 | 29.54 | 17.51 | 20.21 | 1 |

This isolated comparison does **not** demonstrate an average-FPS improvement.
It does verify the specific mass-unload correction:

| Matching streaming event | Legacy wall frame ms | Budgeted wall frame ms | Legacy retirement ms | Budgeted enqueue ms |
| --- | ---: | ---: | ---: | ---: |
| 1,606 hierarchies | 34.99 | 26.93 | 10.55 | 1.89 |
| 1,141 hierarchies | 29.01 | 26.35 | 6.44 | 1.38 |

The new drain's maximum observed call is 1.18 ms; final pending roots, mesh
handles, material handles and overflow counters are all zero. Mesh assets at
the final snapshots are 21,751 / 21,752; RSS is 2,465,684 / 2,429,476 KiB.
These are short streaming snapshots, not evidence of long-term leak elimination.

The remaining worst budgeted frame is a different event at 43.86 seconds:
33.85 ms wall time, with 17.66 ms in PostUpdate. Render work overlaps that
schedule. The trace does not identify an individual PostUpdate system or prove
a GPU stall, so that outlier is not declared fixed by the retirement change.

A separate instrument-free final-build countercheck, matching the earlier
instrument-free control's 4K settings, seed, fresh map and 48-second route:

| Build without opt-in profiling | Average FPS | Minimum FPS | p95 ms | p99 ms | Frames under 32 FPS |
| --- | ---: | ---: | ---: | ---: | ---: |
| Original control | 67.59 | 26.69 | 22.07 | 25.62 | 2 |
| Final build | 72.82 | 36.98 | 17.38 | 20.45 | 0 |

The final clean sample loads in 24.39 seconds and ends at 2,408,604 KiB RSS,
16,070 loaded sections, 4,602 rendered sections, 21,754 mesh assets and zero
pending retirements/overflows. This is a measurable improvement in this route,
not a universal 32-FPS guarantee. Real input and async scheduling produce small
trajectory/composition differences, rather than bit-identical frame workloads.

### Rain And Water Countercheck

Seed 7, 32-second 4K flight with actual rain intensity 0.78 and approximately
1,200 visible water tiles. Same instrumented route as the earlier rain control:

| Build | Average FPS | Minimum FPS | p95 ms | p99 ms | Frames under 32 FPS |
| --- | ---: | ---: | ---: | ---: | ---: |
| Before retirement/prop-neighbor/exposure corrections | 69.30 | 29.29 | 18.57 | 21.94 | 1 |
| Final build | 71.62 | 21.78 | 17.67 | 20.77 | 1 |

The final worst frame is **worse**, not fixed: 45.91 ms at 4.22 seconds.
PostUpdate takes 16.54 ms; the overlapping following Update takes 20.04 ms,
with 347 enqueue calls totaling 1.92 ms and exposure-removal work at 2.92 ms.
The available scopes do not resolve the remaining PostUpdate cost. No additional
production fix is asserted from an unproven culling, transform or GPU hypothesis.
Final RSS is 2,589,128 KiB, with zero retirement backlog/overflows.

An additional final-build rain run without opt-in instrumentation also has
outliers: 68.55 FPS average, 24.70 minimum, 20.68 / 25.57 ms p95/p99,
five frames below 32 FPS. It ends at 2,529,504 KiB RSS and zero retirement backlog.
Its real input route reaches a slightly different position (about 14 blocks
further on), so it is not substituted for the matched instrumented comparison.
The rain/streaming low-FPS problem remains open; the clear-weather improvement
must not be generalized to all scenes.

Kernel call-stack sampling through perf record is unavailable under the current
permissions. No sysctl, capabilities or system profiler installation was changed.
Application scopes and actual GPU diagnostic timestamps remain available.

## Validation

- Original TiltUI runtime: **328 tests passed**, including six new selective
  reactivity/parser/scope regression tests. See `teramic159-tests-tilt.log`.
- `cargo fmt --all --check`: passed.
- `cargo check --locked --workspace -j1`: passed.
- `cargo test --locked --workspace --no-fail-fast -j1`: **3,253 passed,
  zero failed, seven existing ignored tests**. This includes all nine retirement
  lifecycle/budget tests, sparse-neighbor equivalence/probe bounds, the exposure
  comparison, RGB authority/crafting/mining/save tests and both real UDP regressions.
- `cargo clippy --locked --workspace --all-targets -j1 -- -D warnings`: passed.
- `cargo build --locked --workspace -j1`: passed, client and dedicated server.
- Native final-binary performance counterchecks above: completed; each process
  exited successfully. No concurrent Cargo job or second test client.
- Native RGB/filter smoke: passed, four nonblank captures, saved RGB world,
  exact Creative pick, actual filter ghost/switch handlers, unchanged filter
  inventory and authoritative RGB crafting. The world capture was inspected:
  blue wool, pink wool stairs and the now-unoccluded cyan wool slab render with
  their saved colors. The RGB recipe capture was also inspected.
- Dedicated server smoke: passed with DISPLAY, WAYLAND_DISPLAY and WAYLAND_SOCKET
  unset; 27 sections prepared, no window or render adapter required.
- `git diff --check`: passed in Teramic and the original TiltUI repository.

[RGB world and logistics](rgb/rgb-world-filter-merger.png),
[RGB recipe](rgb/rgb-coloring-recipe.png),
[filter allowed](rgb/filtered-funnel-allowed.png),
[filter blocked](rgb/filtered-funnel-blocked.png).

## Limits

Short samples do not establish a hard minimum FPS guarantee or long-term stability
under every weather, seed, entity count or 4K scene. No 10/15/30-minute tests were
run. Rain still has measured low-FPS outliers, so A05 is not closed. The exact
image-specific connection request still needs its attachment. Mixed wool slab
halves still use one RGB value per world cell; differently colored wool halves
are rejected rather than silently recolored.
