# Spirit world and model polish — acceptance evidence

## Scope

The implemented behavior is documented in [spirit-world-polish.md](../../../../docs/spirit-world-polish.md).
All native runs use disposable directories under `target/`; the player's `worlds/Test`
was checked by SHA-256 (12 files, unchanged). No source game save was opened for writing.

- `model-audit.json`: 279 static files, including one fixture; 22 beveled existing assets,
  rounded creature silhouette refinements, 76 new cemetery assets and one repaired legacy rail triangle.
- `model-tests-indexed-flowers.txt`: ten passing geometry, bounds, texture and collision tests.
- `flower-index-audit.json`: six flower models with 29,868 → 9,996 vertices;
  identical 13,340 triangles and material assignments, smooth analytic normals.
- `world-validation-wetlands.txt`: actual wet-column comparison over 18 Spirit regions / three seeds,
  +39.00% water surface and +20.000% dry-core relief.
- `world-validation-rivers.txt`: 50 hydrology regressions, including supported banks and connected watercourses.
- `world-validation-graves.txt`: five grave-route regressions, including real collision treads and rising approach terrain.
- `water/verification.json`: actual swamp and cherry brook water inspected in native screenshots.
- `grave-sites.jsonl`: genuine generated cemetery plus Knight, Priestess and Scholar tombs.
- `atmosphere/verification.json`: native Spirit/Cherry screenshots, animated density drift,
  one Spirit volume, six ray samples, 484-byte terrain field; no fabricated fixture lights.
- `localization.txt`: complete generated German/English catalogs without pending entries.
- `server-final.txt`: isolated dedicated server startup, world initialization and shutdown.

## Interpretation

| Model family | Actual treatment |
| --- | --- |
| Furniture and props | 22 existing models, 335 beveled components; beds, doors, windows, chest, workbench, furnace, torch, chimneys and locomotive door/window. |
| Spirit plants and lanterns | Seven existing models refined; closed petals and continuous lily-pad meshes. Shrine regenerated without a geometric redesign. |
| Rounded creatures | Larger body parts use 12 rather than eight contour points. Repeated details below 35 cm retain the compact original shape and existing vertex budgets. |
| Grave architecture and decoration | 76 new individual GLB/JSON/texture definitions. |
| Terrain and machinery | Checked; exact seams, contact surfaces, openings and UVs preserved. One degenerate rail triangle repaired. |
| Players, residents, armor and other detailed models | Checked; their existing authored detail is preserved. This is not a claim that every model was newly sculpted. |

The 279 static files include one test fixture. Runtime-generated actors are additional
to this file count. The static triangle total is 320,632; the 22 beveled assets change
from 26,992 to 37,712 triangles. These before/after values describe the subsequent
bevel pass; they are not the original native benchmark's asset totals.

Biome screenshots are real generated scenes. Creature galleries use actual model and
animation code in explicit model-only fixtures; they do not replace authoritative AI tests.
The colored clay/masonry/props are original procedural assets guided by the references,
not extracted artwork or exact replicas of the concept illustrations.

The older baseline and final build change both geometry and rendering. The final-build
mist-on/mist-off pair in `perf/` isolates the current incremental fog cost. GPU
transparent-pass timings refer to that individual pass, not total frame GPU time.
CPU wall frames are reported separately and are not interchangeable with GPU timings.

One historical external-fixture test requires `FIVERR/VoxelGrassBlock glb file.glb`,
which is not present in this checkout. The full suite is run with that test explicitly
filtered by name; shipped GLB/glTF assets are checked by the other tests and native runs.

## Successful validation

| Check | Result and evidence |
| --- | --- |
| Full workspace tests | 5,046 passed, zero failed, ten repository-ignored, one named external fixture filtered; `tests-final.txt`, `tests-summary.json`. |
| Final crypt-light and capture changes | 212 client + 1,120 renderer tests passed; `tests-crypt-light.txt`. |
| Final indexed GLB assets | 54 Rust loading tests and ten Python geometry tests passed; `tests-final-flower-assets.txt`, `model-tests-indexed-flowers.txt`. |
| Client, server, building editor builds | All three succeeded; `builds-crypt-light.txt`. |
| Workspace check, all-target Clippy with `-D warnings`, formatting | Passed; `check-crypt-light.txt`, `clippy-crypt-light.txt`, `fmt-final.txt`. |
| Real generated grave sites | All four passed with eight captures, streamed voxel/loot checks and actual lighting snapshots; `native-graves-light-final.txt`, `graves/verification.json`. |
| Biome atmosphere and water | Spirit/Cherry animation and close views passed; `native-atmosphere-models-final.txt`, `atmosphere/verification.json`, `water/verification.json`. |
| Creature models | Nine captures, Spirit variants and 20 male/female residents in representative poses; `native-models-final.txt`, `models/verification.json`. |
| Standalone building editor | Actual placement/removal, catalog, JSON import/save/reload/overwrite and protected platform resize passed; `editor/native.log`, `editor/verification.json`, `editor-dpi-validation.txt`. |
| Headless server | Local UDP startup, generated world and orderly shutdown passed; `server-final.txt`. |
| Original Test save | All 12 files unchanged, no extra files; `original-world-verification.json`. |

The full workspace run precedes the final crypt-light values and capture diagnostics;
the affected client/renderer tests were rerun afterwards. The last asset-only flower
optimization was then checked against the real Rust loader, Python geometry tests and
fresh native captures. Tests listed in separate rows overlap and must not be added
together as a unique test count.

The editor requests 1440×900 logical pixels. Its ten captures are 2160×1350 on this
1.5× HiDPI desktop. The original verification script incorrectly required physical
1440×900; it now checks the exact aspect ratio, minimum size and consistent physical
dimensions. The corrected verifier passed on the same successful native run.

The dungeon light snapshots confirm 12/12 local lights, correctly adapted cave
exposure and effective sources on the current floor. Lanterns remain non-volumetric
and do not allocate shadow maps. `graves-before-light/` retains the earlier dark
interiors for comparison. Stair traversal is checked using actual collision shapes
in world tests; the native capture harness does not pretend to walk an avatar through
every staircase.

## Performance

AMD Radeon RX 6950 XT / RADV / Vulkan, physical 1920×1080, fixed revision-one world,
seed 42042, paused 10:00, fixed camera, view distance five, identical graphics and
water settings. Each run samples 30 seconds without concurrent Cargo or another
native test application. Final ON/OFF has identical 1,089 loaded sections, 455
rendered sections, 1,102 props, four water surfaces and eight unshadowed lights.

| Measurement | Original | Final mist on | Final mist off |
| --- | ---: | ---: | ---: |
| Transparent GPU pass mean | 0.27390 ms | 0.08778 ms | 0.04934 ms |
| Transparent fragment invocations mean | 11.60 million | 3.59 million | 2.11 million |
| CPU wall-frame FPS | 179.68 | 221.57 | 228.59 |
| CPU frame p95 | 6.648 ms | 5.486 ms | 5.254 ms |

The transparent pass takes **67.95% less GPU time** and invokes **69.08% fewer
fragments** than the original. Its final ON-minus-OFF mean difference is approximately
**0.03844 ms**, an estimate of incremental fog work in that pass, not a dedicated
fog timer or total GPU-frame latency. The old fog was already batched; this is not a
claim of eliminating 77 draw calls.

The FPS increase is an observation for this fixed scene with both fog and model
changes, not a general FPS promise or an improvement attributable to fog alone.
Final ON/OFF opaque/prepass geometry matches. The intermediate unoptimized flower
version in `perf-before-flower-indices/` had higher vertex costs; the final version
shares flower vertices with smooth normals while retaining all triangles. Raw GPU
diagnostics and summaries are in `before/`, `perf/` and `perf/summary.json`.

Earlier failed attempts are retained as diagnostics, including the water attribute
mismatch, outdated gallery assumptions, the editor's HiDPI assertion and the discarded
1079-pixel-high benchmark attempt. Only the successful runs above support acceptance.
