# Voxel biome props

The Blood Forest props (412–421) and Spirit Blossom props (316–323) use
24 × 24 × 24 microvoxel sculptures inside one ordinary block. Petals, caps,
vines, roots, bones, lantern ornaments and arches have stepped silhouettes and
axis-aligned faces. This replaces the curved botanical meshes from the previous
model polish pass. Mobs and ordinary terrain cubes are not changed by this pass.

`geometry.py` owns one material per occupied microvoxel, removes all hidden
interior faces (including between different materials) and joins adjacent
coplanar faces with the same material. Thin twigs remain face-connected. Small
flower centres retain at least one cell. Only the outer surface is exported;
the cells do not become runtime entities, blocks or individual colliders.

The 18 exported models change from **21,471 to 18,968 vertices** and from
**23,193 to 9,484 triangles**. The largest model has **1,404 triangles**. The
combined GLBs shrink from **1,373,556 to 1,087,052 bytes**. Per-model counts are
in [report.json](report.json). Stepped diagonal roots need more faces than the
old smooth tubes, while most flowers and lanterns become substantially smaller.

IDs, block JSON, lighting definitions and the existing small set of structure
colliders remain unchanged. Material names and emission are retained. Mushroom
emission is exposed on the cap underside rather than buried inside an opaque
cap. All geometry stays within its original one-block placement footprint.

Regenerate models without rewriting textures, definitions or recipes:

```sh
python3 tools/assets/build_blood_forest.py --prop-models-only
python3 tools/assets/build_spirit_blossom.py --prop-models-only
python3 -m unittest discover -s tools/assets/voxel_props -v
python3 -m unittest discover -s tools/assets/blood_forest -v
python3 -m unittest discover -s tools/assets/model_polish -v
```

Tests inspect the shipped GLBs for grid-aligned geometry, axis-aligned unit
normals, valid tangent frames, outward winding, finite attributes, complete
indices, duplicate faces and strict individual/aggregate budgets. Helper tests
check material-independent interior-face removal, overlap union area, thin
branch connectivity, open arch interiors and visible mushroom emission.
