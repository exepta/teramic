# Block Reference Pass

## Implemented

- Shared 512 x 512 procedural surfaces for neutral turf, pebbled earth, weathered stone, granular sand, warm clay, gravel, pale structure stone and layered leaf coloration. Existing texture dimensions and mip chains are retained.
- Turf remains neutral before biome/weather tinting. Its side cap is 0.18 blocks high instead of 0.10, using the same closed-cube face topology and vertex count.
- Soil and grass sides share one material. All six ore host-rock materials continue sharing the ordinary stone material. Snow keeps its own material.
- Oak leaves use warmer lime tones; birch uses lighter yellow-green leaf patches. Existing spruce-specific dark vertex tinting remains in place. Porous alpha coverage, coverage-preserving mipmaps, wind and near-detail budgets are preserved.
- Two append-only cube materials: `42 = teramic::gravel`, `43 = teramic::structure_stone`. Existing IDs through Bedrock (41) are unchanged. Neither new block is inserted into world generation. Structure Stone is an ordinary full cube, not a non-occluding furniture `struct_block`.
- No per-block GLB, new entity type, tangent vertex stream or new per-cell geometry. New procedural texture CPU storage is released after render-world upload. Gravel and Structure Stone add two shared material definitions; they do not allocate a material per block.

## Native Inspection And Correction

The native material fixture now captures dedicated terrain, foliage and gravel/structure-stone views in addition to its existing all-block, glass, water and mushroom views. Loading diagnostics identify preview or section readiness stalls instead of silently waiting.

The first native screenshots exposed overly regular mortar lines on soil, grass, sand and gravel, and straight grooves on leaves. Those observations led to a second texture pass:

- Soil and turf no longer have masonry joints. Soil has jittered small pebbles and shallow tonal clumps; turf keeps neutral fine grass marks.
- Sand uses fine grain and sparse pores rather than a paving pattern.
- Gravel uses differently sized, offset octagonal chips with tonal edge shading.
- Canopy textures use overlapping, rotated lobed leaf marks rather than horizontal strips.
- Stone and clay panels use unequal widths and row heights with small chipped edges.

The corrected sampler was compiled independently and its repeating-coordinate, finite-color and neutral-turf test passed. A generated texture contact sheet was inspected after this correction. The final native fixture then completed successfully on 2026-10-18 and all seven captures were refreshed. The three focused views were inspected again:

- [Terrain close-up](blocks/materials-reference-terrain.png): soil and turf no longer read as tiny bricks; individual pebbles and a continuous grass cap are visible. Sand is fine-grained. Ordinary cube faces remain closed in this view; no missing-face holes were observed.
- [Foliage close-up](blocks/materials-reference-foliage.png): leaf markings replace the former straight grooves, with distinct green, autumn-orange and pale birch palettes. The existing broad cutout layers are still clearly visible and do not reproduce the reference's dense three-dimensional leaf lobes.
- [Gravel and Structure Stone](blocks/materials-gravel-structure-stone.png): both appended materials render correctly. Gravel is distinguishable from the pale, cracked structure block and no longer looks like small masonry tiles. Its individual chips are mainly texture-shaded, so their silhouette remains substantially flatter than the reference.

These are native rendered views, not mockups. They validate loading and the visible texture correction, but a still image does not validate temporal stability, all chunk boundaries or wind movement.

## Tests And Budget Protection

Focused repository tests cover stable IDs, cube material sharing, closed-shell coverage for both new blocks, unchanged grass-rim vertex counts, opaque terrain mip alpha, bounded leaf-mask coverage, periodic sampler coordinates and neutral turf color. Workspace build/test results are recorded in the main report, not inferred from the isolated sampler test.

Texture sampling occurs when the shared asset library loads, not per frame or while meshing each block. No frame-time improvement is attributed to the visual changes without measurement. The two new cube materials can add material batches in sections where players place them; their world-generation density is zero.

## Fidelity Limits

This is not a pixel-for-pixel recreation of the supplied studio-lit concept image. Its deeply sculpted pebbles, individually extruded leaf lobes and hanging turf edges are only partly represented by the current bounded relief meshes and texture shading. Existing leaf shells remain more planar/lattice-like than the concept's dense overlapping foliage. Texture correction improves the surface motifs, not that geometric limitation. No normal/parallax map or additional high-density displacement pass was added.

The reference has strong warm studio backlighting and a dark background; native captures use gameplay daylight. Lighting was not globally changed to imitate that presentation. The screenshot fixture demonstrates material loading and closed geometry, not reference-quality sign-off for a dense generated forest.
