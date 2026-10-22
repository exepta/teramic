# Terra Block Library

Block JSON files are the single client material registry. They are discovered in
this directory and sorted by stable nonzero `u16` ID; 0 is air. Existing IDs 1
through 26, world IDs and logical one-meter cells retain their meanings.
The dedicated server does not load these rendering assets.

Every shipped block's editable `description` lives in its own JSON definition.
Inventory tooltips read that text directly; changing a description needs no Rust
code changes. Collectible prop descriptions are in individual
`../props/*.json` files, keyed by their existing prop kind. An omitted description
on an older custom block remains compatible and displays no description.

## Cube Definitions

### Construction Placement And Surface Detail (Phases 95/96)

Construction IDs 74..97 retain their existing definitions. Stairs 74..81 now
use all 24 proper cube rotations in the existing object-state `facing` field.
The clicked support face selects floor, ceiling or wall placement; the rise
direction follows the player's view so the treads face the approaching player.
Adjacent perpendicular stairs can produce inner/outer corners. The shared
`terra_core::construction::placed_boxes` profile supplies rendering, picking
and physics, including wall moulding corners.

Slabs 82..89 retain legacy lower-half facings 0..3. Facing 4 is the upper half,
5/6 the -X/+X half, 7/8 the -Z/+Z half, and 9 an identical-material combined
pair. Clicking the inner cut face consumes another slab and completes the cell.
Different materials use facing `10 + (second_material - 82) * 6 + split_index`;
the split indices refer to `[0, 4, 5, 6, 7, 8]`. The original material remains
the cell ID, preserving both materials without changing the saved object-state
layout. Survival drops both halves with their respective material IDs.
Side-face clicks in the bottom/top quarter place horizontal halves; the middle
band attaches vertical halves. Top/bottom faces allow ordinary stacking.
Sand fence 95 remains readable for saved worlds but has no
recipe, creative catalog entry or authorized new placement.

Snow Layer appends ID 98. Its boxes definition supplies the default 1/8 m
inventory shape; live rendering, picking and physics use the shared
`construction::snow_bounds` profile. Saved facings 1..7 select 1/8..7/8 m.
State 8 becomes ordinary Snow (ID 5), with facing 8 marking weather ownership.
Weather edits only its own marked caps above a support and never overwrites
player blocks. The existing region journal, replication and surface-condition
storage retain the states. Client and server require protocol 54 together;
IDs 1..97, generator 5 and existing region formats retain their meanings.

The existing terrain PBR extension provides world-anchored stone seams, earth
clods, turf, fine sand ripples and fine snow grain. Analytic bump normals,
roughness and restrained variation augment the existing mipmapped textures;
no logical material or block identity changes. Near relief and leaf volumes
remain section/material batches, not per-detail entities. Contact tints read
a temporary neighbor halo and do not modify saved block data.

`configs/graphics.toml` contains the validated `[blocks]` settings:
`detail_quality`, `micro_geometry`, `micro_geometry_distance`, `material_detail`,
`normal_mapping`, `surface_variation`, and `leaf_geometry_quality`. Quality is
`low`, `medium`, or `high`; default is `high` with a 48 m micro-detail distance.
Apply invalidates a render-only revision using the existing bounded job queues.
Oak, birch and natural spruce crowns share a finite, lazy species/LOD cache and
retain alpha clipping, wind and seasonal shadow behavior. Spruce species uses
the existing tree-appearance metadata, not a replacement block ID.

Architecture, native captures and measured limitations:
[Phase 95](../../../tools/benchmarks/results/construction-materials-horses-2026-10-21/README.md).
Mixed slabs, physical snow layers, stronger relief and opacity regressions:
[Phase 96](../../../tools/benchmarks/results/slabs-snow-horses-2026-10-21/README.md).

### Rooted Segments (Phase 80)

IDs 50-56 append seagrass, salt crystal, dripstone tip, ice crystal, cave vine,
cave root and amber cave mushroom. Their individual JSON definitions use
`"render_kind": "segment"`. These are ordinary one-meter world cells with
procedural sub-cell geometry, not GLBs or per-plant ECS entities. Cube and static
custom-model loading remain unchanged. The segment kind currently accepts only
these native IDs; adding an unrelated custom procedural species requires a real
shape implementation, not just changing a JSON string.

`terra-world::segments` supplies the same canonical triangles to section batches,
accurate picking and the fracture effect. Neighbor profiles keep taper and rooted
water movement continuous across section boundaries. Crystal collision uses four
clipped tapered bounds per cell instead of a full cube. Plant segments do not
block movement. The JSON `collision_boxes` on crystals opt into shaped colliders;
their final dimensions come from the shared live support profile.

Seagrass remains waterlogged and is supported from below; vines and roots hang
from solid ceilings. Mineral chains can attach at either end. Breaking a segment
only removes unsupported continuations, while detached mineral cells use existing
3D world-drop physics. Salt host IDs 30/38 drop nothing; harvested or landed salt
crystals yield the existing salt resource. None of these rules adds another
registry, save format, entity per detail voxel or renderer to the dedicated server.

Cold ocean generation no longer emits ice. ID 49 remains available for explicit
building and existing saved player edits. Highland waterfall-cave crystals use
the separate ID 53; they are not ocean ice floes.

Ordinary full blocks use `"render_kind": "cube"` for their shared inventory and
terrain geometry. Fluid Water (27) is rendered by the existing fluid surface
pipeline. Furniture and torches use static custom models; the native plants and
mineral segments above use procedural segment geometry.
They use the same procedural face generator and existing section mesher. Only
exposed faces are emitted, then triangles are batched by material per section.
No GLB geometry, per-block entity or per-block draw call is needed.

```json
{
  "schema_version": 1,
  "id": 60001,
  "name": "New Cube",
  "render_kind": "cube",
  "bevel_width": 0.06,
  "textures": { "Painted block": "textures/new-block.png" },
  "materials": { "Painted block": {} },
  "face_materials": [
    "Painted block", "Painted block", "Painted block",
    "Painted block", "Painted block", "Painted block"
  ]
}
```

Face order is West (-X), East (+X), Down (-Y), Up (+Y), North (-Z), South (+Z).
`face_materials` explicitly binds the primary material of each face. Grass has a
green top and earth below; snow is a complete white block on all faces. Optional
`side_cap: { "material": "Painted block", "height": 0.1 }` adds a narrow band to
vertical faces. Both the near geometry and coarse geometry retain this binding.
This removes reliance on arbitrary GLB primitive ordering, which previously
selected earth for some coarse grass surfaces.

Cube material definitions accept `base_color` (linear RGBA, default white),
`roughness` (default 0.92), `metallic` (default 0), and `double_sided` (default
false). Texture keys refer to these material names. A material without a texture
binding is a plain color, such as snow. Complete identical descriptors share one
Bevy `StandardMaterial` handle across blocks; grass and earth therefore
share their soil handle. Snow has a separate whole-snow material.
The shared plant palette remains separate.
`texture_pattern` optionally selects a generated raster texture instead of a
PNG binding. A material cannot specify both sources. `frame_material` names an
additional opaque material for the connected glass perimeter.

The generator rounds the exterior of the cube within [0,1]. Neighbor exposure
flattens its bevel bands to exact cell boundaries, including section edges.
When a vertex touches a culled neighbor interface, every coordinate inside a
bevel boundary band snaps to the canonical cell outline, not only the interface's
normal axis. Normal-only flattening left open wedges at concave three-cell
junctions with different bevel widths or LODs. Free exterior edges retain their
rounded shape. This changes only rendered geometry, not block occupancy.
Collision and editing still use cubic occupancy. There is no global resolution
change and no new voxel storage system.

The ten old Cube GLBs and their JSON `model` references have been removed.
This removes 15,875,208 bytes of redundant authored geometry and embedded images
from the asset directory; runtime Cube loading already made no GLB requests.
Only special authored geometry needs a `.glb` or `.gltf` model. External legacy
Cube definitions containing a `model` field remain readable, but that field is
ignored for Cube rendering. The ten shipped material IDs remain unchanged.

## Textures And Batching

Runtime textures combine shared standalone PNGs for neutral grass, dirt, bark
and canopy with small generated raster patterns: `sand`, `sandstone`,
`stone`, `smooth_stone`, `mossy_stone`, `planks`, `clay`, `mushroom_stem`,
`mushroom_gills`, `birch_bark` and six colored mushroom cap patterns. Patterns are generated
once as deterministic 64x64 RGBA textures, shared across blocks and sections,
and have a complete mip chain. Plank variants share one grain image with separate
linear material tints. Fine sand grain and sediment bands are distinct from the
stone seams; smooth stone has no generated rock relief. No new GLBs or external
texture dependencies are required. Existing PNG sources are retained. Leaves share
canopy-stylized while green, orange autumn and cool green birch use different
linear tints; bark uses wood. Compatible materials reuse handles; the expanded
palette does not imply one draw call per placed block.
Greedy quads use repeating coordinates rather than stretching one image over a
whole merged face. No atlas is introduced, so there are no adjacent atlas tiles
to bleed across UVs or mip levels.

The PNG loader supplies only mip level zero. Startup workers generate the full
chain down to 1x1 once per shared image, using linear-light premultiplied color
averaging, then replace the original image under the same handle. Terrain becomes
ready only after these jobs finish. Four-times anisotropic filtering reduces
oblique surface aliasing. Initial PNGs stay CPU-only until that chain is complete;
publishing the image also invalidates dependent material bindings, because Bevy
does not automatically rebuild a material when its image receives a new GPU view.
Mipmap creation leaves the source PNGs intact; smaller levels add roughly
one third to each uncompressed image's storage. Flattened near-face UVs are
projected from their final connected positions, matching the greedy LOD mapping.

A texture atlas would require padded tiles, mip-safe gutters, and shader-aware
repetition of greedy rectangles. A texture array avoids tile bleeding and would
support one terrain material, but requires array image preparation, a layer
attribute and a custom PBR shader path. The current change keeps Bevy's existing
PBR pipeline and deduplicates compatible handles. Draws remain per visible
material per section, rather than one draw per section across all materials.

## Surface Appearance

Grass top and side-cap vertices receive linear biome colors from the existing
six blended climate weights. Four shared section-corner samples avoid discrete
biome borders; LODs sample the same color field. Triangle interpolation inside
a greedy rectangle is an approximation to the finer near mesh's interpolation.
Prop templates mark foliage/stems explicitly so biome tinting preserves flower
colors. No extra entities, material per biome, or texture atlas is needed.
The sixth tint belongs to Mushroom Park. Dry oasis grass uses straw/olive
material variants rather than the green foliage palette. Dead desert bushes
are connected bare twigs with less wind flexibility than leafy bushes. Both
reuse the existing grass/bush identities, section batches and picking bounds;
variants 128 through 135 select these desert forms. Small mushrooms use six
cap colors matching their giant counterparts and retain color under biome tint.

Clear/rain/snow presentation changes the shared grass/prop materials at runtime.
Rain also reduces grass roughness. This is not a precipitation, snow accumulation
or fire simulation. Local wetness and charring are stored in the existing session
EditJournal. A separate appearance epoch invalidates only the owning section's
render jobs, not voxel data or colliders. Cached geometry is recomposed on workers;
stale appearances cannot replace the current batch. Sparse changes partition only
intersecting greedy faces by condition: one changed cell on a 16x16 face needs at
most five rectangles, not 256. Unloading/reloading preserves the state in-session.
Neither an atlas nor a texture array has been implemented; further reduction
requires a measured draw-call bottleneck to justify that pipeline complexity.

### LOD Replacement

Terrain and prop sections retain their current batch while an asynchronous
replacement is pending. A successful result with current epochs and the expected
LOD replaces the old entities in one deferred command commit. There is no timed
overlap of old and new mesh stages; `LodSwapStatus.completed` counts replacements.
Material bindings and selection hysteresis remain unchanged.

The previous terrain/prop dithering was removed. In the active Bevy 0.19
depth-only prepass, dithering alone does not enable a fragment shader: full depth
could be written where the main pass later discarded fragments, and shadows
were not masked consistently. The replacement does not add render targets,
passes or a custom shader pipeline. Props use `VisibilityRange::abrupt` to the
configured view distance, normally 192 m, behind the fully opaque fog around
179 m. No prop distance-fade mask remains.

This deliberately trades smooth transitions for a consistent opaque path:
geometric LOD changes can still be visible. Historical 48-second dither-flight
measurements do not validate this fix. Its final native and workspace validation
is recorded separately in the root README and PATCH; TODO contains only open work.

## Legacy And Custom Models

`"render_kind": "custom_mesh"` retains the authored GLB path. Missing
`render_kind` defaults to `custom_mesh`, so existing schema-v1 definitions load
without migration. A custom model requires a safe relative `.glb` or `.gltf` path and can
still use named mesh groups, multiple materials and authored detail meshes.

```json
{
  "schema_version": 1,
  "id": 60002,
  "name": "Authored Block",
  "render_kind": "custom_mesh",
  "model": "custom.glb",
  "bevel_width": 0.0,
  "textures": { "Surface": "textures/custom.png" },
  "faces": ["West", "East", "Down", "Up", "North", "South"],
  "details": [{ "mesh": "TopDecoration", "probability": 0.1 }]
}
```

The default `model_layout` is `face_groups`, preserving the original contract:

- glTF 2.0, Y up, indexed finite triangles with normals, UV0 and explicit materials.
- Six named surface groups with baked transforms; surfaces fit inside [0,1].
- Groups partition the authored surface and can contain multiple primitives.
- JSON `bevel_width` describes the band flattened against solid neighbors.
- JSON texture bindings override the named imported PBR materials' textures.
- Optional named details may extend above their supporting cell.
- The group meshes use baked transforms. Full node hierarchies are available in
  the optional scene layout below.

The renderer preserves custom authored surfaces at distance; it must not replace
an arbitrary custom face with the first primitive's material. The dominant face
material metadata is selected by triangle surface area, never primitive order.

### Whole Static Models

Non-cubic special blocks can opt into the entire default glTF scene:

```json
{
  "schema_version": 1,
  "id": 65000,
  "name": "Carved Pedestal",
  "render_kind": "custom_mesh",
  "model_layout": "scene",
  "model": "fixtures/pedestal.gltf"
}
```

The importer uses the default scene, or the first scene when no default is
specified. It recursively bakes translations, rotations and nonuniform scales
from all mesh nodes. Mirrored transforms reverse winding and normals use the
inverse-transpose transform. Multiple primitives, shared mesh instances and
imported PBR materials are preserved. No six named groups are required. Texture
overrides are optional; untextured models and glTF's default material work too.
Imported source meshes are CPU assets only; final section batches are uploaded.

After transforms, geometry must fit [0,1] on all axes. A scene model is one
editable occupied cell for collision, mining and placement, even if its visible
shape is smaller. It has **no render occlusion of neighbors**: the renderer restores
terrain surfaces next to it and emits the full model once per occupied cell,
including models surrounded by other non-cubic models. This rule also applies
across negative-coordinate section boundaries and unloaded neighbors represented
by session edits. Models remain complete at every LOD; their material triangles
share existing section batches, with no per-model entity or draw call.

The normal Cube meshing path is unchanged. Only a library containing scene models
enables additional occupancy snapshots, cached by the existing section mesh epoch.
Composition and supplemental face meshing happen on workers. The renderer does
not maintain a second block registry, modify generation, or change world IDs.
Final terrain, prop and water Mesh assets use RENDER_WORLD: vertex/index data
moves to the GPU, while Bevy retains the AABB and handle metadata. Rebuilds read
the existing cached source geometry, never the uploaded CPU mesh attributes.

Scene layout requires zero bevel width and no legacy `details` entries. It rejects
animations, skins, morph targets, singular transforms and geometry outside the
cell rather than pretending to animate or silently clipping it. Animated or
multi-cell objects still need a gameplay object renderer. Cameras and glTF lights
are not instantiated. Arbitrary per-placement rotation is not stored in existing
voxel data; author the desired orientation in the model's node hierarchy.

`fixtures/pedestal.gltf` is a real non-cubic hierarchical model used by tests.
Its accompanying JSON is intentionally inside `fixtures/`, so it is not
discovered as a new default palette entry and the shipped Cube IDs stay
unchanged. An opt-in rendering fixture places that definition in the block directory;
the model remains referenced from the fixture subdirectory.

The loader rejects duplicate IDs, unsafe paths, unknown JSON fields, invalid
schema versions, missing face/material bindings and non-finite material values.
Restart the client after changing block definitions. The generic block storage
and edit journal preserve arbitrary `u16` custom-model IDs, including the fixture's
65000. Adding a new gameplay block also requires registering its authoritative
`MaterialId`; dropping an arbitrary JSON into the directory does not grant remote
clients permission to place an unregistered ID.

## Plants And Tree Details

The historical grass/sand `details` entries remain compatible authoring metadata.
For Cube definitions, these names do not load model assets or emit decorations
in addition to world props.
Grass, flowers, ferns, shrubs and shore plants use the existing deterministic
ground-cover generation extended with stable identities and section batches.
Small visual elements belong to their logical prop, not to separate ECS entities
or additional editable voxel cells. The prop renderer reuses the common palette.

For custom meshes, `details[].density_field` still selects grass, bush, flower or
fern density. `water_clearance` restricts details to an inclusive dry-bank height
range above the configured waterline. Submerged decorations are suppressed.

Version-5 trees retain editable wood and foliage cells, roots, branches and crown
clusters, including green, autumn and birch variants. Placement hashes global
coordinates; the generator seed and material IDs are unchanged. Visual crown
details do not modify the generated occupancy or world save data.

Flowing water has no block JSON or GLB. It uses the separate streamed surface
mesh and `shaders/terra_water.wgsl`. The decorative solid-water block described
below reuses that shader without entering the fluid simulation.

## Building Palette And World Materials

IDs 1 through 10 retain their original meanings. The appended palette is:

| ID | Material | Natural occurrence |
| --- | --- | --- |
| 11 | Sandstone | Layered beneath desert sand, transitioning to deep stone |
| 12 | Smooth Stone | Building block only; never procedurally generated |
| 13 | Mossy Stone | Sparse damp cave walls near daylight and river-gallery mouths |
| 14 | Solid Water | Building block only; animated, translucent and colliding |
| 15 | Framed Glass | Building block only; connected translucent panes |
| 16 | Oak Planks | Building block only |
| 17 | Birch Planks | Building block only |
| 18 | Spruce Planks | Building block only |
| 19 | Clay | Irregular shallow deposits in submerged sandy beds and river beds |
| 20 | Mushroom Stem | Cream/brown fibrous stems of giant mushrooms |
| 21 | Purple Mushroom Cap | Giant mushroom cap in Mushroom Park |
| 22 | Red Mushroom Cap | Giant mushroom cap in Mushroom Park |
| 23 | Yellow Mushroom Cap | Giant mushroom cap in Mushroom Park |
| 24 | Green Mushroom Cap | Giant mushroom cap in Mushroom Park |
| 25 | Blue Mushroom Cap | Giant mushroom cap in Mushroom Park |
| 26 | White Mushroom Cap | Giant mushroom cap in Mushroom Park |

Material deposits and giant mushroom structures use deterministic cached column
samples. Existing edits override generated materials. Unedited ground may have
new deposits and biome structures; no block ID or saved edit is remapped.
The wire protocol is 14 so clients with older material or biome contracts cannot
silently join an incompatible server.

Mushroom caps share one pale gill material on their underside while their top
and sides use one of six speckled textures. The stem has cream/brown fibers.
All are opaque, solid, individually editable blocks; the existing chunk mesher
culls hidden faces inside each giant structure. Their textures and material
handles are shared by every mushroom. The palette now has 28 terrain material
handles before adding the shared vegetation palette. No giant mushroom needs
a model instance, custom GLB or special collision implementation.

The shared authoritative enum defines opacity, collision, natural spawn support
and flammability. Clay, sandstone and moss are valid natural spawn supports;
glass and decorative water are not interpreted as dry land. New wood planks burn
through the existing surface-condition system. Inventories, timed mining,
creative instant mining, resource drops, placement, section deltas and disk saves
use the same stable IDs. No new inventory or persistence store is introduced.

Solid Water (14) has the same cubic collision and mining semantics as other
building blocks. Its visible faces use the existing animated water material,
with a fixed-cell shader tag and one-meter depth limit. Animation does not move
the logical cell or create a fluid source. Placing it displaces liquid in its
cell; removing it permits neighboring ordinary fluid to flow into the vacancy.
It is excluded from swimming, underwater fog and underwater audio classification.
It remains visible through the water material even if natural-water generation
is disabled for the world. Graphics water-quality settings also apply to it.

Fluid Water (27), `teramic::fluid_water`, creates an actual full source cell in
the existing fluid journal. Its selectable anchor is not solid and emits no
terrain cube faces; the shared fluid mesh renders the source and its finite
outflow. Removing it stops the infinite supply without deleting water already
released. The inventory preview shares Solid Water's animated material. No GLB,
new water renderer or independent persistence format is introduced.

Glass (15) uses alpha 0.1 for the pane, approximately 90 percent transmission for
one face, and an opaque 3.5 cm wood perimeter. Multiple separated panes can still
attenuate more light; this is alpha blending rather than a volumetric optical
model. Adjacent glass cells suppress their shared faces and their internal frame
edges. `BlockFace.edge_mask` stores U-/U+/V-/V+ edge visibility from the existing
one-cell meshing padding, including unloaded and negative-coordinate neighbors.
Glass faces remain cell-sized to preserve irregular joins and frame masks; other
materials retain greedy faces. The same pane/frame construction is used at every
LOD, and border edits remesh the affected neighboring sections. Panes and frames
are batched by material per section, not emitted as individual block entities.
Standard Bevy transparent-batch sorting still applies; intersecting transparent
surfaces are not handled by a new order-independent transparency renderer.
Both pane and frame batch entities explicitly carry `NotShadowCaster`.

Opaque faces behind leaves, glass and solid water remain visible. Transparent
neighbors never punch holes in the terrain, and same-material internal faces
remain culled. Render transparency does not weaken collider occupancy. No block
requires its own Cube GLB; custom non-cubic authored models retain their existing
scene/face-group pipeline.

## Validation And Authoring

Tests cover stable block IDs, legacy schema loading, missing bindings, unsafe
paths, material sharing, explicit grass/snow caps, closed procedural surfaces,
outward triangle winding and finite indices/coordinates. Cube asset tests verify
standalone PNG bindings, current JSON color factors, empty model references and
the absence of the ten obsolete GLBs.
Section seam tests additionally exercise the rendering mesh pipeline.
Static-scene tests run the real Bevy glTF loader, verify hierarchy transforms,
mirrored winding, full-model batching and neighbor-face restoration. They do not
require a renderer. The additional `--smoke-test-custom` native mode renders
three fixture models beside a regular cube; its checked result is recorded in
`screenshots/reference-custom.png`. The root README contains the opt-in command.

The obsolete `tools/assets/build_blocks.py` exporter has been removed so it cannot
regenerate per-material Cube GLBs. Author only non-cubic custom objects in a glTF
2.0 editor, following the scene or face-group constraints above; normal Cube
geometry is maintained in the shared Rust face generator. The small text-based
`fixtures/pedestal.gltf` remains the deterministic custom-model regression asset.
Texture provenance is recorded in `textures/prompt.txt`,
`textures/canopy-prompt.txt` and `textures/stylized-prompts.txt` (new built-in
image_gen textures and exact prompts). Performance measurements and remaining visual
limitations belong in the project overhaul report; material deduplication alone
is not evidence of an FPS improvement.
