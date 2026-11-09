# Train exit, saved rejoin and isolated workshop preview — 2026-11-09

Train dismount searches loaded ground beside tall/wide bodies, preserving terrain
and other-vehicle obstruction checks. Mounted/pending-exit positions no longer
replace the last standing save location. Legacy saved positions are recovered
against the same train shapes used by the spawn collision check.

The workshop provides **Aussteigen** and a two-step **Zug zerstoeren** action.
Destruction permanently deletes the selected vehicle and its cargo/fuel. The
server requires ownership, current revision, reach, build/inventory permission,
a stopped uncoupled vehicle and no remaining occupants. The owner leaves first.
Reliable removal metadata distinguishes deletion from Montage, so replicas do
not wait forever for terrain blocks that will never be restored. Protocol 124;
persistent train/save formats remain unchanged.

The new three-wide locomotive guide raises the cab seat, floor, windows and roof
by two further blocks, with a connected rear stair. Existing custom vehicles are
not automatically rebuilt. Workshop rendering uses its own layer 25, separated
from world miniature terrain (28), creatures (27), environment (26), other
previews and the main camera. Soil-growth grass consumed by grazing animals is
excluded from the unrelated generic forage timer.

## Automated checks

- Workspace: **4496 passed, 10 ignored, 1 filtered**.
- The unfiltered run fails only on the missing, separately deleted original
  `FIVERR/VoxelGrassBlock glb file.glb` fixture. Its filesystem deletion was left
  untouched. Every other library/binary test was run, including real UDP legacy
  train-position recovery and reliable destruction reaching owner and observer.
- Strict workspace Clippy, all targets: passed with `-D warnings`.
- Locked workspace build, formatting and whitespace checks: passed.
- Regression cases include tall-cab dismount, blocked/unloaded exit retention,
  thin-roof recovery obstruction, preserving standing saves while seated,
  unauthorized/stale/out-of-reach deletion, actual HTML button dispatch and
  confirmed dismount before deletion, collision retirement without voxel packets,
  preview layer separation and grazing soil-growth grass.

```sh
cargo test --locked --workspace --lib --bins --jobs 2 -- --test-threads=4 --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
cargo clippy --locked --workspace --all-targets --jobs 2 -- -D warnings
cargo build --locked --workspace --jobs 2
cargo fmt --all --check
git diff --check
```

## Native checks

Native AMD Radeon RX 6950 XT/Vulkan client, using isolated configuration/save
directories. No WARN/ERROR/panic messages in the three completed runs.

- Workshop fixture: 1920×1080 and 1280×720, panel/control bounds checked, actual
  pointer press/release on **Zug zerstoeren** opens the confirmation. Large magenta
  geometry deliberately occupies the miniature/environment layers at the same
  coordinates as the studio; neither sentinel appears in the preview. This is
  a UI-only fixture with controlled session state, not a real vehicle transaction.
- Current user `Test` world: copied, loaded with the saved player identity and
  location, fully synchronized with its real integrated server, rendered and
  closed cleanly. The same copy was then opened and rendered again after saving.
  Both native runs exit 0. All **8 original save files** match their original
  SHA-256 hashes; session lock files were excluded. World screenshots and saved
  player data are not included here; player UUIDs/names in logs are redacted.
- These short rejoin runs establish functional loading, not a sustained
  performance benchmark or live multiplayer destruction test.

Screenshots: [workshop 1080p](train-workshop-1080.png),
[workshop 720p](train-workshop-720.png),
[confirmation 1080p](train-driving-1080.png),
[confirmation 720p](train-driving-720.png).
