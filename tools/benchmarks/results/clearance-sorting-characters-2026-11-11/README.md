# Configurable clearance, sorting dropups and character editing

Completed 2026-11-11, Europe/Berlin. Native logs use UTC. Tests used isolated
profiles, configuration and world paths under `target/live231`. All 17 monitored
original world/profile files retained their pre-run hashes.

## Behavior covered

- The parked driver can set an odd clearance width from 3–15 blocks and a height
  from 3–32 blocks, or use Auto. New vehicles default to width 5 and Auto. Auto
  follows the actual model bounds, including roof/chimney/decorations, with one
  block of headroom. Server authority checks ownership, driver seat, motion and
  dimensions before changing state. Existing-track clearing and extension use the
  same profile. Frozen ID18 saves migrate to ID19 while retaining motion and cargo.
- Icon-only funnel buttons open upward with A–Z, ID and quantity choices. Names
  use the shared display-name catalog; block IDs sort numerically. Atomic sorting,
  metadata, stack limits, reservations, access checks and hotbar protection remain
  covered by core, UDP and UI transaction tests. Active linked/train menus sit
  above their dismissal layer, while underlying storage slots remain blocked.
- There are 12 male and 14 female hairstyles, with real colored highlights.
  Independent eyes preserve the old shared color on profile migration. Four tabs
  separate body, hair, face and clothing controls. Hands have corrected palm/back
  orientation and nails; torso, knees and calves have additional contours. The
  random button uses an available dice icon and the footer stays inside its frame.
- Dragging the inventory character rotates a live shared avatar with equipment.
  Rotation reuses its meshes and render target. The image preserves its aspect
  ratio, menus block dragging underneath, and closing removes the live scene.

## Results

**4598 distinct tests passed** across the workspace run and one corrective rerun.
The initial run had one failing new height fixture: an ordinary decoration was
missing its `TrainBlockView` shape. It now builds a valid connected decorated
vehicle and verifies its bounds grow with the roof. The corrected test passed.
Ten existing tests remain ignored. The original FIVERR GLB reader test is filtered
because its input was already absent from this checkout.

Strict workspace/all-target Clippy, client/server builds, formatting, whitespace
and the actual TiltUI stylesheet parser passed. Protocol is **133**, train record
format **19**. Client and server must be restarted together.

The final native editor run passed **125 steps**. Real pointer events exercise
tabs, new hairstyles, highlights, random/reset/cancel/save, rotation and per-server
choices. Color/material values use the production change-event path. Checks cover
the outer frame, inner control frame, field rows and labels. Requested window sizes
were HD, Full HD, QHD and UHD; actual captures were 1280×720, 1920×1080, 2560×1440
and 3828×2043, with the window manager clamping the requested UHD window.

The final inventory gallery passed **30 captures** at 1680×936 and 1024×640:
24 station views, two player views, two linked dropups, player dropup and rotated
portrait. It verifies real pointer toggles, menu stack ordering, bounds, live
rotation and unchanged authoritative inventory. The first editor inspection found
a hair-tab overflow; the final two-row layout passes the stronger inner-frame
checks. Harness fixes order synthetic input before picking and label compact
captures correctly. One intermediate gallery run reported an inventory-baseline
change; it did not recur in the subsequent recorded runs or the complete final
run. Menu-toggle logs contain no sort transaction.

No panic or stylesheet loading error occurred in the final native runs. A
transient Bevy warning about a zero-size text span during reactive control
replacement remains in both logs. The figures are authored voxel models;
human reference-art approval remains A01.

## Native captures

- [Female hairstyles and highlights](hair-female-fhd.png)
- [Male hairstyles](hair-male-fhd.png)
- [Hair controls at HD](hair-hd.png)
- [Independent eyes](independent-eyes.png)
- [Inventory dropup](inventory-sort-compact.png)
- [Inventory rotation](inventory-rotation-compact.png)
- [Linked storage dropup](linked-sort-compact.png)

## Reproduction and logs

```sh
cargo test --locked --workspace --lib --bins --no-fail-fast --jobs 2 -- --test-threads=4 --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
cargo test --locked --workspace --lib --bins --jobs 2 automatic_clearance_covers_tall_roofs_and_updates_with_vehicle_geometry -- --test-threads=4
cargo clippy --locked --workspace --all-targets --jobs 2 -- -D warnings
cargo build --locked --workspace --bins --jobs 2
cargo fmt --all -- --check
```

Logs: `target/phase231-tests.log`, `target/phase231-clearance-rerun.log`,
`target/phase231-clippy-final.log`, `target/phase231-build-final.log`,
`target/phase231-styles.log`, `target/live231/editor.log` and
`target/live231/inventory.log`. Isolated launch settings are in
`target/live231/start_client.py`; earlier diagnostic runs remain in separate
subdirectories. Actual capture sizes and totals are in [results.json](results.json).
