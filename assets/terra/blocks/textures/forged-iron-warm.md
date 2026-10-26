# Forged Iron Surface

`forged-iron-warm.png` is a shared albedo texture for the crusher and hand crank.
It is multiplied by the existing glTF material factors and uses the normal
terrain texture/mipmap loader. No emission or separate shader is involved.

Generated with the built-in image generation tool on 2026-10-22. The generated
square was retained at its native 1254 x 1254 resolution. Geometry, normals,
UVs and material factors are authored by `tools/assets/build_crusher.mjs`.

Prompt:

> Use case: stylized-concept. Asset type: seamless square albedo/base-color
> texture for a hand-painted fantasy voxel game's dark warm cast-iron machinery.
> Generate the material texture ONLY, a flat orthographic scan covering the
> entire square, no object, no scene, no perspective, no text, no borders. Very
> fine tiny rectangular and square hammered metal facets, subtle forged strata
> and short fine scratches, dark chocolate-gray iron with restrained warm
> copper-gray highlights and deep brown-gray crevices. Dense but subtle
> hand-painted blocky/pixel-style surface, similar to tiny carved voxel facets.
> The smallest facets occupy about 1/40 image width. Low contrast overall, avoid
> shiny chrome, blue hues, large cracks, bolts, ornamental motifs, baked
> directional lighting and large dark patches. Seamlessly tileable on all four
> sides. No specular bloom or glowing highlights. 1024x1024.

The material is a stylistic approximation, not an exact reproduction of the
reference image. The texture is not a normal map and does not replace the
actual hollow hopper geometry or articulated roller/crank meshes.

The first native shadow-side capture exposed an albedo that was too dark when
multiplied by the old brown material factors. A built-in image edit retained
the painted facets but changed the albedo to a medium-light warm neutral gray.
The textured iron/wood material factors are now neutral rather than multiplying
the color by a second dark brown tint. No exposure, world lighting, emission or
vertex-lighting changes were used.

Edit prompt:

> Edit only the color values/exposure of this texture, preserving the arrangement,
> sizes, fine hand-painted facets, seams, scratches, square coverage, and seamless
> tile edges. Change it into medium-light warm neutral gray iron, not dark brown
> stone. Desired sRGB palette: broad midtones around RGB(146,135,121), modest
> highlights RGB(187,175,156), darkest little crevices RGB(92,83,73). Preserve warm
> forged metal feel and fine blocky painted facets. Avoid black regions, blue hues,
> orange saturation, strong gloss, baked lighting, objects, perspective, text,
> borders, or new decorative elements. It must stay a flat, evenly exposed surface
> texture with its color data doing the work; no scene or illuminated render.
