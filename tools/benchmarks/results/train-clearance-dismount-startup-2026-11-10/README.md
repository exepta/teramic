# Train running gear, coupling, dismount and startup — 2026-11-10

Driving-wheel treads now match the existing **2 m rail gauge**. Bogie wheel
meshes are omitted when driving wheels are present; bogie structure, axles and
rail-contact routing remain functional. Existing saved wheel profiles use the
corrected canonical wheel collision shape, matching the new mesh and pivot.

The vehicle workshop previously returned before dispatching Couple/Uncouple if
no station was set. Those actions now accept an optional station. The authority
still checks ownership, permissions, revision, stopped state and proximity;
an owner seated in either car can operate that pair. Geometric coupling checks
remain required. Client and server require **protocol 126**, without changing
persistent save formats.

Dismount uses one shared physics implementation for the authority and saved-world
diagnostics. It checks a lateral-then-down route from tall cabs and can land on
an actual free carriage platform when no dry terrain landing is reachable.
Loaded terrain and other vehicles remain obstacles. Final platform positions
must be fully clear and have real supporting geometry.

## Verification

- Workspace: **4506 passed, 10 ignored, 1 filtered**, 12 library/binary suites.
  The sole filter excludes the already deleted original FIVERR GLB fixture; its
  external deletion was preserved. All other tests, including real UDP cases, ran.
- Strict workspace Clippy with all targets and `-D warnings`, locked build,
  formatting, whitespace and the CPU asset lint passed. All **85 train parts**
  retain valid models, collision bounds and nonempty projected inventory icons.
- Regressions exercise the actual HTML coupling handler without a station;
  server coupling/release and stale/out-of-reach rejection; canonical saved wheel
  profiles; matching driving-wheel gauge and absence of duplicate bogie wheels;
  a fueled decorated locomotive advancing through a clear sweep; and platform
  dismount above a void in all four building orientations.
- A disposable copy of the current user `Test` world was inspected using the
  real saved journal, regenerated terrain with saved overrides, train geometry,
  rail motion and shared dismount search. Both saved vehicles have clear static
  clearance. The locomotive advances for 60 ticks in both directions, about
  0.625 m each, with clear swept shapes. Moving its saved pose to a compatible
  **1 m coupling gap** reproduces the close-car case: coupling geometry is valid
  and dismount now succeeds. All ten passenger seats also have a safe exit.
  Private world coordinates and player records are not published here.
- Original world content is verified against SHA-256 hashes: **11 files
  unchanged**, excluding the runtime session lock.

```sh
cargo test --locked --workspace --lib --bins --jobs 2 -- --test-threads=4 --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
cargo clippy --locked --workspace --all-targets --jobs 2 -- -D warnings
cargo build --locked --workspace --jobs 2
cargo fmt --all -- --check
python3 tools/assets/lint_train_parts.py
git diff --check
# Supply a disposable world-library copy and its world ID:
cargo run --locked -p terra-client --example inspect_train_save -- <copied-world-library> <world-id>
```

## Startup comparison

The normal frontend still waits for actual models, textures, fonts, UI images,
icons and audio. It no longer serially renders every inventory variant before
revealing the menu. Visible inventory slots request their previews through the
existing bounded queue. Full catalog warmup remains available explicitly for
diagnostics with `TERRA_PREVIEW_WARMUP=1`.

Two isolated native startup-only runs, same 1920×1080 windowed configuration,
Vulkan/RX 6950 XT, vsync off and no frame cap:

| Preparation | Inventory previews before menu | Process duration |
| --- | ---: | ---: |
| Explicit full catalog | 617 | 18.448 s |
| Normal demand-driven previews | 0 | 5.774 s |

Both exit successfully. They verify the loading title, progress numbers and
centered asset path, intermediate fade opacity, no premature menu actions and
no world/server creation. Both retain 1611 UI icon images and 1642 UI assets.
The normal renderer gate counts one model-library readiness step; this is not
an inventory preview. These are single complete smoke-process timings, including
captures and shutdown, with warm OS/GPU caches. They are not cold-start guarantees.

Native workshop checks cover 1080p/720p frames and actual pointer dispatch;
model captures cover the corrected wheel meshes and visible exhaust. These are
controlled visual fixtures. Authoritative motion/coupling behavior is checked
separately above.

The copied `Test` world was also loaded by the native client and its real
integrated server. The final run synchronized, rendered the saved locomotive
on its track and exited successfully without WARN/ERROR/panic messages. Its
private screenshot and world log remain local. This short scene check does not
replace the separate saved-motion and dismount probes.

Screenshots: [loading](startup-loading.png), [fade](startup-fade.png),
[running gear](running-gear.png), [workshop](workshop.png).
Reviewed logs and counts accompany this report; raw private-world diagnostics
and copied save/profile data remain local.
