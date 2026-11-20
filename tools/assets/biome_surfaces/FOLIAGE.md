# Botanical foliage with real gaps

`foliage.py` constructs twenty leaf/blossom PNGs directly at 512 × 512 pixels.
It does not upscale old art. Irregular sprays of overlapping blades replace the
former regular 3 × 3 arrangement and opaque painted background. Gaps are authored
by the actual silhouettes: no random holes are punched through leaf interiors.

| IDs | Botanical profile |
| --- | --- |
| 7, 8 | Lobed oak leaves, green and autumn orange |
| 9 | Light yellow-green birch blades with serrated margins |
| 132, 133 | Pink/white cherry flowers with rounded notched petals and stamens |
| 195 | Pointed green apple leaves |
| 310, 311, 312 | Pink, cyan/pearl-white and lavender Spirit blossoms |
| 406 | Bright crimson-pink Blood Forest leaves |

The existing material IDs, side material names, PNG paths and side/top bindings
remain unchanged. Each type uses two images in
`assets/terra/blocks/textures/foliage/`, white base color, matte roughness `0.94`
and low reflectance `0.08`; no duplicate color pattern, light or emission.
The subsequent `leaf_clusters.py` pass adds eleven separate motif sheets:
ten broadleaf/blossom families and conifer fronds/cones for Spruce/Fir.
Each uses an explicitly named `foliage_cluster` material binding, separate from
the unchanged face materials. Each 512px image contains four complete motifs
in a 2×2 layout, with transparent borders around each cell.
All 31 images total 31 MiB of uncompressed base pixels, approximately 41.3 MiB
including complete mip chains, before renderer-specific copies.

## Far/Low face-image contract

- The entire face maps to UV `[0,1]²`. Main leaves no longer occupy fixed grid
  centres; geometry must not assume nine centre coordinates.
- Side U is horizontal and V follows world Y. Leaf tips predominantly face
  smaller V; the raw side PNG therefore appears inverted relative to world Y.
  Top textures use differently oriented branching groups.
- Face-material order remains `[side, side, side, top, side, side]`.
- Between 65% and 75% of pixels pass alpha 128; the remaining 25–35% form real
  gaps between leaves. Blade interiors are alpha 255. Only antialiased silhouette
  edges have intermediate alpha. The outer two pixels are completely transparent;
  no leaf or blossom is cut off by a sheet boundary.
- Bounded irregular spacing places complete blades inside the image. The full
  rotated silhouette, curved spine and antialias fringe determine its bounds;
  an entire five-petal flower is moved together. This translates elements without
  cropping, fading tips or wrapping them to the opposite side. Use the complete
  sheet UV range, optionally mirrored/rotated, without shifted UV crops.
  Cluster size, direction, pigmentation and leaf count vary. Fine branching
  veins and curled margins follow each blade's coordinates, including the
  asymmetric lobes/serrations; petal folds and pigment vary individually.
- Each blade has only a short attached petiole, under 2% of sheet width.
  There are no long exposed lines between independently positioned blades or
  their original cluster centres; flower stamens remain intact.
- Whole botanical groups are drawn until the coverage target is reached. There
  is no later opacity noise, grid mask or alpha reduction inside the artwork.
- Renderer geometry, mask threshold, back-face policy, LOD and wind remain the
  responsibility of the foliage renderer. These PNGs require alpha masking;
  rendering them as opaque would reveal the stored RGB of transparent pixels.

## Regeneration and verification

Near/middle geometry selects whole cells from each additional motif sheet.
Two cells supply distinct individual leaves, the other two supply separate
flowers, apples or needle/cone motifs. Twelve folded leaves and two forward
ornaments overlap at different depths on each exposed face. Near uses a
seven-vertex convex hexagon per motif; Middle uses four vertices. Motif anchors
and UV cells agree across LODs, while the polygon outlines differ. Coverage is
checked against the final indexed mesh and real PNG alpha from several views.
The renderer uses inset UVs and transparent cell borders to prevent neighboring
motifs from bleeding through. Decorative apples do not add extra harvestable
items; existing fruit gameplay remains unchanged. `leaf_clusters.py` regenerates
the eleven sheets using the same botanical drawing helpers as the face images.

```bash
# Preview without changing active assets:
python3 tools/assets/biome_surfaces/foliage.py --profile oak --output /tmp/terra-foliage-preview
# Generate and bind all ten existing foliage types:
python3 tools/assets/biome_surfaces/foliage.py --activate
# Regenerate the eleven detailed four-motif sheets and material bindings:
python3 tools/assets/biome_surfaces/leaf_clusters.py --activate
python3 -m unittest discover -s tools/assets/biome_surfaces -v
python3 -m unittest discover -s tools/assets/blood_forest -v
```

Spirit/Blood full and `--textures-only` generators refresh their respective
foliage profiles afterwards. `--prop-models-only` leaves foliage unchanged.
Portable authoring GLBs remain unchanged: live leaf rendering uses these JSON
PNG bindings and renderer-generated meshes.

Tests inspect shipped PNGs and definitions: native dimensions and fine detail,
25–35% transparent area, opaque interiors, fully transparent two-pixel borders, broad empty
regions rather than alpha speckles, absence of a repeated third-tile mask,
correct side/top bindings, distinct palettes and Blood Forest readability.
Rasterized leaf/flower area comparisons also verify that translating elements
against sheet corners preserves the complete silhouette rather than clipping it.
Color/readability measurements exclude transparent pixels. Nonfoliage terrain
textures must still be completely opaque. Native silhouettes, illumination,
LOD and runtime cost require the separate client gallery and world captures;
Python asset checks alone do not establish those runtime properties.

Motif tests additionally verify exact reproduction of all eleven shipped PNGs,
transparent cell borders, opaque interiors, complete silhouettes inside their
hexagonal geometry, distinct ornament cells and preserved side/top paths. The
full-face coverage percentages above apply to the original 20 images, not the
motif sheets. The folded-leaf geometry uses bounded cached buffers, keeps
interior-face culling and wind/ownership behavior, and creates no entity per leaf.
