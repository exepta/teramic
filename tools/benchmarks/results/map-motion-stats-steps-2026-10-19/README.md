# Phase 71: Map Motion, Reference Stats HUD And Material Steps

Implementation and final validation are recorded below. Native Vulkan captures
were inspected on KDE Wayland, AMD Radeon RX 6950 XT, Mesa 26.2.4. No controlled
FPS/p95/p99, upload or memory comparison has been performed. Audio-source and
playback checks do not constitute human listening acceptance.

## Implemented Behavior

The MiniMap uses the interpolated local character presentation position and the
current camera look. Cached terrain and its projection reference are committed
together; the existing UI material receives fresh projection uniforms every
render frame. The terrain cache remains asynchronous and limited to five
rasterizations per second. Marker textures retain their configurable
24/30/40/60-Hz cadence and use their own UV reference. The local direction arrow
is drawn live in the same existing GPU UI pass, rather than baked into those
slower marker uploads.

Terrain caches are north-oriented. Relief and shading remain anchored to world
directions while the final image rotates. A stable elevation reference per map
layer prevents player jumps from changing the terrain relief base. FullMap
dragging retains the starting pointer, map center and viewport scale throughout
the gesture. New map images do not replace that active drag anchor.

The transparent TiltUI stats HUD now follows the supplied reference silhouette:
a large octagonal dark-stone/gold heart medallion, faceted red heart, emerald
ornaments and connected health/stamina rails with a lightning badge. Decorative
bitmaps carry the artwork; health, stamina and hunger remain live native
progress-bar widgets. The separate hunger card has a short yellow meter and
uses the five existing eating stages. Thresholds are unchanged: full at >=90%,
first bite at >=75%, half at >=50%, last piece above 10%, bare bone at <=10%.
The 30% example retains the last piece. Oxygen remains conditional and does not
resize or reposition the HUD when it appears. Reference-only poison, cold,
fire and swirl icons introduce no gameplay mechanics.

Material contacts now use nine families and 51 short clips:

| Family | Contacts | Source Type |
| --- | ---: | --- |
| Grass | 5 | Kenney grass-footstep Foley |
| Sand | 6 | Existing Fantozzi sand recordings, additional takes |
| Stone | 6 | Existing Fantozzi stone recordings, additional takes |
| Dirt | 6 | Monte32 dirt-footstep recording |
| Snow | 6 | Existing Corsica_S snow-covered-gravel recordings, additional takes |
| Wood | 5 | Kenney wood-footstep Foley |
| Gravel | 10 | C-Dogs gravel footsteps |
| Glass | 5 | Kenney light glass-contact Foley, not literal walking-on-glass recordings |
| Leaves | 2 | Existing TinyWorlds leaf-ground Foley |

All existing voxel material IDs have explicit mappings. Ores, mineral blocks,
masonry and formations use stone; workbench/chest/plank/log variants use wood;
mushrooms use dirt; foliage variants use leaves. The fallback remains stone.
Each family avoids its previous take even after stepping on another material.
Small pitch/level variation and per-family gain preserve material distinctions.
Playback continues through the existing contact events and bounded cave-reverb
path, without a new audio engine or new authoritative game rules.

## Assets And Reproduction

Audio-source URLs, licenses, exact files and processing are recorded in
[assets/audio/CREDITS.md](../../../../assets/audio/CREDITS.md). New contacts live
in `assets/audio/footsteps/detail/`. They are mono 24000-Hz PCM16 with contact
trimming, high/low-pass filtering, level adjustment and short fades. The
reproduction script is `tools/assets/prepare71-steps.mjs`; it requires the
credited extracted source recordings and the existing local ffmpeg toolchain.

HUD artwork was generated with the built-in imagegen tool using the user's
reference and genuine transparent output. Final workspace assets:

- `src-ui/assets/survival-chrome.png` (2106 x 747 RGBA).
- `src-ui/assets/survival-status-frame.png` (1254 x 1254 RGBA).
- Prompts, provenance and native source-rect details:
  `src-ui/assets/survival-chrome.txt`.

The existing shared `survival-atlas.png` still supplies oxygen and hunger art.
The new chrome embeds the medallion heart and lightning badge; ordinary menu
and inventory `ornate-frame` components remain separate from this HUD artwork.
Source-pixel inspection verified actual exterior transparency and positioned
live meter rectangles inside the black channels without covering their gold
edges. That is not a substitute for inspecting the native composed result.

## Work And Memory Bounds

- Existing map tiles, world snapshots, asynchronous workers and cache limits
  remain in use. No extra world camera or world-generation pass is introduced.
- Terrain still rasterizes at most five times per second. Render-frame motion
  updates small existing material uniforms; it does not rasterize terrain at
  the render rate.
- Marker cadence still bounds CPU marker painting and texture uploads.
  Geographic marker textures and the local shader arrow have separate roles.
- Map images remain 320x320 for MiniMap and 640x640 for FullMap. The new UV
  uniforms and arrow do not imply an additional fullscreen pass.
- Audio uses the existing clip bank/contact mixer. More source clips increase
  asset and decoded-bank memory; no measured memory parity is claimed.
- HUD decoration adds two shared raster images and native widget geometry.
  Transparent artwork and shared handles do not make that render work free.

These are implementation properties, not measured runtime improvements. Long
travel, dense mobs, repeated teleports, high DPI, multiple GPUs and concurrent
audio still need broader profiling and acceptance.

## Validation Status

| Check | Current Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo check --workspace` | Passed |
| `cargo test --workspace` | 1072 passed, one existing UDP soak ignored |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo build --workspace` | Passed |
| Native MiniMap motion / FullMap drag | 25 stages, three additional boost-flight samples and scaled editor passed |
| Native survival HUD, hunger, stamina and layout | Passed with real integrated authority, food, exhaustion, respawn, drag/save/reload |
| Native recorded material playback | 27 injected contacts across nine families, 30 cues consumed by a real AudioSink |
| Headless dedicated server | Passed without DISPLAY/WAYLAND_DISPLAY, 27 sections |
| Active contact signal checks | 51 unique WAVs, mono 24000-Hz PCM16, peak <=0.720 |
| Source asset alpha / meter-channel placement | Inspected |
| Controlled FPS/p95/p99 / memory comparison | Not measured |
| Human listening acceptance | Not performed |

Regression coverage has been added for interpolated presentation coordinates,
projection/cache consistency, independent overlay transforms and fixed drag
coordinates. HUD tests cover the real TiltUI CSS parser, hunger examples
90/75/50/30/10 and position-stable oxygen visibility. The native survival fixture
checks the transparent outer container, chrome aspect ratio, shortened stamina
rail, hunger placement and rendered progress-bar widths during food/stamina and
respawn transitions. Native verification distinguishes bound numeric targets,
the widget's target layout and the actual animated pixel width: TiltUI's
existing 100-ms numeric transition deliberately interpolates moving stamina
values. A settled respawn additionally verifies convergence. The first native
run caught an unsupported blurred text-shadow syntax; it was corrected to the
actual two-offset TiltUI syntax before the successful rerun.

### Native Motion And Rendering

The map fixture uses a real integrated server plus a second real headless
network client. Its existing surface/cave, peer/mob, FPS-preset, scaled-layout,
waypoint-CRUD and TOML-reload checks remain. FullMap dragging now exercises
three successive pointer movements, holds the same pointer across background
cache updates and verifies the center remains fixed after release.

The added flight segment teleports through the authoritative chat command,
then holds the configured Forward/Sprint keys and briefly Jump, with real
MouseMotion rotation. At the tightest 96-metre zoom and 24 marker FPS it travelled
146.8306 metres horizontally, climbed 30.4056 metres and turned 0.3600 radians.
Three actual GPU screenshot samples retained nonblank, varied map terrain;
the local arrow stayed centered. These short captures and regression invariants
are not a continuous-video, long-session or multi-GPU performance acceptance.

The final flight image contains unknown terrain, not an out-of-bounds UV patch:
its exact sRGB fill is `(20,26,25)`, the CPU terrain raster's unknown color.
The shader's UV fallback is `(20,25,24)`. The same region is fully mapped in
`map-24.png` after three seconds stationary. Existing scheduling captures at
most four new live tiles every 0.2 seconds, so rapid exploration can temporarily
outpace cartography. This is documented rather than increasing chunk work.

One synthetic Wayland pointer-warp error remains in the fixture while the
cursor is deliberately unlocked. All drag/zoom assertions passed; normal map
input does not request these test cursor warps. No final shader/stylesheet
errors occurred. HUD captures include the default top-left placement and the
saved moved layout, with no opaque outer panel or placeholder status values.

### Audio Validation

The active bank is 752232 encoded WAV bytes. Independent sample inspection
found 51 unique hashes, lengths 0.100-0.694 seconds, RMS 0.0372-0.1049 and maximum
peak 0.71997. Per-file measurements and hashes are in
[audio-signal.tsv](logs/terra71-audio-signal.tsv). Decode, exhaustive material
routing, nonrepeating variation and real loaded support/cadence tests passed,
including a low workbench top. The native fixture injects contact messages;
it is not claimed as physical walking or human listening acceptance.

Final command logs are in [logs](logs/), inspected native snapshots in
[captures](captures/). Cargo compilation used `CARGO_BUILD_JOBS=2` to reduce
link-time memory pressure. The historical phase-70 report remains unchanged.

Use isolated fresh directories rather than normal player worlds, for example:

```bash
TERRA_CONFIG_DIR=/tmp/terra71-configs TERRA_SCREENSHOT_DIR=/tmp/terra71-minimap target/debug/terra-client --smoke-test-minimap
TERRA_CONFIG_DIR=/tmp/terra71-configs TERRA_SURVIVAL_SMOKE_DIR=/tmp/terra71-survival-world TERRA_CAPTURE_DIR=/tmp/terra71-survival target/debug/terra-client --smoke-test-survival
TERRA_CONFIG_DIR=/tmp/terra71-configs TERRA_BENCH_SECONDS=60 target/debug/terra-client --smoke-test-steps
env -u DISPLAY -u WAYLAND_DISPLAY TERRA_CONFIG_DIR=/tmp/terra71-configs target/debug/terra-server --smoke-test
```

Survival smoke requires a fresh fixture world directory. Historical Phase-70
captures do not establish this update's visual or motion acceptance.

## Boundaries

Map exploration, private waypoints, cave bands and save formats are preserved.
This update does not add new dimensions, shared server maps, a globally solved
cave map or persisted detailed relief for unloaded regions. It changes client
presentation, not movement authority, world IDs, seeds or block IDs.

Recorded or contextual Foley is identified by source. Distinct material
families and automated decoding/level tests cannot prove that a mix sounds
natural on headphones and speakers. Human acceptance and long-session
performance remain open even after functional/native tests pass.
