# River Transitions, Auto-Climb And Configuration Paths

## Changes

- Retain fractional drainage heights. Interpolate descents below five meters
  instead of introducing an abrupt step at every eight-meter path sample.
- Keep a vertical waterfall only for a cliff of at least five blocks. Place the
  cliff on an integer world edge. Surface corner heights come from the same
  padded columns on both sides of horizontal section boundaries.
- Offset a waterfall sheet four centimeters into its downstream cell and join
  its top to the original surface with a lip. This removes coplanar terrain/water
  triangle artifacts without separating section edges. Clip falls vertically;
  retain the lip when its crest is exactly at a vertical section boundary.
- Carve rounded beds and wider graded banks into existing terrain. Do not raise
  retaining walls. Vary channel width continuously and blend the terminal pool.
- Use the geometry normal, plus animated wave normals, in the existing water
  shader. Reduce waterfall foam opacity. Existing depth absorption, SSR,
  transmission, underwater environment and water-quality controls remain.
- Add a TiltUI Auto-Climb switch as the first Controls row. Default false,
  including old controls files. Existing Apply, Reset, discard confirmation,
  validation and TOML persistence apply to the new boolean.
- Carry the boolean in movement packets and replay inputs. Authority and local
  prediction use the same controller. Deliberate swimming shore jumps remain
  available independently of automatic land stepping. Protocol version is 6;
  update and restart both endpoints together.
- Move the existing client.toml, server.toml and world.toml, preserving their
  contents, from config/ to configs/. Remove the old directory. The default
  configuration root is now configs/ for client, server and menu preferences.
  TERRA_CONFIG_DIR overrides that root; TERRA_MENU_CONFIG_DIR remains an optional
  menu-only override. Existing graphics, controls, profile and servers files are
  not overwritten by this move.

## Compatibility And Limits

No dependency, seed, block ID, world ID, world-format or generator-version change.
Worlds with rivers disabled retain the old terrain path. Corrected river terrain
does change the generated base of existing worlds with rivers enabled; their
journal edits stay at their original coordinates. Use a new world to compare
the new banks without old edits. No destructive save migration is performed.

These are still static generated streams, not a fluid solver, erosion system or
terrain-aware global drainage network. Waterfalls use a translucent animated
sheet, not volumetric spray. No waterfall force, sound or damage was added.
Full art-direction approval and long-running multi-machine tests remain open.

## Validation

The workspace checks are recorded under logs/. New regression coverage includes
the exact five-block threshold, slopes, negative section seams, valid normals
and indices, vertical clipping and boundary lips, non-raised river banks,
default-off stepping, enabled stepping/ceiling checks, controls Apply/Reset and
reload, packet-to-authority/replay conversion and the existing swimming exit.

Native Vulkan captures were inspected for the revised spawn-adjacent river,
an actual greater-than-five-block waterfall and the Controls switch. The menu
fixture also toggles Auto-Climb through TiltUI, verifies saved/reloaded state,
then disables it before testing the integrated session. Native physics and
headless no-display server smoke tests were run. The separate dedicated-server
dependency tree has no bevy_render, bevy_winit, wgpu or TiltUI dependency.

The waterfall fixture locates seed 42042 at X=95.5, Z=404.5, with water heights
43.086174 and 48.364502 (about 5.28 blocks of fall). The original spawn-adjacent
reach near X=62.5, Z=24.5 now flows continuously instead of using small waterfalls.

Final checks: cargo fmt --all --check, cargo check --locked --workspace -j 2,
cargo test --locked --workspace -j 2 (248 passed, one existing soak test ignored),
cargo clippy --locked --workspace --all-targets -j 2 -- -D warnings and
cargo build --locked --workspace -j 2 all succeeded. The separate
cargo build --locked -p terra-server -j 2 also succeeded. The long network soak
was not rerun in this change.

The 1280x800 native menu/session suite passed, including the new controls
switch, actual integrated host, rebound movement, authoritative edits,
save/restart and journal reload. The 960x640 suite passed under X11, and that
Controls layout was also inspected. A first small Wayland run failed at the
world-mode dropdown pointer selection before reaching Controls. It is archived
as a failed test, not counted as a passed Wayland suite. The backend-specific
pointer issue remains open; no claim that the small Wayland view is fully
validated is made.

## Performance

The existing run-hydrology-suite.sh is used unchanged: sequential isolated
18-second windows at physical 1920x1200, VSync off, identical worker budgets,
view radius 16/3, no screenshots and GPU pass diagnostics. Forest and controller
fixtures retain their former cameras. The river camera now locates a real
five-block waterfall and is not comparable with the previous three-block-fall
camera. Screenshot-overlay FPS is not a benchmark.

Final measured results (frame times in ms, resident memory in KiB):

| Fixture | Average FPS | p95 | p99 | Initial ready s | Resident KiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| Forest 1 | 112.750 | 10.915 | 14.697 | 3.322 | 752668 |
| Forest 2 | 115.693 | 10.670 | 14.423 | 3.290 | 740424 |
| Controller offline | 142.628 | 9.293 | 10.847 | 3.155 | 772020 |
| Controller integrated | 142.896 | 8.688 | 9.640 | 3.105 | 770076 |
| River/fall 1 | 146.009 | 8.739 | 10.136 | 2.990 | 758324 |
| River/fall 2 | 145.843 | 8.866 | 10.390 | 2.937 | 753676 |

The previous hydrology run recorded forest 119.448/119.194 FPS, p95
9.993/9.924 ms and p99 13.360/11.033 ms. The final forest samples here are
approximately 2.9-5.6% lower FPS, not an improvement. Geometry counts remain
identical: 7704 loaded sections, 3531 rendered sections, 9171 meshes,
89866 represented props and 1277 prop batches. No new forest draw calls or
per-prop entities were introduced. These are sequential historical runs, not
interleaved baseline/patch runs; the cause of the frame-time difference is not
isolated and should not be asserted. The earlier controller averages were
147.598/147.265 FPS, also above the current measurements. No performance
improvement or universal FPS guarantee is claimed.

The new river scene has 7633 loaded / 4351 rendered sections, 9075 mesh assets,
61042 represented props, 1222 prop batches and 85 water tiles. The instrumented
transmission pass averages 0.3254/0.3262 ms (p95 0.3332/0.3340 ms,
p99 0.3381/0.3432 ms). This is not total GPU frame time or a draw-call count.
Its different camera/terrain prevents a before/after FPS comparison against
the previous river fixture.

Controller CPU mean/max is 0.0501/0.2074 ms offline and 0.0351/0.4907 ms
integrated. Peak process RSS, user/system CPU seconds and composition-job
timings are retained in each benchmark log. The intermediate six-run suite
before the exact-Y16 lip regression case is retained separately and not mixed
into the final table. The original 4375-section CPU-only generation benchmark
was not rerun in this change; these native initial-ready timings include
streaming, mesh composition and uploads, not just generation CPU time.

Captures: [river](captures/river-overview.png),
[waterfall](captures/river-waterfall.png),
[underwater](captures/river-underwater.png),
[controls](captures/controls.png),
[small X11 controls](captures/controls-small-x11.png).
