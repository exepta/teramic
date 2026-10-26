# Phase 126: Villagers, Village Sizes, Pipes And Jump Corrections

## Performance Findings: Diagnosis Only

No further rendering, quality, streaming or water performance optimization is
part of this phase. The user requested the remaining bottlenecks first, before
approving their implementation. The physics corrections below repair lost input
and invalid state transactions, not frame rate.

### Fresh Native Baseline

`performance-baseline.log` is an actual native Vulkan run on the RX 6950 XT,
using the existing seed 42042, structure revision 2, integrated authoritative
server and full HUD. The camera faces the same village field as phase 125.
1280 x 800, view distance 12, High shadows/water, rain, an eight-second sample
after loading. No Cargo process ran concurrently. No long-duration soak test.

| Metric | Measured Value |
|---|---:|
| Average FPS | 57.03 |
| p95 frame time | 23.46 ms |
| p99 frame time | 26.65 ms |
| Initial loading | 25.33 s |
| Resident memory | 1,744,172 KiB |
| Loaded / rendered sections | 5,781 / 2,785 |
| Mesh assets | 12,187 |
| Visible mesh entities | 5,008 |
| Shadow-casting point lights | 5 |
| Render schedule wall interval | 16.79 ms/frame |
| Main schedule wall interval | 9.00 ms/frame |
| Render execution interval | 10.13 ms/frame |
| Point-shadow group, inside render execution | 3.14 ms/frame |
| Bind-group preparation interval | 1.51 ms/frame |
| Asset preparation interval, stationary | 0.50 ms/frame |

Intervals overlap between threads; the shadow group is nested within render
execution. These numbers must not be added. They are wall intervals, not measured
GPU pass times. The baseline executable precedes this phase's feature changes.

`flight-baseline.log` repeats the existing out-and-back forest route with rain,
the same seed, view distance and resolution, offline and without the integrated
HUD. It sampled 24 seconds, returned and completed all pending mesh jobs:

| Flight Metric | Measured Value |
|---|---:|
| Average FPS | 69.71 |
| p95 / p99 frame time | 22.41 / 29.38 ms |
| Main / render wall interval | 8.85 / 13.41 ms/frame |
| Asset preparation | 4.08 ms/frame |
| Bind groups | 1.55 ms/frame |
| Terrain dispatch | 1.34 ms/frame |
| Props dispatch | 0.51 ms/frame |
| Farming | 0.059 ms total over the entire sample |

The different scenarios are not an FPS before/after comparison. Both freshly
measured runs are diagnostic baselines without performance code changes.

### Remaining Problems In Priority Order

1. **Rendering remains the critical path in stationary villages.** The fresh
   sample spends 16.79 ms in the render schedule versus 9.00 ms in Main. Actual
   execution is 10.13 ms, including 3.14 ms for the retained point shadows.
   Further per-pass CPU/GPU measurements are needed before choosing a fix.
2. **Asset preparation grows substantially while flying/streaming.** The earlier
   same-machine flight sample measured 4.044 ms/frame, freshly reproduced at
   4.085 ms/frame in `render.assets`, versus
   0.50 ms in this stationary run. Bevy's existing `PrepareAssets` includes mesh
   allocation, release, copies and preparation, but the aggregate interval cannot
   identify which of those is responsible. Split those measurements next.
3. **Bind-group preparation is a persistent cost.** 1.51 ms in this run;
   1.54-1.87 ms across the preceding village/flight/4K samples. The existing Bevy
   mesh and preprocess binding paths iterate render phases/views. This makes
   them candidates for closer measurement, not proof of an asset leak or a
   particular cache fix.
4. **Streaming also costs Main-thread preparation time.** The previous flight
   measured terrain dispatch 1.324 ms/frame, including 0.623 ms job preparation
   and 0.430 ms full queue refresh. Props dispatch added 0.473 ms. Upload work
   was smaller (terrain 0.294, props 0.191, water 0.197 ms/frame). These scopes
   are not all additive and should be correlated with individual slow frames.
5. **Not every outlier is chunk loading.** The preceding 4K/view20 run recorded
   an 81.77 ms maximum frame and 43.72 ms p99 with some worst frames showing
   zero generation/meshing/prop jobs, zero uploaded vertices and zero chunk
   animations. Main-system and GPU timing is needed to locate these outliers.

The earlier raw logs and complete configurations remain in
`../tree-structures-performance-2026-10-25/`. They are not a before/after
comparison with new village geometry. Farming was only 2.63 ms **total** in this
eight-second run, so wheat growth is not supported as the stationary bottleneck.
No growing resource count, leak, or GPU bottleneck is claimed from these short
samples. No 10/15/30-minute tests were performed.

## Implementation

- Chimney infill now samples actual oriented slab/stair contact profiles,
  including mixed slabs and stair corners. The minimum of three touching roof
  neighbors remains. Fill pieces use their neighboring material; the lumen
  stays open. Two-cell corner dependency changes invalidate the affected pipe.
- Graphics choice boxes for cave lighting and tree fall animation are present
  in the actual TiltUI template and use the existing saved setting handlers.
- Collision-unready physical ticks preserve complete movement state and do not
  consume queued jumps. Server movement acknowledgment is transactional with a
  successful physical tick. Incomplete client collision replays are deferred.
- A reproduced mob-support defect treated an overhead door lintel as a possible
  foot support, rejected that height and never tried the actual floor below it.
  Support candidates now obey the existing 1.01 m step-height bound. Exact door
  centering and stable entry/exit targets supplement the shared bounded A*, so
  a coarse one-meter route cannot leave a citizen offset against the door frame.
- Villagers append to the existing creature catalog, shared models, snapshots
  and region persistence. Stable settlement/bed slots give persistent IDs,
  names and ten visual role variants. No new independent NPC registry or
  per-clothing material instances. One resident per generated bed; dead IDs
  remain tombstones. Residents do not consume the natural animal cap.
- Loaded prepared columns expose cached settlement metadata. Discovery runs at
  1 Hz, with a bounded runtime site cache, and does not generate extra regions.
  Daytime walks use existing swept support probes and budgeted A*. At night
  residents return to their assigned beds, open their home's actual door and
  lie chest-up on the mattress. Bed removal and unloaded terrain fail closed.
  A right-click greets the player and turns an awake resident toward them.
  Automatic doors use the same paired-state validation and player-clearance
  preflight as manual interactions; a blocked door is retried, not forced.
  Replaced beds must still have valid loaded foot/head peers and the assigned
  orientation. Dye changes remain valid; stale bed directions are not used.
- New worlds use structure revision 4. Residential house counts are additional
  to civic buildings: Small 6-8 + one Medic and Church; Medium 10-16 + one or
  two Medics, one Church and Smith; Large 18-24 + one or two Medics, one Church,
  Smith and Police. Each house has at least two beds. Existing revisions 0-3
  are not silently rebuilt, and player edits are preserved.

## Scope And Limitations

The ten citizen models follow the supplied clothing/tool/color reference using
shared procedural voxel meshes; exact pixel-for-pixel reproduction is not
claimed. Roles currently define appearance and household allocation. Trading,
profession production, policing/combat jobs and an economic inventory are a
later expansion, as specified by F10. Household meals and overnight recovery
are abstracted rather than consuming simulated merchant stock.

## Native Validation

All runs use the native RX 6950 XT/Vulkan client. No concurrent Cargo work and
no long-duration soak tests. These fixtures validate features, not an FPS
before/after comparison against the older village layout.

- `menu-graphics.log`: actual TiltUI choice-box parts, selected labels, options
  and event bindings for both previously blank controls. Apply/reload checks
  exercise Low/Hell Fall, Ultra/Magic and Medium/Current.
  `menu/graphics-cave-lighting.png` and
  `menu/graphics-tree-fall-animation.png` show the actual controls scrolled
  into view and were visually inspected. The successful run logs one
  `Failed to send screenshot: sending on a closed channel` at shutdown,
  after the saved captures and passed assertions. That asynchronous screenshot
  teardown message remains open; it is not presented as an error-free log.
- `villager-models.log`: all ten shared villager models, gait/idle poses and
  four rotated bed poses render successfully. `villagers/villagers-1.png` and
  `villagers/villagers-3.png` were visually inspected. This gallery is not an
  authoritative AI simulation.
- `pipe-roofs.log`: actual loaded world cells, orientation journal states,
  current uploaded mesh epochs and open pipe lumens checked for full blocks,
  stairs and mixed upper/lower slabs. `pipe-roofs.png` was visually inspected.
- `generated-village-night.log`: a real seed-42042 settlement at region [-2, 2],
  center [-574, 32, 962], loaded through the integrated authoritative server.
  All 17 generated residents are replicated and rendered; 16 are sleeping at
  the final checkpoint. The assertion requires real replicated/rendered NPCs
  and completed sleep, not manually injected gallery creatures. The outdoor
  screenshot is an overview, not a view through house walls at sleeping NPCs.
  An eight-second sample measured 50.56 FPS, p95 23.80 ms, p99 25.57 ms and
  1,895,060 KiB RSS; this different night scene is not the old rain baseline.
- `generated-village-night-final.log`: repeated with the final executable,
  including safe automatic doors and replaced-bed validation. All **17 of 17**
  residents are replicated, rendered and sleeping at the final checkpoint.
  The eight-second sample measured 50.33 FPS, p95 24.10 ms, p99 26.05 ms and
  1,908,976 KiB RSS. It completed without warnings/errors. The earlier run's
  sixteen sleepers are preserved as a separate checkpoint, not overwritten
  or retroactively described as seventeen.
- `real-terrain-villages.log`: reproducible real-terrain plans for seeds 42042,
  1337 and 99173 produce Small/Medium/Large settlements with 7/11/19 residential
  houses and 17/28/46 total beds respectively, meeting their civic quotas.
- `headless-server.log`: dedicated server starts without DISPLAY or
  WAYLAND_DISPLAY and completes its 27-section smoke test.

## Automated Validation

- `cargo fmt --all --check`: passed.
- `cargo check --locked --workspace`: passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo build --locked --workspace`: passed for client and dedicated server.
- `cargo test --locked --workspace --no-fail-fast`: **2,474 passed, zero failed,
  seven ignored**. This is the final complete run after the shared mob-support,
  safe automatic-door and replaced-bed corrections, not just the earlier
  targeted tests. Compilation was serialized with `CARGO_BUILD_JOBS=1`.

Regression coverage includes real UDP resident replication and actor-only
greetings, persistent identities/death tombstones, all ten role models,
night/day bed transitions, precise door paths in all four rotations, replaced
beds, occupied door swing areas, atomic paired-door preflight, late collision
readiness, retained jump input and deferred authoritative movement ACK/replay.
The real-terrain settlement tests check size quotas, sufficient valid beds,
cached resident metadata and preserved legacy generator contracts.

Raw command output is retained in `cargo-check.log`, `cargo-clippy.log`,
`cargo-test.log` and `cargo-build.log`. `git diff --check` also passed. The
native menu capture teardown message above remains an explicit open issue;
no long-term performance acceptance is inferred from feature smoke tests.
