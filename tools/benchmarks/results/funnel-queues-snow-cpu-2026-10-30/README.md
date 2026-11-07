# Funnel Queues And Shaped Snow

Phase 161, locked Bevy 0.20 workspace. Existing block IDs, save formats,
world generation, graphics quality and dependencies are unchanged.

## Logistics

Exports and passive imports transfer one item per port tick, not four. The
existing world drops now share a bounded spatial occupancy index during belt
transport and port processing. A moving item checks nearby items at every
movement substep; movement, admission and removal update the current index.
Items queue on the actual carrying surface. A blocked outlet leaves the item
in its source inventory. Existing saved stacks retain their identity and
remaining quantity during partial imports. No second virtual belt inventory
or transport simulation is introduced.

Tunnel 152, Funnel 153 and Funnel (Filtered) 157 reuse the existing ghost-item,
Allowed/Blocked filter, authorization, snapshot and persistence paths. Normal
ports without a configured filter still accept every item. Block 157's display
name is now `Funnel (Filtered)`; its identifier and recipe remain unchanged.

`crushers/belts.rs::inventory_contact` validates the mounted port, outward peer,
inventory and any paired inventory cells. Matching horizontal endpoint contacts
extend the existing ribbon by half a cell to the inventory face. Only a
previously observed and unchanged contact may survive an unloaded neighbor;
unknown neighbors, contradictions, removal and wrong-side peers create no
extension. The existing bounded mesh cache is reused, with no extra entity,
material or independent draw call per contact.

## Snow And Categories

`weather::supports_snow_layers` permits separate caps only on complete voxel
surfaces. Stairs, slabs, fences and paths receive snow on their actual geometry
through the existing wet-surface shader. Custom-model composition now supplies
the saved surface payload to the same UV1 batching path as the other terrain.
Paths cap their visual snow payload at 0.25: a light frost keeps their compacted
earth texture visible even when the saved weather condition reaches 1.0. The
stored condition and rain wetness are not reduced.
Old weather-marked caps on shaped supports are removed during budgeted weather
visits. Ordinary player snow blocks remain protected. The legacy format cannot
distinguish externally forged weather markers from real natural markers.

Thin SnowLayer 98 cells can be replaced by construction, including both cells
of a colored bed. Full Snow 5 cells remain solid and are not replaced. Client
targeting and server authority share this rule; line of sight, revisions,
collision checks and survival inventory consumption remain enforced.

Natural accumulation uses 0.00075 instead of 0.003 per second, four times
slower. One eighth-height layer takes about 167 seconds at unit snowfall before
the existing intensity and deterministic local modifiers. Winter retention and
manual placement are unchanged. Machinery and furniture keep their existing
snow exclusions.

The data-driven `resources` category is named `Rohstoffe` and uses a dye icon.
Named and dynamically mixed RGB dyes share that category, without enumerating
RGB colors in Too Many Items. Explicit content-pack category overrides remain
authoritative. Existing mineral classifications are unchanged.

## CPU Corrections

`scene/caves/lights/updates.rs` consumes the existing publication, removal and
edit messages every frame, including between light-selection ticks. Replica
metadata-only changes no longer rescan all loaded sections or flatten all
cached light lists. Bootstrap, changed focus/reach and changed light definitions
seed discovery; new raw discoveries have a 128-section tick budget and pending
positions have a 32,768 hard limit. Edits and exposure changes receive immediate
attention. Missing messages/local-authority fixtures retain the conservative
reference path, and overflow falls back to reconciliation. Unload/reset clears
obsolete entries; the cache cannot exceed observed still-loaded sections.

`meshing/visibility.rs::hidden_with_wall_ring` retains the existing portal BFS
and its visible wall/shadow ring. Dense outdoor graphs now test the smaller
unreached complement instead of inserting six mostly duplicate neighbors per
reached section. Sparse graphs use the original algorithm. Tests compare both
algorithms at negative and extreme coordinates, including zero-port seeds.
No additional geometry is culled and no LOD quality is reduced.

## Measurement Conditions

Ryzen 9 5900X, Radeon RX 6950 XT, RADV Vulkan, native Wayland, actual 3840x2160,
view distance 20, high block/shadow/water detail, micro geometry enabled,
medium cave quality, ChunkAnimation None, integrated authority and real TiltUI.
The fixture's compute cap is 16; normal production pool policies are unchanged.
The preserved previous executable already contains phase 160's Mailbox policy.
No driver present-mode override is used.

Sampling windows are 48 seconds after actual scene readiness. No Cargo build,
second native client or asset generation runs during sampling. Seed 86420 uses
clear weather and 12 m/s exploration. Seed 99173 uses rain and 20 m/s exploration
from identical copies of the same saved world, including saved player location,
MAIN landscape revision 1, river revision 4 and its biome-definition snapshot.
Short runs measure these scenes, not long-term stability or a universal FPS
floor. Screenshot-adjacent frames follow the fixture's existing exclusion;
unfocused frames are not filtered. New worst-frame context records focus,
visibility and the actual present mode without changing gameplay or statistics.

## Measured Results

| Seed / weather | Build | AVG FPS | MIN FPS | p95 ms | p99 ms | Frames below 32 FPS | End RSS KiB |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 86420 / clear | Before | 85.78 | 7.67 | 15.66 | 18.37 | 1 | 2,454,180 |
| 86420 / clear | First validation | 83.56 | 30.92 | 15.90 | 19.24 | 1 | 2,568,764 |
| 86420 / clear | Final | 88.75 | 38.35 | 15.10 | 18.08 | 0 | 2,543,512 |
| 99173 / rain | Before | 82.94 | 45.53 | 15.85 | 18.14 | 0 | 2,172,064 |
| 99173 / rain | First validation | 83.04 | 46.86 | 15.39 | 17.89 | 0 | 2,268,932 |
| 99173 / rain | Final | 87.47 | 43.47 | 14.66 | 17.19 | 0 | 2,252,944 |

Final AVG increases about 3.5%/5.5% and both p95/p99 values improve, but rain's
minimum is slightly worse than before. The first validation, before the final
path-frost refinement, did not show an overall AVG gain; clear also had a worse
p99. Both repetitions are retained rather than selecting only the better pair.
The small path-color adjustment is not claimed to cause the repeat's FPS gain.
Both final RSS values are higher, so this is not a measured memory reduction.
End loaded/rendered section counts are 13,816/4,625 before versus 13,816/4,614
final on 86420, and 18,898/3,491 versus 18,917/3,488 on 99173. Streaming timing
and end camera height vary slightly; these are matching routes/settings, not
pixel-identical frames or statistically established universal improvements.

| Sampled CPU wall scope | Seed | Before | Final |
| --- | ---: | ---: | ---: |
| Light selection, total ms in 48 s | 86420 | 433.82 | 176.79 |
| Light selection, total ms in 48 s | 99173 | 847.49 | 312.60 |
| Portal worker, mean ms per call | 86420 | 14.220 | 10.432 |
| Portal worker, mean ms per call | 99173 | 21.574 | 16.079 |

Light selection falls about 59% and 63%. Its final inclusive scope includes
79.71/124.05 ms of seeding and 56.40/101.95 ms of incremental discovery; these
nested values must not be added to the parent again. Portal analysis has
105/104 calls on 86420 and 158/157 on 99173. Worker timings include scheduling
and may overlap Main, so reductions are not additive frame-time savings.
Mean Main time is 10.916 -> 10.570 ms and 11.319 -> 10.674 ms respectively.

The isolated ignored regression benchmark uses the same 18,491-section graph,
16,642 reached entries and exact output equality. For 64 iterations the old
wall-ring step takes 312.442 ms and the optimized step 41.181 ms, about 86.8%
less wall time for that step. It runs without Cargo or a native client.

### Remaining Presentation Tail

The original 86420 run reproduces a 130.43 ms frame, including a 118.07 ms
`prepare_windows.body` span and no preceding generation, meshing or upload
work. The first validation still has a 32.34 ms frame with a 21.75 ms window-preparation
span and no preceding jobs/uploads. Its recorded window is visible but not
focused, using Mailbox. This is correlation, not proof of an OS/driver cause;
the previous executable does not record focus, and no unfocused frame is
excluded. The final repeat has no frame below 32 FPS on either seed, with worst
frames of 26.08/23.00 ms. All twelve recorded worst-frame contexts in each final
repeat are focused and visible. The earlier bad frames are not discarded, so
these results do not prove rare presentation stalls are eliminated. A05 remains
open; a universal 32-FPS floor is not established. No new native 10/15/30-minute
run was performed.

## Validation And Artifacts

All commands exit successfully on the final sources:

```sh
cargo fmt --all --check
cargo check --locked --workspace -j1
cargo test --locked --workspace --no-fail-fast -j1
cargo clippy --locked --workspace --all-targets -j1 -- -D warnings
cargo build --locked --workspace -j1
```

The workspace has **3308 passed tests**. The separately invoked ignored portal
microbenchmark also passes. Regression coverage includes full/stopped belts,
horizontal and sloped queues behind a real authored overhang, removal and
resumed transport, source/output conservation, partial saved stacks and IDs,
multiple port types, exact RGB ghost filters and UI controls, repeated storage
reloads, matching/unknown/unloaded inventory contacts, negative boundaries,
natural/player snow ownership, survival consumption, full-snow protection,
colored paired beds, geometry-preserving snow payloads, capped path frost,
bounded light discovery and metadata-only idle updates.

Native `--smoke-test-rgb-logistics` passes with five captures and actual
authority, ghost/switch/craft handlers and save/reload. The endpoint detail and
filter UI were visually inspected. Native `--smoke-test-materials` passes all
16 capture stages and 158 GPU previews; the new snow-shapes view was visually
inspected, including the final restrained path frost. The first over-white path
capture is retained as a comparison. These fixtures do not replace manual play
testing of every building arrangement. The display-free dedicated-server smoke
test passes with 45 loaded sections. Final native logs contain no ERROR, panic
or unhandled-message entry.

- [Mounted belt inventory connection](belt-inventory-contact-detail.png)
- [Filter Allowed](filtered-funnel-allowed.png), [Filter Blocked](filtered-funnel-blocked.png)
- [Final shaped snow and path frost](materials-snow-shapes.png)
- [Initial path before visual correction](materials-snow-shapes-initial.png)
- [Before clear](teramic161-before-86420.log), [before rain](teramic161-before-99173.log)
- [First clear](teramic161-first-86420.log), [first rain](teramic161-first-99173.log)
- [Final clear](teramic161-final-86420.log), [final rain](teramic161-final-99173.log)
- [CPU microbenchmark](teramic161-portal-wall-ring.log), [workspace tests](teramic161-tests.log)
- [Native logistics](teramic161-native-rgb.log), [native materials](teramic161-native-materials.log), [headless server](teramic161-server.log)
- [Rounded measurement summary](measurements.csv)

Graphics and world-configuration copies accompany the raw logs. No user login
profiles, authentication files, world saves or large executables are included.
PNG and log files are local runtime artifacts under the repository's existing
ignore policy; the report, CSV and configuration metadata remain versionable.
