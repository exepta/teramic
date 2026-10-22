# Stone Strata HD

`stone-strata-hd.png` is a generated opaque RGB albedo, 1254 x 1254 pixels.
It replaces the shared stone host texture without new block IDs or GLB files.
The existing worker creates the complete mip chain, handles odd dimensions,
uses repeat addressing and anisotropic filtering, and releases CPU mip data.
Stone and the six ore host rocks share the same texture and PBR material.
This is an albedo texture, not a normal map or dynamic lighting bake.

Mode: built-in `image_gen` generation, not CLI/API fallback. Generated 2026-10-17.
No reference game asset was used as an edit target. The user's cave photograph
guided formation scale separately; it was not copied into this texture.

## Final Prompt

Use case: stylized-concept
Asset type: seamless repeating square albedo texture for natural stone voxel blocks in TERAMIC, a high quality stylized sandbox game.
Primary request: Generate one production-ready flat 2D stone surface texture, completely filling a square image edge to edge. Orthographic directly facing a continuous rocky surface, absolutely no perspective. Seamlessly tileable both horizontally and vertically. Hand-painted stylized angular mineral facets, irregular interconnected shallow hairline fractures, some slightly raised and depressed broad rock planes and small chipped edges. Rich but restrained small-scale material detail with a clear hierarchy of broad grey planes and fine grain. Comparable polished game-material craft, not a photograph and not Minecraft pixel art.
Color palette: neutral slightly warm medium greys, subtle variation between quartz-grey and warm grey. Matte dry stone. No blue tint, no liquid or wet shine. Uniform overall brightness so repeating large fields do not make a checkerboard.
Lighting: diffuse flat albedo only, no directional lighting or cast shadows baked in, no highlights, no gradient or vignette.
Constraints: single seamless material swatch, prefer 1024x1024, opaque. No image frame, no borders, no separate stones laid like masonry or paving, no text, no symbols, no ore, no moss, no scenery. Detail remains legible on a one-meter cube. Natural thin cracks, not thick black comic outlines.

The actual generator output is 1254 x 1254, not the preferred 1024 x 1024.
Repeat-compatible appearance is visually inspected in the client; the generated
art is not a mathematical guarantee of equal opposing edge texels.
