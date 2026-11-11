# Character editor and grade recovery verification

The change adds local and per-server appearances, reference player/NPC models,
and corrects wheel clearance at rail grade transitions.

## Native editor

The ordinary Vulkan client was run with isolated configuration and world
directories under `target/live227`. The opt-in `character_live` harness pressed
and released real UI pointer targets; color changes used the materialized
color-picker handler. No original world or player profile was opened for writing.

All 47 scripted steps passed, including:

- Opening the lower-left main-menu button and selecting female hairstyle F10.
- Independent hair, shirt and trouser colors, without changing the saved profile
  until Save.
- Reset, Cancel, save/reopen persistence, and preserved UUID/name.
- Saving a separate server appearance without changing the local default.
- First-join inheritance and cancellation without starting a game connection.
- Visible control bounds and undistorted 640:896 preview at 1280×720,
  1920×1080, 2560×1440 and 3840×2160.

Both Bevy render captures and direct native-window captures were inspected.
The latter avoid relying on an offscreen test's interpretation of layout.
The final repeat's window manager constrained the requested UHD window to
3828×2043; the earlier UHD run used 3840×2160. Observed sizes are logged by each
layout check, and the final direct captures retain their actual dimensions.
Local artifacts: `target/live227/character-ui-result.json`, `editor.log`,
`captures/native-editor-*.png` and `gallery/{players,npcs}.png`.

## Final checks

- `cargo build --locked --workspace --bins --jobs 2`: passed.
- `cargo clippy --locked --workspace --all-targets --jobs 2 -- -D warnings`:
  passed.
- `cargo test --locked --workspace --lib --bins --jobs 2 -- --test-threads=4
  --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images`:
  **4540 passed**, zero failures, ten pre-existing ignored tests. The single
  explicitly filtered test requires the original, already deleted
  `FIVERR/VoxelGrassBlock glb file.glb`; the user's deletion was preserved.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Train asset lint: 85 models, 933 bounded collision boxes and 417 recipes passed.
- SHA-256 verification: all 17 original files remain identical, including all
  15 files of the Test world, the local profile and the pre-existing gameplay
  configuration.

Logs: `target/character227-{build,tests,clippy}-verified.log`,
`target/character227-assets-final.log` and
`target/live227/original-hashes-verified.json`.

## Rail clearance

The original Test world was read without writing it. Its affected locomotive was
reproduced in an isolated test and traversed 16 m downhill, then 32 m back uphill.
The production collision path allows at most 0.5 m of suspension travel on
real, loaded rail supports. Walls, unknown terrain and
body/decorative parts retain normal collision checks.

Regression cases cover a broad locomotive crossing a short crest in both
directions, reversing away from a real wall, and rejecting wheel clearance on
unrelated terrain. Existing saved wheel profiles and registry geometry budgets
are preserved, including worlds whose trains already approach the existing limit.

## Scope

The models are authored three-dimensional reconstructions of the supplied front
views. Their unseen backs and articulation are authored rather than extracted
from original 3D assets. New NPC roles use existing persistence, settlement and
navigation paths; this change does not implement the separate trading/work
economy tasks.

Local evidence remains under ignored `target/` directories. Large screenshots,
private saves and machine-specific configuration are not added to the repository.
