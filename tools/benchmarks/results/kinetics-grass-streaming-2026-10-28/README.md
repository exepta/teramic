# Grass, Dirt, Kinetics And Streaming - 2026-10-28

## Implemented Changes

- Distant grass: a greedy quad could interpolate a single ledge's ambient
  occlusion across a large grass surface. Adaptive, bounded subdivision now
  samples the actual local lighting, including negative coordinates and section
  borders. Uniform faces remain greedy. The authored grass model, texture and
  top palette have not been replaced.
- Dirt, dirt stairs, slabs and fences now reference the exact grass-soil PBR
  material through a validated `coarse_face` alias. Albedo, normal and ORM handles
  are shared; this is not a new approximately matching brown texture.
- Streaming: temporarily pending column-exposure capture no longer invalidates
  completed worker/staged geometry. Unchanged exposure reuses that work; a real
  roof/exposure change still discards and rebuilds it. Stale staged handles are
  released on the changed path.
- Static terrain normal/tangent packing moved into the meshing worker. A
  normal/tangent pair uses two normalized signed four-byte attributes, saving
  20 CPU bytes per vertex versus float vectors before staging/splitting.
  Normal-only meshes preserve the absent tangent and save eight bytes. Directions
  are quantized; positions, UVs, triangles and tangent handedness are preserved.
  Water and animated foliage/flames retain their existing paths. Authored GPU
  frames were already compressed to eight bytes, so this is **not** a further
  20-byte GPU/VRAM saving for those meshes. No Bevy/vendor shader modifications.

Teramic columns already have a 16x16 horizontal footprint and consist of
16x16x16 sections, with surface/cave streaming envelopes. Increasing section
height would increase individual rebuild/upload granularity, not remove these
measured costs. This change does not alter world generation or saved block IDs.

## F25 Kinetic System

New workbench recipes and blocks, appended without renumbering old blocks:

| ID | Block |
|---|---|
| 141-143 | Wooden, Stone, Iron Gear |
| 144-146 | Wooden, Stone, Iron Shaft |
| 147 | Water Wheel |
| 148 | Conveyor |

The existing hand crank can power shafts/gears as well as the existing crusher.
The water wheel samples the existing liquid-current API, including partial
mechanical-block fluid occupancy. Side falls require a continuously wetted rim;
opposing falls cancel, and dry gaps or unloaded neighbors cannot create power.
Standing water does not create power. Signed
RPM, reversing gear contacts, conflicting sources, cycles and load limits are
solved authoritatively at 10 Hz. Conveyors transport existing world-drop stacks,
not a parallel inventory. All recipes use the existing TMI browser/autofill.

Placement axes persist in the existing journal. Power is reconstructed from
loaded indexed sections after reload, rather than saved as permanent energy.
Discovery is capped at 1024 loaded nodes/tick and 256 nodes/component. Incomplete
components stop. Replication is changed-only within 64 m, at most 256 states per
client. Client and server must both use protocol 78.

The rotor library has 25 shared meshes; no entity per tooth. Six-axis shaft/gear
poses, signed rotation, smooth stopping and unloading use existing material and
mesh ownership. Wheel axles are horizontal. There are no bevel gears, unequal
gear ratios or belt item insertion/extraction inventories. Over 128 simultaneous
on-belt stacks share the transport budget and therefore travel more slowly.

Recipes, mounting, loads and limits are documented in the
[block library](../../../../assets/terra/blocks/README.md#kinetic-drives-and-conveyors).

## Native Short Measurements

Actual machine: Ryzen 9 5900X, Radeon RX 6950 XT/RADV, Vulkan, X11, 1280x800,
view distance 20, high shadows/water/weather. Normal desktop applications were
left open. No compiler or test runner ran during the measurement windows.
Graphics and world TOML snapshots are in `configs/`.

The existing offline LOD smoke route moves camera/streaming focus 224 m out and
back. It exercises loading, meshing and rendering, **not authoritative walking
physics** (`controller_calls=0`). Clear seed 42042 uses 32-second sampling windows;
rain seed 7 uses 24 seconds. Initial loading and screenshot-adjacent frames are
excluded. No 10/15/30-minute soak was run, as requested.

| Scene / Build | AVG FPS | Minimum FPS | p95 ms | p99 ms | Frames below 32 FPS |
|---|---:|---:|---:|---:|---:|
| Clear, before changes | 50.50 | 22.54 | 31.89 | 35.62 | 105 / 1616 |
| Clear, grass/F25/exposure guard, before packing | 49.76 | 22.02 | 32.94 | 38.05 | 123 / 1592 |
| Clear, final binary, packing OFF | 50.11 | 22.50 | 31.71 | 36.50 | 91 / 1604 |
| Clear, final binary, packing ON | 51.59 | 25.22 | 32.56 | 36.73 | 112 / 1651 |
| Clear, packing ON, repeat | 49.53 | 23.68 | 33.52 | 38.14 | 124 / 1585 |
| Rain, before changes | 58.84 | 26.23 | 27.77 | 30.84 | 10 / 1412 |
| Rain, grass/F25/exposure guard, before packing | 58.05 | 30.24 | 27.50 | 30.86 | 12 / 1393 |
| Rain, packing ON | 58.59 | 29.04 | 27.73 | 31.00 | 12 / 1405 |

Controlled same-binary clear comparison:

| Metric | Packing OFF | Packing ON | ON repeat |
|---|---:|---:|---:|
| Mean CPU terrain upload interval | 1.7163 ms | 0.6483 ms | 0.6817 ms |
| Mean main schedule wall interval | 14.7664 ms | 13.4126 ms | 13.8242 ms |
| Peak admitted upload bytes/frame | 13,161,900 | 11,774,564 | 11,712,056 |
| Final resident memory, KiB | 2,613,040 | 2,587,036 | 2,586,744 |

Intervals overlap and are not additive. Upload bytes are CPU staging accounting,
not measured GPU transfers. These runs did not capture GPU pass timings, VRAM
usage or actual draw submissions. Each clear run finishes with 15,192 loaded
sections, 7,588 rendered sections and 166,183 represented props; queues drain.

**Result:** terrain upload CPU work is reproducibly lower. There is no robust
AVG/tail-FPS improvement across repetitions, and the 32-FPS floor is not met.
Do not describe the overall chunk-loading drop problem as solved. The repeated
run still spends substantial render-schedule wall time (18.56 ms), including
mesh allocation (3.98 ms body), preprocessing bindings (1.91 ms body) and shadow
bin uploads (1.15 ms body). Those are measured intervals, not an attribution of
all remaining time to one cause, nor justification to patch Bevy internals.

Raw logs for every row are in `native/`. The pre-change binary used an isolated
asset tree with the four old dirt definitions (`baseline-blocks/`), because that
binary predates the `coarse_face` schema. No repository assets were reverted.

Reproduce the clear short route with a fresh temporary world directory:

```bash
env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET WINIT_X11_SCALE_FACTOR=1 \
  TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/kinetics-grass-streaming-2026-10-28/configs" \
  TERRA_WORLD_DIR=/tmp/teramic-kinetic-benchmark-fresh \
  TERRA_BENCH_GRAPHICS=1 TERRA_BENCH_SECONDS=32 TERRA_BENCH_LOAD_SECONDS=120 \
  TERRA_CPU_SCOPES=1 WGPU_BACKEND=vulkan \
  target/debug/terra-client --smoke-test-lod
```

`TERRA_PACKED_TERRAIN=0` is a smoke-mode-only A/B override. Ordinary gameplay
always uses worker packing. For the rain route use a separate config directory
with `world-seed7.toml` as `world.toml`, `TERRA_BENCH_WEATHER=rain` and 24 seconds.

## Validation

Format, all-targets check, strict Clippy and workspace build (client and dedicated
server) passed. Final workspace suite: **2814 passed, seven existing ignored**.
Final command logs are in `tests/`. The workspace tests include deterministic
AO localization, mesh indices/attributes, PBR alias/tangents, negative section
edges, packed-frame reconstruction/splitting/byte budgets, pending exposure
reuse/release, signed kinetic graphs, cycles/overloads, vertical crank mounts,
partial fluid occupancy, current reversal, idle water, transport collision and
count preservation, unload/reload, recipe autofill and protocol validation.

The native grass gallery covers four close/coarse views and was actually
inspected before and after packing (`native/grass*`). The kinetic gallery passed
with three captures, 25 shared meshes and no warning/error log entries. It
injects positive, negative and stopped RPM to test rendering/asset stability;
it is not evidence of an authoritative power source. The separate integrated
crusher fixture passed real held Place input, mounted crank, release pause,
TMI, audio and all 19 recipe results, with five captures and no warning/error
entries. Authority behavior is also covered by server tests. Native logs and
captures are retained alongside the measurements.

```bash
cargo fmt --all --check
cargo check --locked --workspace --all-targets -j1
cargo test --locked --workspace -j1 -- --test-threads=2
cargo clippy --locked --workspace --all-targets -j1 -- -D warnings
cargo build --locked --workspace -j1
```

Native fixtures use `--smoke-test-fiverr-grass`, `--smoke-test-kinetics` and
`--smoke-test-crusher` with `TERRA_CAPTURE_DIR` pointing to an empty directory.
Fixture-only corrections keep the new 25-mesh shared library expectation,
detach inactive atmosphere environment lights in the isolated gallery, and
preserve preview-layer lighting when neutralizing the gallery's world sun.
