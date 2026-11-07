# Mechanical Mounting Wrench And Fine Sand

Mechanical placement now selects a compatible free port when a side click on
a shaft or device cannot make a direct connection. Both the placement outline
and authority use the same target and axis rules. Existing directly valid
gear meshes remain selected, including diagonal ratios and right-angle pairs.
Obstructed ports, unloaded clearance and forged placement targets remain
invalid.
Holding a mechanical part also takes priority over opening a crusher or
turning an existing crank, so PreUpdate no longer blocks the placement before
it reaches these rules. Empty-hand, food and ordinary-block use retain their
existing station/crank behavior.

## Conveyor And Small Gears

The conveyor surface is 0.625 m high. Its axle is at y = 0.5 m and spans both
cell edges. Raised supports, bearings, band geometry, collision, item transport
and gravity use these shared dimensions. Small gear tooth tips now have a
true circular radius of 0.5 m instead of protruding 5 cm below a floor at the
same axle height. This preserves clearance checks rather than letting a
mechanical part penetrate solid terrain. Small-small contact is tangential;
deep tooth interpenetration is not physically possible at a 1 m center spacing
with this strictly floor-clear radius. Transmission rules are unchanged.
Large gears and water wheels keep their existing larger radii and require
real free space.

## Fine Sand

All gray pebbles have been removed from the texture and geometry. The existing
512 px procedural texture now has cohesive golden colors, fine two-pixel grain
and subtle curved wind ripples. Surface relief is 4 mm, not 18 mm, and keeps
its planar seam border. Full detail uses at most 28 vertices per visible face,
down from 148; there is still one shared material rather than per-grain assets.

## Wrench

The non-stackable `teramic::wrench` uses the normal tool rendering and held
pose, with an open metal head, wooden grip and 1 m equipped length. It is made
at a workbench from three iron ingots and a stick, and appears in Tools/TMI.

Right click/place cycles a directional block's orientation; Shift reverses.
Ordinary blocks and fences remain unchanged, and pots do not have their
contents rotated. The same action covers stairs, mixed slabs, shafts/gears,
conveyors, chimneys, doors/windows and paired furniture. Colors, wood species,
door/window open bits, chest inventories and machine progress are preserved.
Cranks follow their owner and occupied beds reject rotation.

The server validates the held item, inventory/section revisions and target
ray, then checks placement clearance and player bounds. A bounded transaction
preflights affected blocks and sparse section metadata before changing the
world. It never clones the whole world/journal or calls the mining/drop path.
Blocked, stale or unloaded rotations cannot leave half an object behind.
Client and server both require protocol 80; block IDs and generator/save
versions are unchanged.

## Validation

Commands use the existing optimized development profile and `--locked`.
Native checks run separately from Cargo jobs on Vulkan/RADV. No long soak
test or overall FPS improvement is claimed for this change.

```bash
cargo fmt --all --check
cargo check --locked --workspace --all-targets -j1
cargo test --locked --workspace -j1 -- --test-threads=2
cargo clippy --locked --workspace --all-targets -j1 -- -D warnings
cargo build --locked --workspace -j1
```

Native entry points are `--smoke-test-materials`, `--smoke-test-kinetics` and
the short three-stage `--smoke-test-wrench`. The hand fixture is a rendering
check; authoritative rotations and rejected forged requests are exercised
separately by network/world tests.

### Completed Checks

| Check | Result | Log |
|---|---|---|
| Formatting | Passed | [fmt](logs/teramic151-fmt.log) |
| Workspace all-targets check | Passed | [check](logs/teramic151-check.log) |
| Workspace tests | 2,888 passed, 7 existing ignored, no failures | [tests](logs/teramic151-tests.log) |
| Strict workspace all-targets Clippy | Passed with `-D warnings` | [Clippy](logs/teramic151-clippy.log) |
| Client and dedicated-server build | Passed | [build](logs/teramic151-build.log) |
| Native wrench | Passed idle, action and third-person stages | [wrench](logs/teramic151-wrench.log) |
| Native kinetics | Passed connected transmissions, fractional RPM, icon readbacks and raised conveyor ports | [kinetics](logs/teramic151-kinetics.log) |
| Native materials | Passed all 151 block definitions and GPU previews, terrain and construction views | [materials](logs/teramic151-materials.log) |

Native captures ran at 1280 x 800 on the RX 6950 XT using Vulkan/RADV and
Mesa 26.2.5-arch3.1 under X11. All three final native logs are free of warnings
and errors. They ran after Cargo completed, with isolated temporary config
directories. These were short rendering checks, not FPS benchmarks or soaks.

The regression tests exercise actual server picking and held-item validation,
the UI's PreUpdate placement gate, both crank-input paths, all supported
orientation families, contents/progress preservation, negative section borders,
blocked/unloaded footprints and late revision/mesh/water-epoch overflow.

The extended kinetic gallery initially exposed a genuinely overloaded wooden
end shaft at 7.5 RPM. The fixture now uses a rated iron shaft there; source
strength and production load limits were not increased. Its final gallery
retains 31 shared moving meshes. The older conveyor-height assertion was
updated to the shared new height constant, not removed.

### Captures

- [Wrench in first person](captures/hands-18.png)
- [Wrench action](captures/hands-19.png)
- [Wrench in third person](captures/hands-20.png)
- [Connected kinetic train](captures/kinetics-forward.png)
- [Raised conveyor with flush shaft connection](captures/kinetics-conveyor-ports.png)
- [Fine stone-free sand beside the other terrain materials](captures/stone-free-sand.png)

The first/third-person wrench, close conveyor connection and sand terrain
captures were visually inspected. No pixel-identical reference or general
FPS improvement is claimed.

## Main Code Paths

- `terra-world/src/targeting.rs` and `placement.rs`: shared mechanical port
  targeting and orientation.
- `terra-core/src/kinetic.rs`, block JSON/glTF models and
  `tools/assets/build_kinetic.mjs`: dimensions and connected geometry.
- `terra-render/src/blocks/sand.rs` and `surface.rs`: stone-free sand.
- `terra-world/src/wrench.rs` and `edits/rotation.rs`: orientation planning and
  atomic inventory-preserving commits.
- `terra-network/src/server_wrench.rs`, client requests and protocol: authority.
- `terra-player/src/interaction/mining.rs` and UI inventory input routing:
  wrench interaction without opening the device UI or cranking it.
- `terra-render/src/previews/tools/wrench.rs`: shared 3D tool model and preview.
