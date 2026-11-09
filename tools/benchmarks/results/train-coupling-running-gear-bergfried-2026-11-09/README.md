# Train coupling, running gear and Bergfried UI — 2026-11-09

Coupling now uses continuous world positions and outward drawbar directions,
including parked curves and slopes. Both ends must face each other, have at most
0.35 m lateral offset and a center distance of 0.55–2.15 m. Selecting the second
vehicle preselects the closest compatible free ends. Rear blueprint couplers face
outward in every building orientation.

Train presentation uses a bounded 32-sample buffer with 60 ms delay. Wheel travel
and speed recover sample spacing when reliable packets arrive in a burst.
Body, riders and wheels share the same presentation time; interpolation takes the
short yaw path and never extrapolates past confirmed poses. Teleports and long
gaps reset the buffer. Protocol 125 is required on client and server; persistent
save formats are unchanged.

The three-wide locomotive guide includes 1.7 m driving wheels. Black tires,
steel rims, red spokes, hubs, counterweights and crank pins are actual geometry.
Small wheels and bogies use the same construction. Updated bounds, animation
pivots and crank direction agree with the new meshes. Existing custom vehicles
are not automatically rebuilt; Montage allows adding the new guide parts.

The Bergfried layout follows the supplied reference using TERAMIC's copper,
brown and gold theme. It includes identity/banner editing, upgrade costs, land
counts, current-chunk actions, a member/permission table and diplomacy rows.
The icon picker, color picker, role editing, founding types and dissolution
confirmation retain their actual handlers and permissions. Compact-height
overview columns keep all three claim buttons visible. The map diplomacy tooltip
has bounded text size and wrapping. Action icons use the shipped icon catalog.

## Automated verification

- **4500 passed, 10 ignored, 1 filtered**, all 12 workspace library/binary suites.
  The single filter excludes the already deleted original
  `FIVERR/VoxelGrassBlock glb file.glb` fixture. That external deletion was left
  untouched; every other test ran, including real UDP train authority cases.
- Regression coverage includes fractional coupling on curves/slopes, opposite
  endpoints, atomic symmetric links, all blueprint orientations, automatic UI
  endpoint selection, bursty-packet constant-speed interpolation, no
  extrapolation, common body/rider sampling and crank-pin/rod agreement.
- Strict Clippy with all targets, locked workspace build, formatting and
  whitespace checks passed.
- Asset lint: **85 parts**, valid JSON/recipes, glTF indices/bounds/textures,
  finite collision boxes, consistent winding/normals and nonempty projected
  inventory icons. See [asset-lint.log](asset-lint.log).

```sh
cargo test --locked --workspace --lib --bins --jobs 2 -- --test-threads=4 --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
cargo clippy --locked --workspace --all-targets --jobs 2 -- -D warnings
cargo build --locked --workspace --jobs 2
cargo fmt --all -- --check
python3 tools/assets/lint_train_parts.py
git diff --check
```

## Native verification

AMD Radeon RX 6950 XT/Vulkan; isolated configuration, capture and save directories.
No existing user world was opened or changed. The three final native runs exit
successfully without WARN, ERROR or panic messages. Logs redact player identities.

- **Bergfried:** actual integrated server founding/appearance acknowledgements;
  searchable full icon catalog; themed color picker; frame checks at QHD,
  1024×768 and 4K. All three overview claim actions must be fully inside the
  visible scroll viewport. Real secondary Visitor join, Worker grant/revocation,
  rejected self-promotion and authenticated reconnect/read-only controls pass.
  Actual map pointer hover checks the diplomacy tooltip text rectangle and
  materialized font size. [Run log](kingdom-native.log).
- **Workshop:** 1080p and 720p frame/control checks, actual pointer press/release
  opens destruction confirmation; isolated preview excludes geometry on other
  miniature layers. Controlled UI fixture, not a server transaction.
  [Run log](workshop-native.log).
- **Running gear:** native world renderer captures the assembled broad guide,
  different wheel phases and visible exhaust. This is a visual fixture, not
  authoritative driving or a sustained frame-time benchmark. Packet jitter and
  real train authority are covered by automated tests. [Run log](model-native.log).

Screenshots: [Bergfried QHD](bergfried-qhd.png),
[compact overview](bergfried-compact.png),
[members and permissions](bergfried-members.png),
[diplomacy tooltip](diplomacy-tooltip.png),
[workshop](train-workshop.png), [running gear](train-running-gear.png).

Original visual references: [locomotive](../../../../docs/references/locomotive-running-gear-223.png),
[Bergfried layout](../../../../docs/references/bergfried-layout-223.png).
