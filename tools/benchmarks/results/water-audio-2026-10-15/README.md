# Water Geometry, Animation, Audio And Atmosphere

## Implemented

The previous river level was based only on its centre line, then applied across
the entire terrain-blending zone. A lower natural bank could therefore be filled
at an elevated level until the influence abruptly stopped. Corner averaging also
ignored dry bed heights. These were the causes of unsupported water edges.

Cached drainage paths now include surrounding bank elevations. The water-bearing
ribbon is distinct from the wider terrain-carving influence; the latter no longer
creates elevated lakes. A one-meter support margin accounts for voxel rounding.
Terrain still only carves existing land. Shared corners include dry bed heights,
and gentle intersecting slopes use a common height group rather than a different
average for each requesting cell. Depth and shore coverage are shared too.

Falls require at least five meters. The connected lip feeds a curved curtain,
with bounded lateral relief inside the downstream cell and a world-aligned
one-meter vertical raster. Polygon clipping retains sloping lips/feet across
section planes, including negative coordinates. Existing surface quads carry
crest/impact foam metadata; no extra overlay entities or rendering pass.

The existing water shader now uses a stable integer hash, coherent world-space
animation phases, stronger filtered normal waves, flowing ripples and moving
shore wash. Waterfall strands, curvature-driven normals, refractive motion,
crest foam and impact churn are distinct from calm water. Shallow absorption
approaches zero at the bank. Existing Beer-Lambert transmission, SSR and
atmosphere reflection remain, with the same quality controls and ray counts.
Waves change normals/refraction, not the geometric height of the water surface.

Warm midtone grading, restrained saturation and less pale biome grass colors
bring the palette closer to the reference. The neutral grass texture still
receives biome, weather, wetness and burn state. Underwater environment blending
restores the same shared surface grading/ambient baseline on emergence.

## Audio

The client enables the existing Bevy audio feature without upgrading dependency
versions. Its new transitive audio packages are recorded in Cargo.lock. A single
AudioPlayer mixes three deterministic procedural noise voices: shore, current
and waterfall. No external recordings or per-water-cell sound entities.

An asynchronous bounded 17x17 scan every 0.5 seconds locates sources in the
production water field. The PlayerBody head controls distance and immersion;
camera orientation affects stereo panning but camera position does not.
Unloaded or blocked source cells are silent. Real two-pole low-pass filtering
moves from 12 kHz to 650 Hz underwater, with smooth transitions and 35 percent
level reduction. Pause retains ambient sound; session exit removes stream/jobs.

There was no previous game sound bus. Future unrelated SFX must be connected to
an environmental mix bus separately. These synthesized sounds have not received
a human listening/mixing review.

## Validation

All required final workspace commands passed:
- cargo fmt --all --check
- cargo check --locked --workspace -j 2
- cargo test --locked --workspace -j 2: 258 passed, one existing network soak ignored
- cargo clippy --locked --workspace --all-targets -j 2 -- -D warnings
- cargo build --locked --workspace -j 2

Additional validation:
- Separate cargo build --locked -p terra-server -j 2 and no-display/no-Wayland
  dedicated smoke start passed with 27 generated sections.
- Dedicated feature tree contains no bevy_audio, rodio, cpal, bevy_render,
  bevy_winit, wgpu or TiltUI.
- Native Vulkan shader compilation and river/fall/shore/underwater captures
  passed and were visually inspected. Native physics smoke passed.
- Native Low water with zero wave strength and disabled SSR passed; captured and
  inspected separately from the High river/fall path.
- Same-camera captures three seconds apart show animated water. The 700x400
  waterfall region at pixel 700,400 has normalized RMSE 0.0870; this region
  excludes the changing debug panel. This measures change, not visual quality.
- Native audio fixture used a real AudioSink. A dry player with submerged camera
  remained unfiltered; an underwater player with a dry remote camera reached
  filter blend 0.99988556. Emergence, pause and session cleanup passed.
- Unit tests cover deterministic river support, negative coordinates, end caps,
  exact five-meter threshold, index/winding validity, shared corners, curved
  seams, sloping boundary clipping, bounded DSP output, reduced high-frequency
  energy, spatial attenuation and silence after disabling the soundscape.
- Existing tests preserve world generation references without rivers,
  persistent edits/props and the shared authoritative movement path.

Native fixtures:
`--smoke-test-rivers` finds a supported natural waterfall for seed 42042 at
X=3628.5, Z=3260.5, levels 16.733887/10.624793 (about 6.11 m).
`TERRA_RIVER_OVERVIEW=1` uses the nearby reach at X=62.5, Z=24.5.
`--smoke-test-water-audio` uses the same genuine fall and player-based listener.

## Performance

Before and after use the existing six sequential 18-second hydrology-suite
runs, physical 1920x1200, VSync off, view radius 16/3, identical worker budgets,
no screenshots and enabled GPU pass diagnostics on Radeon RX 6950 XT/Vulkan.
No builds or other game instances ran alongside either measurement suite.

The after suite sets `TERRA_RIVER_BASELINE_VIEW=1` to preserve the original
river camera (102.5, 52.364502, 416.5) and orientation. The corrected terrain
there now contains a supported sloping river instead of the old unsupported
fall. Thus camera/settings match, but river geometry legitimately differs.

Frame times in milliseconds, resident memory in KiB:

| Fixture | FPS before -> after | p95 before -> after | p99 before -> after | Initial ready s | Resident KiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| forest-1 | 120.516 -> 120.296 | 9.977 -> 10.057 | 13.236 -> 12.806 | 3.159 -> 3.082 | 751248 -> 760820 |
| forest-2 | 120.034 -> 120.219 | 9.893 -> 10.122 | 13.763 -> 13.889 | 3.046 -> 3.157 | 739100 -> 765184 |
| offline | 147.758 -> 148.382 | 8.292 -> 8.350 | 9.412 -> 9.319 | 2.991 -> 2.978 | 770836 -> 761520 |
| integrated | 144.596 -> 148.116 | 8.640 -> 8.317 | 12.511 -> 9.176 | 3.010 -> 3.020 | 775492 -> 776176 |
| river-1 | 152.123 -> 150.095 | 8.243 -> 8.325 | 10.225 -> 10.127 | 3.326 -> 2.888 | 767932 -> 758340 |
| river-2 | 152.399 -> 150.062 | 8.153 -> 8.231 | 9.682 -> 9.977 | 2.897 -> 2.826 | 765412 -> 780164 |

Forest FPS changes are -0.18/+0.15 percent; river FPS is 1.33/1.53 percent lower.
There is no general FPS improvement claim. p95/p99 and memory variation are
reported rather than hidden. These short sequential samples are not a
statistical proof of zero overhead or a guarantee at every world position.

Forest geometry stays at 7704 loaded / 3531 rendered sections, 9171 mesh assets,
4764 visible mesh entities, 89866 represented props and 1277 prop batches.
The fixed river view changes from 7633/4351 to 7635/4342 loaded/rendered sections,
9075 -> 9023 mesh assets, 4617 -> 4586 visible mesh entities,
61042 -> 63355 represented props, 1222 -> 1240 prop batches and 85 -> 49 water
tiles, because the corrected banks change the generated surface.

River transmission-pass GPU means change from 0.3246/0.3404 ms to 0.2484/0.2484 ms.
This is one pass, not total GPU frame time; fewer water tiles also contribute.
Actual draw-call counts were not captured and are not inferred from entities.
Terrain/prop composition worker sums and maxima, process CPU seconds and peak
RSS are retained in the raw logs. Worker sums are not wall-clock load time.

The new supported waterfall scene was additionally measured for 18 seconds:
138.008 FPS, p95 8.745 ms, p99 11.617 ms, initial ready 3.130 s,
760536 KiB resident, 7623 loaded / 4414 rendered sections, 9242 meshes,
70031 represented props, 1269 prop batches and 215 water tiles.
Transmission-pass GPU mean/p95/p99: 0.4686/0.4753/0.4868 ms.
This camera has no matching before sample and is not compared as a speedup.

CPU-only streaming benchmark (unchanged default world with rivers disabled):
4375 cold sections in 2.626 s, one-column move 0.117 s,
eviction 5273 sections in 4.934 s, reload 4375 in 2.626 s.
This retains the historical 2.626 s baseline; that old measurement is not a new
paired run. GPU uploads are excluded here but included in native initial-ready
times. The river-support work only runs for cached river paths.

## Compatibility And Limits

Seed 42042, generator version 5, block/world IDs, save format and protocol 6 remain.
Existing river-enabled worlds regenerate corrected base terrain; journal edits
stay at their recorded coordinates. Use a fresh river world to compare cleanly.
Rebuild client and dedicated server together. Worlds without rivers retain
their geometry. No destructive save migration or extra prop registry.

Still static generated waterways, not dynamic fluid volume, erosion, redirected
flow, cave flooding or volumetric spray. There is no vertical wave displacement.
SSR cannot reflect off-screen/occluded geometry; no temporal SSR filter was added.
No promise of reference-identical terrain density/proportions or complete
art-direction approval. Human audio review and the previously documented small
Wayland dropdown issue remain open. Long network soak was not rerun.
The Low-water wide view also shows the pre-existing planetary atmosphere ground
beyond finite chunk visibility as a distant color band. Its physical ground
lighting differs from the fixed terrain/water fog color; no view-independent
horizon fix is claimed in this change.

Final captures: [river](captures/river-overview.png),
[waterfall](captures/river-waterfall.png), [motion](captures/river-motion.png),
[underwater](captures/river-underwater.png),
[corrected former problem view](captures/fixed-view.png),
[Low water](captures/water-low.png).
Raw baseline and final samples are in before/ and after/; checks in logs/.
