# Player, Settings And Surface Appearance

Native Linux/Vulkan runs on 2026-10-14, AMD RX 6950 XT, RADV Mesa 26.2.4.
Seed 42042, generator version 5, existing 16-m sections and block IDs unchanged.
All runs were serial, without a concurrent build or second game benchmark.

## Functional Runs

- `physics.log`: native fall, landing, 1.335-m jump, wall collision, menu input
  suppression and falling after support removal passed. 74 local colliders;
  7.745 seconds including loading. The fixture edits only this process's session.
- `appearance.log`: dry/rain/snow, local charring/wetting/reset and real section
  eviction/reload passed in 10.332 seconds. Global weather retained terrain mesh
  and material handles. Local edits rebuilt only the owner; logical mesh/prop
  epochs stayed unchanged. Seven images show the resulting states.
- `settings.log`: real TiltUI event handlers, actual numeric/checkbox/selection
  component values, FOV/view-distance/VSync preview, input blocking, rollback,
  apply/reset and an isolated TOML save/load roundtrip passed. Captures cover
  1280x800 and 640x480 logical windows (1.5 display scale).
- `server.log`: server started and generated 27 sections without DISPLAY or
  WAYLAND_DISPLAY. Its normal dependency tree contains no bevy_render,
  bevy_winit, bevy_ui, wgpu or TiltUI.

The settings smoke dispatches native UI events but does not itself simulate
pointer dragging. Screenshots from its first run exposed incorrect TiltUI
bindings, low-contrast theme text and clipped values. Those were fixed and the
logged run also checks the real widget state, not just GameConfig.

The additional physical-pointer attempt is **incomplete**, not passed. XWayland
captures stayed stale and the window geometry changed during the attempt, so a
drag/dropdown/save result could not be established reliably. The isolated temporary
config was not saved, the user's config was untouched, and the process was stopped.
See `settings-pointer-status.txt`. The theme's remaining purple slider gradient
was also removed; this change is included in the final client build.

## Scope

Rain/snow controls alter material appearance, not precipitation or snow volume.
Local wetness/charring is session state, not a fire simulation. The world remains
editable at one-meter resolution. No claim of matching the reference image
pixel-for-pixel or of a performance improvement follows from these tests.

Ten obsolete Cube GLBs were deleted (15,875,208 file bytes). They were already
unused at runtime, so this cleanup does not imply fewer runtime draws. Special
authored models continue through the custom GLTF/GLB path.

## Workspace Verification

All passed with the pinned lockfile: `cargo fmt --all --check`,
`cargo check --locked --workspace`, `cargo test --locked --workspace` (171 tests),
`cargo clippy --locked --workspace --all-targets -- -D warnings`, and
`cargo build --locked --workspace`. Logs are stored beside this file.
Additional native prop removal, block mining and water smoke tests passed.

## CPU Generation

Same seed/version, four workers, 625 columns / 4,375 sections, one warmup and
five runs. Previous values are the archived `reference-2026-10-14` run.

| Worker throughput | Previous mean | Current mean |
|---|---:|---:|
| Headless generation | 0.203699 s | 0.204690 s |
| Render preparation | 0.368271 s | 0.363971 s |

Render preparation includes padded voxels, greedy meshing, props and water,
not GPU upload or renderer-specific mesh composition. Checksums remain
22,955,883, with 50,506 generated props and 450,427 greedy quads. These small
timing differences are not evidence of a meaningful speedup.

The 60-Hz ECS-paced streaming benchmark is unchanged to the reported precision:
cold 2.626 s, one-column move 0.117 s, eviction 4.934 s, reload 2.626 s.
Peak RSS: 166,284 KiB versus 165,772 KiB previously. Raw logs: `workers.log`
and `streaming.log`. Neither benchmark exercises the renderer or controller.

## Native Flight Comparison

48-second out-and-back flight, VSync, 1920x1200 physical pixels, render diagnostics
enabled, identical camera path. Previous run is
`../seam-fix-2026-10-14/terra-seam-flight.log`; current is `flight-final.log`.

| Metric | Previous | Current |
|---|---:|---:|
| Mean FPS | 60.005 | 59.997 |
| p95 wall-frame time | 17.584 ms | 18.286 ms |
| p99 wall-frame time | 18.250 ms | 29.424 ms |
| Initial ready | 3.308 s | 3.310 s |
| Terrain worker spans | 3.037 s | 3.128 s |
| Prop worker spans | 1.042 s | 1.147 s |
| End-of-run RSS | 655628 KiB | 668812 KiB |
| Loaded/rendered sections | 4404/1988 | 4404/1988 |
| Mesh assets/visible mesh entities | 5181/1672 | 5181/1672 |
| Represented props/batches | 50605/721 | 50605/721 |
| Completed LOD replacements | 12997 | 12997 |

Two interim runs (`flight.log`, `flight-repeat.log`) measured p99 29.116 and
28.415 ms. The tail-frame regression remains unresolved, not dismissed as an
outlier. Section-shared biome samples reduced measured prop composition versus
these interim runs (1.298/1.318 s), but did not remove the frame-time spikes.
No isolated causal attribution to physics, renderer or streaming is claimed.

Main opaque GPU pass: previous mean 1.239 ms, current 1.244 ms (p95 1.394,
p99 1.654 ms). Current early prepass mean 0.501 ms. Passes are not summed into
a claimed total GPU time. Exact draw calls and individually visible props are
not instrumented. End RSS is the client's /proc reading; the wrapper's getrusage
peak is retained separately in raw JSON, not substituted for that metric.

Reproduce after `cargo build --locked --workspace`:

```bash
TERRA_BENCH_SECONDS=48 TERRA_RENDER_DIAGNOSTICS=1 TERRA_SCREENSHOT=/tmp/terra-flight.png python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-lod
python3 tools/benchmarks/measure.py target/debug/examples/worker_benchmark
python3 tools/benchmarks/measure.py target/debug/examples/streaming_benchmark
```

Build the two examples first with `cargo build --locked -p terra-world
--features bevy --example worker_benchmark --example streaming_benchmark`.
