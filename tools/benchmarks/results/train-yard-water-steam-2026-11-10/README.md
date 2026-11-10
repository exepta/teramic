# Assembly berths, water around curved rails and F53 steam services — 2026-11-10

Assembly searches the actual nearby axle positions on a complete rail path.
A long authored module may contain a locally usable berth despite a later bend,
a slight sideways drift or an unloaded remote owner section. The two bogies must
still match real loaded gauge slices. A bounded local search replaces rejection
of the entire module for a tiny perpendicular offset.

Loaded assembly stations mark the nearest rail with a combined procedural mesh:
metal side plates, yellow hazard stripes and clamps following the real path.
These are visual service markings and add no colliders or extra track items.

Water surfaces use the six exact planes of the same solid beams that draw curved
rails and sleepers. Convex clipping retains wet corners that disappeared when
coarse axis-aligned collision bounds were used. Natural surfaces above submerged
curves and both switch branches remain complete. Physical collision stays intact.

## Remaining F53 operation

- Parked, braked tank cars accept finite water from an assembly dock connected
  to the existing pipe network, a real adjacent open pipe mouth, or an overhead
  Fluid Infucer. Each supplied unit debits 125 mB from its actual source.
- A genuinely connected, loaded tender can supply a locomotive from its finite
  tank. Ownership and kingdom transport/inventory permissions are enforced.
- Dismantling vents pressure and preserves tank water in the dock. Reassembly
  restores it to a tank; ordinary pipes can drain it. Overflow rejects the
  dismantle transaction before any ownership or block changes.
- A parked installed boiler at working pressure powers one adjacent axial shaft
  through the existing kinetic solver: 40 RPM and 128 capacity. One boiler cannot
  duplicate source capacity through several docks or shafts. Real mechanical
  load consumes water and pressure; unloaded or unpermitted vehicles cannot
  supply it. Service budgets and per-car elapsed time remain bounded.
- Physical curve radius and forward stopping distance limit all cars in a
  consist. Critical overspeed secures the train at its last valid pose with a
  derailment-danger reason; it does not destroy the vehicle. Live ballast limits
  are evaluated through the existing function, with its 50% bonus applied once.
- The travelled local gauge must be loaded; a distant missing owner section does
  not block known local movement. Unknown track, buffers and obstructions fail
  closed. Repair/load the route and deliberately release the brake to resume.

Protocol 128 is required by client and server. Only new dock water reserves add
an RG14 outer region wrapper; old RG12 plumbing bytes and RG13 curve payloads
remain unchanged. Invalid, duplicate, misplaced and orphaned reserves are rejected.

## Verification

The complete workspace suite passes **4523 tests**, with 10 existing ignored
checks and only the separately missing original FIVERR GLB fixture filtered out.
The water-reserve regression wraps the exact prior region bytes, roundtrips the
reserve and rejects corrupt/orphaned inputs. The server regression uses the real
kinetic solver and pump; it does not inject a fabricated running state. Curve
limits, critical overspeed, the single ballast bonus, local rail unload and
explicit post-save resumption have focused passing regressions.

The saved-world CPU inspection uses the user's disposable Test copy and validates
real 3 m, 8 m and 15 m wheelbases with the actual creative build planner. All three
produce valid guides. SHA-256 checks cover all 14 original world files; no original
file is changed. Private save data and coordinates are excluded from this report.

```sh
cargo test --locked --workspace --lib --bins --jobs 2 -- --test-threads=4 --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
cargo clippy --locked --workspace --all-targets --jobs 2 -- -D warnings
cargo build --locked --workspace --bins --examples --jobs 2
cargo fmt --all -- --check
python3 tools/assets/lint_train_parts.py --log target/train226-assets.log
```


Native Vulkan/TiltUI inspection against a disposable Test-world copy passes the
actual workshop handler, materialized preview, world guide and real build
planner: the selected 8 m / 3x locomotive exposes 116 valid missing cells.
The inspection moves only the copied player viewpoint; it never assembles a
train or changes a user's original save. Screenshots and raw private-world logs
remain local under `target/live226` rather than being published in this report.

Strict Clippy (`-D warnings`), workspace binary/example builds, formatting,
diff whitespace validation and the train asset/catalog linter pass. These are
functional and visual regressions, not a claim of long-duration multiplayer
coverage or a new frame-time benchmark.

Additional focused Vulkan scenes inspect the real Test-world assembly track and
shoreline curve from temporary spectator viewpoints. Native window captures show
the yellow/metal service markings and continuous water around the curved rails;
the original rectangular dry cutouts are absent. These scenes use copied saves,
not generated replacement terrain. The spectator scenes do not test train
operation or player permissions; those are covered by the workspace regressions.
