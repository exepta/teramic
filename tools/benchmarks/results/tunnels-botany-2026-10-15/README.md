# Connected River Roofs And Botanical Detail

## Changes

The eroded exterior height and the independently subtracted river cavity could
leave detached roof islands. General cave noise could also sever a supported
arch's underside. Both failures were reproduced before correction, at
`[551,21,-1112]` and `[55,64,-1081]` respectively.

Roof spans now require a voxel-face-connected horizontal path to a bank. This
is a column-level check with early exits, capped at 64 columns per direction,
not a production voxel flood fill. Unsupported roofs are removed before surface
materials, trees and props are placed. The explicit river roof is protected from
the second cave subtraction; ordinary caves below the bed/outside the corridor
retain their field. Connected overhangs remain, rather than flattening all roofs.
The regression flood starts only in real bed/bank terrain, never arbitrary
boundary voxels. Eleven regions with caves on/off must retain at least sixteen
roof voxels each, so removing every roof cannot pass.

Grass keeps the existing 14/8/4-blade templates but now has blunt square ends,
slender 0.034-0.052 m widths and varied heights. Vertex counts remain 280/160/80.
Flowers have cupped, six-petal or layered four-petal forms with slim stems,
staggered leaves, raised centers and small calyx details. Near flower tufts stay
within 480 vertices; Middle/Far costs remain 144/48. All variants, existing
colors, IDs, picking bounds, persistence and section batching remain in use.
No per-blade entities or new rendering pipeline were introduced.

`TODO.md` now contains only open work, including the requested block variants,
breaking/building animations, general cave entrances, biome work, quiet chunk
logging and the F1/biome overlay. These roadmap entries are not falsely marked
implemented. `PATCH.md` archives completed work in short prose without checkboxes.

## Validation

All five commands passed on the final code:

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

306 tests passed, zero failed, one existing network soak was ignored. Tests
cover all 256 prop variants and LODs, finite vertices, valid indices/normals,
rotated picking bounds, deterministic output and geometry budgets. Existing
seam, persistence, networking and fluid tests remain enabled.

Separate `cargo build -p terra-server` and a smoke start with DISPLAY and
WAYLAND_DISPLAY unset passed. Logs are in `native/`.

Native Vulkan captures on RX 6950 XT / Mesa 26.2.4 were inspected for grass,
flowers, the additional flower close-up, tunnel and 30+ m waterfall. The river
fixtures also verify underwater fog and restoration after surfacing. Screenshots
are in `captures/`, baseline plant screenshots in `before/`. The corrected portal
fixture moves three meters to the first genuinely supported roof, so its view
is not a pixel-identical A/B camera. Visual-smoke HUD FPS is not a benchmark.

## Measurements

Twelve isolated native runs, six before/six after, each with an 18-second sample.
Identical camera anchors, seed 42042, 1920x1200 physical pixels, VSync off,
FOV 90, streaming radius 16/3 and job budgets. No concurrent compilation or
second client during measured runs. Config snapshots and complete logs are
included. River geometry intentionally changes, despite the fixed benchmark view.

| View | FPS Before / After | p95 ms Before / After | p99 ms Before / After |
|---|---:|---:|---:|
| Forest 1 | 110.378 / 110.734 | 11.036 / 10.303 | 14.951 / 14.417 |
| Forest 2 | 109.435 / 110.761 | 11.029 / 10.463 | 15.222 / 14.540 |
| Offline | 141.900 / 144.095 | 8.993 / 8.503 | 10.203 / 9.800 |
| Integrated | 141.497 / 143.952 | 8.861 / 8.232 | 11.577 / 9.164 |
| Fixed river 1 | 144.061 / 145.695 | 8.325 / 8.096 | 10.050 / 8.867 |
| Fixed river 2 | 143.543 / 145.427 | 8.478 / 8.248 | 10.823 / 10.340 |

Forest mean 109.906 -> 110.747 FPS; fixed-river mean 143.802 -> 145.561 FPS.
No additional FPS regression appears in these short views. The small differences
are not a demonstrated general speedup, nor do they reverse the earlier measured
wind-foliage regression documented in phase 20.

| First Run | Initial Load s Before / After | Process RSS KiB Before / After | Prop Worker s Before / After |
|---|---:|---:|---:|
| Forest | 3.359 / 3.339 | 865788 / 853860 | 0.257 / 0.252 |
| Offline | 3.129 / 3.075 | 825072 / 819196 | 0.244 / 0.237 |
| Integrated | 3.116 / 3.181 | 837056 / 824792 | 0.232 / 0.239 |
| Fixed river | 3.060 / 3.025 | 809568 / 816296 | 0.178 / 0.187 |

Loaded section counts remain 7704 / 7623 / 7635 for forest / spawn / fixed river;
mesh assets remain 9313 / 9316 / 9085 and prop batches 1277 / 1248 / 1231.
Represented props remain 89866 / 86727 in forest/spawn; corrected river terrain
changes its count from 63180 to 63169. These counts are not GPU draw-call counts.
Worker seconds are summed job time, not wall latency. Individual instrumented
GPU passes are in the logs; no total GPU-frame measurement is inferred. The
separate 4375-section CPU example was not rerun for this step.

## Compatibility And Limits

Seeds, block/world IDs, dependencies and generator version 5 remain unchanged.
Existing `rivers = true` worlds regenerate corrected base terrain, while edits
stay at their coordinates. Player-built unsupported blocks are not collapsed,
and saved edits are not erased. Worlds without rivers retain their base terrain.
The eleven regression regions are bounded evidence, not an exhaustive proof for
every seed and coordinate. Long streaming/flood tests and art-direction approval
remain open; this work does not implement the new blocks/biomes in TODO.

## Reproduction

```bash
TERRA_SCREENSHOT=/tmp/botanical.png target/debug/terra-client --smoke-test-botanical
TERRA_RIVER_TUNNEL=1 TERRA_CAPTURE_DIR=/tmp/tunnel target/debug/terra-client --smoke-test-rivers
TERRA_RIVER_MIN_FALL=30 TERRA_CAPTURE_DIR=/tmp/fall target/debug/terra-client --smoke-test-rivers
TERRA_RIVER_BASELINE_VIEW=1 bash tools/benchmarks/run-hydrology-suite.sh /tmp/benchmarks
env -u DISPLAY -u WAYLAND_DISPLAY target/debug/terra-server --bind 127.0.0.1:0 --no-save --smoke-test
```
