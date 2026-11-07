# RGB Fabrics, Filtered Funnel And Belt Merger

Date: 2026-10-29. Functional validation, not an FPS benchmark.

## Implemented

- Exact RGB on wool, carpets, bed fabric and conveyor ribbons; existing cape
  support is retained. Wood and metal are not recolored. RGB and wood survive
  crafting, placement, creative pick, mining, bed rotation and save/reload.
  Wool-to-carpet crafting preserves the source color. An existing RGB item can
  show its specific coloring recipe without enumerating colors in TMI.
- `teramic::filtered_funnel` (157): one non-consuming ghost item and an
  Allowed/Blocked switch, default Allowed. Matching includes exact RGB and
  wood identity. An empty filter passes all items. Rejected drops rest at the
  mouth; blocked, full or unloaded outlets do not destroy or duplicate items.
- `teramic::belt_merger` (158): three possible input directions and one oriented
  output on the existing powered conveyor system. Round-robin arbitration,
  blocked-outlet backpressure and the ordinary belt entry gate prevent bypass
  and starvation. Existing drop IDs and counts are preserved.
- Both parts have shared reusable models, icons, placement and workbench
  recipes in the Technic catalog. No new item or material per RGB value.

## Resource And Storage Rules

Tint changes update appearance without terrain/collider/water rebuilds.
Moving single belts share a material and use a tagged RGB value; changing tint
does not restart belt UV animation or transport. Dynamic held/drop/icon caches
are bounded to 128 entries and eviction does not manually remove live assets.
Sparse RGB and filter metadata use the RGA7 wrapper around the previous region
layout. Old serialized section fields remain frozen, and regions without new
metadata keep their existing format. Protocol 86 requires matching client and
server versions. Existing block IDs are unchanged.

The existing logistics budget processes at most 64 ports per 0.5-second tick.
Its preceding loaded-port sort and drop indexing still inspect the full loaded
sets; this change does not claim to remove that existing scaling limit.

## Automated Checks

All commands exited successfully:

```sh
cargo fmt --all --check
cargo check --locked --workspace -j1
cargo test --locked --workspace -j1 --no-fail-fast
cargo clippy --locked --workspace --all-targets -j1 -- -D warnings
cargo build --locked --workspace -j1
```

The final complete suite passed **3193 tests**, failed **0**, with **7 existing
tests intentionally ignored**. Client and dedicated server were built.

Regression coverage includes exact RGB crafting and ingredient consumption;
negative coordinates, section/region boundaries and repeated save/reload;
legacy region binary layout; tint-only replication without terrain rebuilds;
both bed halves and all four rotations; invalid/stale placement atomicity;
ghost authorization and denied inventory transfers; real gravity at funnel
mouths, full/unloaded/blocked outlets and manual collection; and all four
merger orientations with real RGB drops traversing the existing belt system.
Renderer tests cover sRGB transfer, material isolation, bounded variant caches
and stable belt entities, shared material counts and animation phase.

Logs:

- [Final tests](teramic158-tests-final.log)
- [Workspace check](teramic158-check-verified.log)
- [Strict Clippy](teramic158-clippy-verified.log)
- [Formatting](teramic158-fmt-verified.log)
- [Client/server build](teramic158-build.log)

## Native Client Checks

Linux/X11, Vulkan, RX 6950 XT (RADV), 1280x800, High, render distance 6.
These short functional tests use separate temporary worlds and do not modify
the user's saved worlds. No new 10/15/30-minute soak test was performed.

`--smoke-test-rgb-logistics` passed. It loads saved RGB objects, teleports to
the fixture, checks an exact creative pick, operates the real TiltUI ghost
slot and Allowed/Blocked handler, checks that selecting a filter leaves the
inventory unchanged, submits an authoritative RGB crafting request, verifies
consumption/output and opens the concrete coloring recipe. Four nonblank
screenshots were captured and inspected. No logged warnings, shader errors,
renderer errors or panics in this run.

- [RGB world, filter and merger](rgb/rgb-world-filter-merger.png)
- [Allowed](rgb/filtered-funnel-allowed.png)
- [Blocked](rgb/filtered-funnel-blocked.png)
- [RGB coloring recipe](rgb/rgb-coloring-recipe.png)
- [Native RGB log](teramic158-rgb-native.log)

`--smoke-test-mixer` also passed: real color picker and authoritative recipe,
crank RPM, pause when released, exact RGB result, shared icon, ingredient
ghosts and result collection. Three screenshots were captured; the RGB output
recipe was inspected. No logged warnings, renderer errors or panics.

- [Mixer world](mixer/mixer-world.png)
- [Color picker](mixer/mixer-color-picker.png)
- [RGB output recipe](mixer/mixer-rgb-output.png)
- [Native mixer log](teramic158-mixer-native.log)

The new native fixture drives real widget handlers programmatically, not OS
mouse input. Automated logistics tests, rather than this static screenshot
gallery, verify physical multi-input merging and filtering under load.
No FPS gain, remote multiplayer validation or visual reference parity is
claimed by these functional checks.
