# Inventory sorting, character materials and rail-builder activation

Date: 2026-11-10. Native checks used fresh profiles and isolated configuration/world
directories in `target/live230`. All 17 monitored original profile/world files
retained their pre-run hashes.

## Implementation and regression coverage

- The rail-builder button was missing from the component's accepted command list.
  Real HTML events on the button and its children now produce the authoritative
  enable/disable requests. A server scenario seats the driver, enables the module,
  drives on a continuous track, clears an obstacle and extends the track. Survival
  consumes cargo materials; Creative does not. Working speed remains at most 3 m/s.
- Backpack and supported storage inventories offer independent type/quantity
  sorting. Exact item metadata and totals, the hotbar, reserved crafting slots,
  cursor restrictions, revisions, access checks and atomic double-chest updates
  are covered. Real UDP tests verify two-client synchronization. Functional machine
  slots reject storage sorting; train cargo and backpack use existing train access
  and stale-response handling.
- Each garment independently selects linen, canvas, leather or wool. Legacy
  profiles receive suitable defaults. Preview/save/reset/cancel and per-server
  profiles retain the choices. Geometry tests check changed surfaces, valid mesh
  data and unchanged unrelated body parts. The cape, boots and faces received
  additional modeled details. Human reference-art approval remains A01.

## Checks

**4581 distinct tests passed across the workspace run and corrective reruns.**
The initial run had five failures: four new fixtures assumed 64-item stacks or
attempted to clear an already empty inventory, and one stylesheet used a `gap`
shorthand unsupported by TiltUI. Fixtures now derive actual stack limits and test
overflow explicitly. The stylesheet uses separate row/column gaps. All affected
tests passed on rerun. Ten existing tests remain ignored; the original FIVERR GLB
reader test is explicitly filtered because its input was already absent.

Strict workspace/all-target Clippy, formatting, the real stylesheet parser and
the client/server build passed. Client and server require protocol **132**.

The native character editor passed **88 steps**: real pointer clicks and preview
rotation, four material changes through the production change-event path,
persistence, cancellation, server overrides and layout bounds. Dropdown values
were injected as change events; individual popup choices were not clicked by the
pointer harness. Actual capture sizes were 1280×720, 1920×1080, 2560×1440 and
3828×2043. The window manager clamped the requested 3840×2160 window; the harness's
resolution list records requests, while `results.json` records actual captures.

The native inventory gallery passed **26 views** at 1680×936 and 1024×640,
including player, chest, double chest, connected workbench and machine inventories.
Slot bounds, no content scrolling and unchanged authoritative player inventory
were checked. Sort headers were also inspected in the captures. One initial wide
player capture contains the ordinary territory-entry notification over the UI;
the later compact capture shows the header unobscured.

There were no native stylesheet loading errors or panics. Logs contain a transient
zero-size text-span warning during editor tab replacement and one audio-buffer
underrun during the inventory fixture startup. These did not fail the UI checks.

## Captures

- [Character materials, Full HD](character-materials-fhd.png)
- [Character materials, HD](character-materials-hd.png)
- [Cape and boots](character-reference-dressed.png)
- [NPC gallery](npcs.png)
- [Player inventory, compact](player-inventory-compact.png)
- [Double chest](double-chest-wide.png)
- [Connected workbench, compact](workbench-linked-compact.png)

These are native rendered images. Models are authored from the supplied reference
artwork, not imported original 3D assets.

## Commands and local logs

```sh
cargo test --locked --workspace --lib --bins --no-fail-fast --jobs 2 -- --test-threads=4 --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
cargo test --locked --workspace --lib --bins --jobs 2 sort -- --test-threads=4
cargo clippy --locked --workspace --all-targets --jobs 2 -- -D warnings
cargo build --locked --workspace --bins --jobs 2
cargo fmt --all -- --check
```

The stylesheet parser was rerun directly through the built `terra-ui` test binary.
Logs: `target/phase230-tests.log`, `target/phase230-sort-rerun.log`,
`target/phase230-styles.log`, `target/phase230-clippy-final.log`,
`target/phase230-build.log`, and `target/live230/{editor,inventory,npcs}.log`.
Isolated launch settings are recorded in `target/live230/start_client.py`.
