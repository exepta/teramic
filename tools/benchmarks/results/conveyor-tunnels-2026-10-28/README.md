# Conveyor Casings, Funnels And Tunnel Direction

## Changes

- Port removal no longer treats an attached inventory as a second furniture
  half. Survival mining/tool requirements still apply, and removal returns one
  port without changing its source inventory or machine progress.
- A newly placed side chest inherits the existing single chest's orientation.
  Both inventories and wood types survive joining/splitting, including when a
  logistics port is mounted. Triple and front/back connections remain invalid.
- Moving ribbons default to gray. Right-click with a dye to color the complete
  owned band using one pigment. Wood selects the grounded frame, not the ribbon.
  Grounded horizontal cells have the same supported frame in rendering and
  collision; suspended and sloped spans stay bare. Two shared frame meshes and
  four wood materials avoid one mesh/asset per beam or bolt.
- Carried drops run upright on the centerline of the continuous upper surface.
  Their reported velocity drives interpolation, while ordinary fall physics and
  proximity pickup are skipped. Right-click a visible carried stack to collect
  it; the server checks the actual drop ID, ray, reach, loading and inventory.
  Removing/leaving the band restores ordinary drop physics and pickup.
  Snapshot interval measurement now continues after interpolation completes,
  preventing a repeated move/pause cadence. Transport retains the full rated
  speed through 256 RPM, with at most 26 collision steps of at most 5 cm per
  update instead of capping item speed at 2 m/s.
- ID 152 (`teramic::item_hopper`) now renders as the wooden Hopper/Tunnel;
  ID 153 (`teramic::item_importer`) as the open Funnel. Their lower profiles
  reach the ribbon without a floating gap and leave the actual item run clear
  in both horizontal axes. Authored meshes, collision, selection and negative-Y
  section halos share the same bounds. Visible arrows show transfer direction.
- Both ports attach to chests, furnaces or crushers. The Wrench reverses their
  arrow and input/output mode without moving the inventory connection. New
  chest attachments default to intake, new machines to finished-output export.
  Legacy low-facing modes retain their old behavior. Chest partners can use
  different woods. Machine imports accept only valid input/fuel slots; machine
  exports never consume inputs, fuel or upgrades.
- Crusher, crank and IDs 141-153 neither accumulate natural snow nor receive
  shader snow caps. This does not delete player-placed snow blocks.

## Use

1. Connect two parallel shafts with the Conveyor item. Put solid ground one
   cell below horizontal spans for wooden framing; remove it for bare spans.
2. Mount a Tunnel or Funnel on the inventory, directly above the carrying band.
3. An arrow toward the inventory imports; an arrow away exports. Right-click
   with a Wrench to reverse it. Extraction requires a running band below.
4. Right-click the ribbon with dye to color it. Right-click a carried item to
   collect it; walking beside the band no longer takes transported goods.

## Compatibility And Limits

Existing material IDs and generator/save identities remain unchanged. Belt
color uses the existing facing metadata (low two bits are orientation); port
direction uses the existing mode bit. No second item registry, inventory or
transport engine was added. Client/server protocol is 82.

Existing limits remain: 32 intermediate belt cells, 128 transported stacks per
tick in round-robin order, 64 active ports per half-second cycle and four items
per export. Larger active populations share the budget. Full, blocked or
unloaded destinations preserve unaccepted goods. No FPS improvement or long
native soak is claimed for this functional change.

## Validation

| Command / Scenario | Actual Result |
|---|---|
| `cargo fmt --all --check` | Passed |
| `cargo check --locked --workspace --all-targets -j1` | Passed |
| `cargo test --locked --workspace --no-fail-fast -j1 -- --test-threads=2` | 2,963 passed, 0 failed, 7 existing ignored tests |
| `cargo clippy --locked --workspace --all-targets -j1 -- -D warnings` | Passed |
| `cargo build --locked --workspace -j1` | Passed, client and dedicated server |
| `--smoke-test-kinetics` | Passed on RX 6950 XT, Vulkan/RADV, Mesa 26.2.5 |
| `--smoke-test-materials`, attempt 1 | Partial: all 153 block previews published after GPU pixel checks, seven gallery views captured; fixed 120-second limit reached at stage 8 with no material/render error |
| `--smoke-test-materials`, attempt 2 | Partial: all 153 block previews published, six gallery views captured; same unmodified limit reached at stage 7 with no material/render error |

Regression coverage includes real survival mining/tool timing without deleting
the attached inventory, merging/splitting mixed-wood chests, preserving contents
and port attachments, rejecting foreign gear and stale wall collisions, input
and output direction, bounded machine-slot imports, exact stack conservation,
right-click collection authority/occlusion/replay, dye consumption/replay,
negative section seams, sloped contact, both signs of high RPM, unsupported
legacy belt collisions and natural/shader snow exclusion.

The native kinetic fixture verifies actual rendered drop meshes rather than
only journal positions. It checks gray/blue/red ribbons, five supported casing
cells, two transported stacks, forward/reverse/stopped motion, agreement of
authoritative and rendered positions and a stable cache of 33 meshes while
stopped. Four scene captures and six gear-icon captures are archived. The
close-up and the complete-block gallery were visually inspected; this is not
an exact 1:1 art acceptance or a long-duration FPS benchmark. The broad material
gallery did **not** pass completely; its two timeout logs remain archived.

![Native conveyor, Tunnel and Funnel close-up](captures/kinetics-stretched-belts-items-woods.png)

### Evidence

- [Workspace tests](logs/teramic153-tests.log)
- [All-targets check](logs/teramic153-check.log)
- [Strict Clippy](logs/teramic153-clippy.log)
- [Workspace build](logs/teramic153-build.log)
- [Native kinetic fixture](logs/teramic153-kinetics.log)
- [First material timeout](logs/materials-first-attempt-timeout.log)
- [Repeated material timeout](logs/materials-second-attempt-timeout.log)

The native commands used separate `/tmp/teramic153-*-config` copies and X11
with `WAYLAND_DISPLAY`/`WAYLAND_SOCKET` unset. No player save was modified.
No 10-, 15- or 30-minute soak, multiplayer human acceptance or new FPS gain is
claimed.
