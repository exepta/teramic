# Belt Contacts, Shaft Ownership, Biome Distribution And Presentation Stalls

Phase 160. Rust 1.97.1, locked Bevy 0.20 workspace. No dependency upgrade,
vendor directory, renderer replacement, quality reduction or FPS limiter.

## Belt Corrections

The supplied numbered image was inspected. The conveyor ribbon ended around
its endpoint shaft center, leaving approximately 0.55 m to a side-entry merger.
A loaded, geometrically matching merger/belt contact now adds a short carrying
tongue to the existing ribbon mesh, at the same carrying height and with the
same UV/material. The end mask has 16 bounded variants and at most 48 extra
vertices per ribbon; it adds no individual entity or material. Unconnected
ends retain the original geometry.

Conveyor wood crossed the real shaft axis and a static metal replacement cap
covered the shaft. The authored conveyor now has an open bearing and matching
picking geometry. Its real endpoint/guide shaft is visible and rotates through
the opening. Only the conveyor asset was regenerated, with 1,272 vertices and
four shared material groups.

The previous span format needed at least one empty inner cell to hold material
148 as controller. Thus four directly adjacent shafts could not represent the
requested complete belt. Shaft insertion into an existing 148 cell was also
rejected by both placement and authority.

`BeltOwner { start, end, item }` extends the existing sparse edit metadata. A
controller can now be material 148 or a real shaft (144..146). Its belt item
retains frame wood and exact named/RGB dye separately from each shaft's own
material, wood and axis. Existing old 148-controller spans remain readable.
All routing, drop transport, power, recoloring and removal reuse the existing
bounded span engine. No extra transport simulation or virtual item inventory.

Four adjacent shafts connect the selected first and last endpoints. A shaft
can be inserted into any inner cell of a linked span, including the old
controller and the last remaining 148 cell. The old unlinked one-cell 148
decorative belt is not silently converted into a linked shaft assembly.
Selected endpoints still need an inner cell (distance at least two cells).

Protocol **89** covers the new SectionDelta metadata and biome selection
weight. Update client and dedicated server together. Region envelope **RGA8**
appends belt ownership outside the frozen old section bytes. Old RGA7 and
older saves remain readable, and regions without new metadata retain their
previous envelope. Removing the new owner cannot leave stale RGA8 data behind.
No existing block ID or world ID is changed.

Regression coverage includes exact four-shaft ray selection; later insertion
in horizontal, sloped and diagonal spans; raw RGB/frame/shaft independence;
transport, stress load, atomic revision overflow, actual authority inventory
consumption and exact drops; SectionDelta and negative section/region
boundaries; repeated RGA8 save/load; legacy section layout and malformed owner
rejection. A combined RGA8 fixture also carries RGB fabric, a filtered funnel
and a mixer in the same region.

## Highlands And Rivers

The actual new-world terrain is the restored MAIN landscape (revision 1), not
the optional advanced heightfield. Its Highlands selection was hardcoded and
WorldLibrary removed biome JSON snapshots for these worlds, so changing a
Highlands JSON file alone could not change its frequency.

New local and dedicated worlds now save a validated biome-definition snapshot.
`selection_weight` defaults to exactly 1.0 for old definitions. Highlands use
0.65; land-only weights are normalized without changing the ocean noise weight.
Dominant-biome boundaries can move slightly in new worlds; sampled land counts
are included in the raw CSVs rather than assumed identical.
The existing MAIN height/profile architecture is retained.

River revision 4 uses the existing bounded brook generator on 416 m cells
instead of 512 m cells. Its curves and banks remain inside each cell's finite
bounds. Main river networks and outlet routes remain unchanged. An initially
tested extra-tributary approach was rejected: it displaced existing brooks and
did not increase measured actual wet area.

Existing saved worlds, absent snapshots, old unit-weight snapshots and river
revisions <=3 retain their original generation. These distribution changes
require a newly created world; old worlds are not retroactively reshaped.

Five seeds: 42042, 7, 99173, 12345, 86420. Biome grids cover an 8192 m square per
seed; water grids cover a 4096 m square per seed. Highlands are normalized to
actual land. Water coverage counts actual open wet river surfaces, excluding
oceans, lakes and merely river-influenced banks.

| Metric | Before | Final new-world configuration |
| --- | ---: | ---: |
| Highlands / land | 16.145512% | 13.583636% |
| Open wet river / land, 8 m grid | 0.325795% | 0.410852% |
| Sampled river water area | 0.142336 km2 | 0.178880 km2 |

The dense water grid contains 1,310,720 points across the five seeds. Every seed
increases wet river coverage; the combined relative increase is 26.1%. With
identical new biome weights, changing only river revision 3 to 4 increases
coverage from 0.329703% to 0.410852% (+24.6%). The independent 16 m grid also
shows an increase. These are sampled regional shares, not guaranteed percentages
at every spawn.

## FPS: Reproduced Synchronization Stall

Ryzen 9 5900X, Radeon RX 6950 XT / RADV Vulkan, Xwayland, actual 3840x2160,
view distance 20, high shadows/water/block detail, micro geometry enabled,
ChunkAnimation None, real integrated authority and real TiltUI HUD. Separate
fresh configuration/map directories, same input route and no concurrent Cargo
or second native client. Normal production task-pool policies are unchanged;
the fixture's Compute cap is 16, with default IO/Async policies.

The original binary was preserved before this phase's edits. On new seed
86420 it reproduced **7.68 FPS / 130.23 ms**. An exact system-body scope measured
**118.93 ms in `prepare_windows`**, while Main completed in 17.37 ms. The main
thread then spent 112.33 ms in the pipelined render handoff. This event is a
render-window acquisition path stall, not a 119 ms chunk generation job.
Another acquisition event lasted 37.39 ms. No mass retirement overflow occurred.

### Isolated Driver-Mode Experiment

Same preserved executable, same quality and route, 48-second sampling windows
after real scene readiness. Flight speed 12 m/s, with a slower collision-checked
canopy exit. The temporary per-process `MESA_VK_WSI_PRESENT_MODE=mailbox`
override is documented by [Mesa](https://docs.mesa3d.org/envvars.html#envvar-MESA_VK_WSI_PRESENT_MODE).
It was used only for diagnosis; the product does not set this environment
variable or modify the system driver.

| Seed / weather | Mode | Average FPS | Minimum FPS | p95 ms | p99 ms | Frames below 32 FPS |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 86420 / clear | Original AutoNoVsync | 72.68 | 7.68 | 18.40 | 21.36 | 3 |
| 86420 / clear | Mesa Mailbox experiment | 71.28 | 31.60 | 18.19 | 21.31 | 1 |
| 12345 / rain | Original AutoNoVsync | 71.82 | 20.85 | 18.93 | 21.82 | 5 |
| 12345 / rain | Mesa Mailbox experiment | 77.32 | 38.80 | 16.75 | 19.09 | 0 |

Seed 86420's acquisition scope averages 0.059 ms in the Mailbox run; its
30-second interval has no >8 ms acquisition calls and maximum 1.04 ms.
This is a tail-latency correction, not an average-FPS improvement in both seeds.
RSS before/experiment is 2,382,416 / 2,484,940 KiB for 86420 and
2,298,288 / 2,235,604 KiB for 12345. Initial load is respectively
27.77 / 27.36 seconds and 14.17 / 21.86 seconds; startup is not claimed faster.

### Application Correction

With VSync disabled, the existing Window API now prefers Mailbox specifically
on **Linux + AMD vendor 0x1002 + Vulkan + RADV**. Other adapters/backends keep
AutoNoVsync; VSync enabled always keeps AutoVsync. Bevy's existing supported-mode
fallback remains in charge. No frame-latency hint or explicit FPS limit changed.
This avoids the measured problematic Immediate presentation path without
patching Bevy/wgpu or reducing terrain quality.

The explicit benchmark-only `TERRA_BENCH_PRESENT_MODE=auto|mailbox` permits
same-final-binary counterchecks. Normal startup ignores this variable. The
real-input route additionally accepts `TERRA_SOAK_ROUTE_SPEED=20` for the saved
player flight speed; its unchanged default remains 12 for earlier comparisons.

### Final Application-Only Measurements

Both presentation environment overrides were unset. The final executable logs
`Applied window presentation policy mode=Mailbox` itself. The Xwayland runs use
the same legacy world configuration as their preserved-executable controls:
MAIN revision 1, river revision 3, no biome snapshot. Thus the distribution
change cannot make their terrain cheaper. Every row is one actual 48-second
12 m/s real-input sample at the common 4K/view-20 settings above.

| Window path / seed / weather | Build | Average FPS | Minimum FPS | p95 ms | p99 ms | Frames below 32 FPS |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Xwayland / 86420 / clear | Preserved before | 72.68 | 7.68 | 18.40 | 21.36 | 3 |
| Xwayland / 86420 / clear | Final application policy | 71.96 | 34.98 | 17.97 | 20.15 | 0 |
| Xwayland / 12345 / rain | Preserved before | 71.82 | 20.85 | 18.93 | 21.82 | 5 |
| Xwayland / 12345 / rain | Final application policy | 70.43 | 41.99 | 18.45 | 20.78 | 0 |
| Native Wayland / 86420 / clear | Preserved before | 59.98 | 29.06 | 20.44 | 26.58 | 2 |
| Native Wayland / 86420 / clear | Final application policy | 79.55 | 30.32 | 16.79 | 19.77 | 1 |

Native Wayland was forced by removing DISPLAY while retaining WAYLAND_DISPLAY;
both runs confirmed the same actual 3840x2160 framebuffer. Its average improves
32.6%, while the sampled `render.prepare_windows.body` mean decreases from
7.476 ms to 0.098 ms. The final Xwayland scope means are 0.064 ms (clear) and
0.060 ms (rain); whole-run acquisition maxima are 8.30 and 4.95 ms. Average FPS
does not improve in both Xwayland comparisons, so this is not presented as a
universal throughput gain.

A separate **same-final-executable** AutoNoVsync control on Xwayland seed 86420
measures AVG 67.48, MIN 32.30, p95 19.09 ms and p99 21.89 ms. Its 48-second window
does not reproduce the rare 119 ms acquisition event. The event is intermittent;
one favorable control is not substituted for the earlier recorded bad frame.

Before/final RSS is 2,382,416 / 2,456,476 KiB for Xwayland 86420,
2,298,288 / 2,351,232 KiB for Xwayland 12345, and
2,480,236 / 2,556,696 KiB for native Wayland 86420. Initial loads are respectively
27.77 / 27.90, 14.17 / 22.26 and 26.52 / 26.77 seconds. No memory reduction or
startup-speed improvement is claimed from these runs.

### Actual New World And Faster Flight

The native loading fixture creates a **new** world on seed 99173 using the real
WorldLibrary menu path. Its saved [manifest](new-world/world.toml) contains MAIN
revision 1, river revision 4 and the actual biome snapshot with Highlands
selection_weight 0.65. The fixture verifies the 100% hold, reveal, delayed world
audio and a successful integrated gameplay save. Screenshots are under
[new-world](new-world/loading-complete.png).

The benchmark then reopens that exact isolated saved world, rather than
constructing an unsaved default generator. Flight uses real input at **20 m/s**,
without teleportation or noclip, at 4K/view 20 with rain and the real TiltUI HUD.
The 48-second sample measures AVG **63.20**, MIN **31.94**, p95 **20.11 ms**,
p99 **22.63 ms**, and **one** frame below 32 FPS (31.313 ms). It ends with
17,556 loaded sections, 1,681 water tiles and 2,428,056 KiB RSS. This faster,
different scene has no matching old-world baseline and is not treated as one.

The remaining worst-frame CPU scope windows include Main/update/post-update
work, visibility worker work and light discovery; overlapping worker spans are
not proof of serial main-thread execution. They are not the previous 119 ms
window-acquisition event. A universal 32-FPS floor is **not** achieved.

## Validation

All commands ran sequentially through the single locked workspace graph; no
Cargo or second native client ran during an FPS measurement:

```text
cargo fmt --all --check
cargo check --locked --workspace -j1
cargo test --locked --workspace --no-fail-fast -j1
cargo clippy --locked --workspace --all-targets -j1 -- -D warnings
cargo build --locked --workspace -j1
```

Final result: **3,277 passed, 0 failed, 7 ignored**; all formal commands exit 0.
The initial full suite exposed two outdated geometry/snapshot expectations and
a corrupt-file test that attempted an exclusive create over its own fixture.
The geometry test now asserts the open bearing, real ring and inserted shaft;
the dedicated test preserves the entire explicit ore configuration through
resolve/open/restart; the malformed-save fixture now actually overwrites its
test file. No production ore rule or validation was weakened.

The native RGB/logistics fixture passes with four captures, saved RGB data,
exact creative picking, real filter controls and authoritative crafting. The
[world capture](rgb/rgb-world-filter-merger.png) was visually inspected and shows
the open frames and real shafts. Its frontal angle partly hides the side-entry
tongue; the exact contact reach, carrying plane, mesh indices and bounded cache
are additionally checked by geometry regressions, not claimed as a clear
close-up photo. The headless dedicated server starts without either display
environment and completes its smoke test with 45 sections.

Raw before, driver-experiment, final, native-Wayland, loading, RGB, headless and
formal-test logs are adjacent to this report. Distribution CSV seconds were
recorded during compilation and must not be interpreted as CPU performance
comparisons. No authentication profiles or player save files are archived.

## Limits

Short samples cannot establish a universal minimum-FPS guarantee or long-term
resource stability. The isolated clear experiment, final native-Wayland run and
20 m/s new-world run still include frames below 32 FPS. No new 10/15/30-minute
test is run. CPU/render wall scopes overlap
and must not be added as serial GPU time. Existing world shapes are protected;
the new biome/river distribution is not applied to old worlds.
