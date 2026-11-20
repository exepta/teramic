# Model quality pass — 2026-11-13

The subsequent [voxel biome prop revision](../voxel_props/README.md) replaces
the curved Spirit Blossom props described below with stepped cube silhouettes,
as requested by the user. The measurements below describe this earlier pass;
the linked revision records the current geometry and counts for those props.

This pass audits the complete shipped GLB/glTF inventory, including the imported
terrain models and the deterministic test pedestal. It also reviews the runtime
player, resident, animal, armor, tool and botanical model builders. It is **not**
a claim that every existing asset was replaced with a new sculpt.

## Implemented

- 22 existing furniture and decorative prop files receive closed beveled
  edges on 335 previously square components. Their triangle count changes from
  **26,992 to 37,712**. The changes improve silhouettes and normal lighting;
  they do not subdivide coplanar faces. Bounds, materials, texture bindings,
  scene names, transforms and UV placement are retained. Replaced vertices are
  removed from the live primitive so they consume no GPU vertex-buffer space.
- Eight Spirit Blossom prop scenes are rebuilt. Flowers have closed curved
  petals, and lotus/lily leaves have one continuous 24-segment silhouette and a
  notch, replacing stair-step strips. Seven scenes (316–322) gain new geometry;
  the regenerated shrine (323) keeps its existing carved framing. The six scenes
  containing ellipsoid petals now use shared indexed vertices with analytic
  smooth normals only on these untextured petals: **29,868 → 9,996 vertices**
  (−66.5%), with exactly the same **13,340 triangles**, Float32 triangle positions,
  material assignments and bounds. Texture-bearing geometry is excluded. This
  deliberately smooths petal lighting; it is not a lossless normal rewrite.
- Shared rounded animal body pieces at least 35 cm long on their largest axis
  use 12 outline samples instead of eight: **92 instead of 60 triangles** per
  piece. Smaller repeated tufts, toes and eyelids retain their original eight
  outline samples and compact quad caps (60 triangles / 120 vertices). This refines large silhouettes in
  mammals, aquatic creatures, bees, horses and Spirit creatures. Already-authored
  player/resident/armor forms retain their explicit bevels. Small planar cloth,
  fur, scale and bark tiles retain their original geometry, neutral colors,
  spacing and animation bindings instead of multiplying dense detail costs.
- 76 new cemetery blocks (stable IDs 324–399) have separate JSON, PNG and GLB
  assets. The new models total **33,712 triangles**, with at most **3,468** in one
  model. Sculpted arches, columns, reliefs, skulls, candles, foliage and crystals
  use closed surfaces. Their geometry and collision profiles fit one voxel.

The final static inventory is **279 model files / 320,632 triangles**, comprising
203 previously present files (including one test fixture) and 76 new files.
Cube GLBs include authoring references: normal solid terrain rendering still
uses the shared closed cube surface. These counts exclude runtime actors.
[`report.json`](report.json) contains the path, before/after counts and decision
for every static asset. Its before/after comparison concerns the conservative
bevel pass, after new cemetery and rebuilt Spirit assets were generated.
Thus **309,913 → 320,632 triangles (+3.46%) measures only that later bevel pass**;
it is not a total before/after comparison with the native performance baseline.
The separate flower vertex comparison is recorded in
`tools/benchmarks/results/spirit-world-polish-2026-11-13/flower-index-audit.json`.

## Audited groups deliberately preserved

| Group | Reason |
| --- | --- |
| Imported Fiverr terrain/face groups, ordinary cube references | Keep neighbor-face culling, matching chunk seams and original UV/topology. Rounding these would reopen cracks. |
| Existing castle masonry, arches and heraldry | Already have chamfers, shaped arches and layered reliefs; further global subdivision adds no silhouette. |
| Existing profession stations | Already authored with beveled structural parts, curved details and separate material groups. |
| Existing detailed locomotive assemblies, wheels, rods and boilers | Already contain shaped wheels, curved boiler rings and beveled metalwork; node names and original connections are retained. The cab door/window components receive the safe pass; technical valve/profile geometry is preserved. |
| Rail profiles, machinery, gears, shafts, hydraulics and water wheel | Preserve exact renderer/server rail profiles, open shafts, housing coverage, conveyor contact pads and UV-derived tangent frames. Existing authored curves and bevels remain. All 31 mechanical files initially considered for generic bevels were restored from their retained original accessors after full regression tests exposed those contracts; no committed revision replaced user data. |
| Armor and tools | Already use the shared bevel builder, tapered blade profiles, grips, rivets and material-specific details. |
| Runtime grass, ordinary flowers, saplings, mushrooms, crops and reef props | Already have bounded near/middle/far detail, rooted deformation and precise picking geometry. Dense vegetation remains within its existing budgets; square botanical styling and open gaps are preserved. |
| Very thin surfaces and small facial details | Preserve readable details without raising dense-scene geometry costs. |

## Regeneration and checks

From the repository root:

```sh
python3 tools/assets/build_spirit_blossom.py
# To update only the six flower GLBs without touching JSON/PNG/recipes:
python3 tools/assets/build_spirit_blossom.py --flower-models-only
python3 tools/assets/build_cemetery.py
python3 tools/assets/polish_models.py --apply
python3 tools/assets/repair_models.py assets/terra/blocks/models/rail_switch_left.gltf
python3 -m unittest discover -s tools/assets/model_polish -v
```

Run the last polish command after other asset generators too. It does not touch
skin/animation data, non-triangle primitives, normalized integer attributes,
face-group terrain, exact mechanical profiles, authored tangent frames, already-rounded shapes or previously polished files.
Original binary payloads/accessors are retained when writing appended geometry;
this avoids invalidating embedded textures and unrelated accessor references.
A repeated revised polish/repair pass is byte-identical. The report records the
final audited result; new assets and tooltip text remain player-facing.

A reviewed duplicate-corner triangle in `rail_switch_left.gltf` is removed by
the targeted repair command, preserving all positions, UVs, materials and nodes.
The ornate chest includes eight stepped lid collider strips, so feet rest on its
visible lid rather than intersecting it. Only block 377 can be regenerated with
`python3 tools/assets/build_cemetery.py --only 377`.

The Python tests check stable IDs, finite unit normals, non-degenerate triangles,
texture existence, one-voxel geometry/colliders, shadow-free light definitions,
UV-safe bevel projection, preserved bounds, and rejection of thin/already-beveled
surfaces. The revised pass also checks all referenced polished vertices, a lower bound on
triangle area, preserved collider extrema and exact mechanical exclusions. Rust
tests cover shared actor budgets and outward-facing new surfaces;
workspace execution is coordinated by the main implementation task.

## Scope limits

The reference images guide the colors and forms; these are original procedural
models, not extracted models or exact sculpted reproductions. The pressure plate,
lever, wall switch, gate and trap models are decorative at this stage. The mist
block is a translucent decoration, not another expensive volumetric fog source.
The ornate chest provides real single-cell storage, hopper import/export and
Kingdom construction stock. It does not auto-link to a workbench, form a double
chest or become train cargo. Further
artist-authored high detail sculpting, texture painting and new dungeon trap
mechanics remain independent work.
