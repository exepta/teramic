# Junction Order, Real Belt Endpoints And Bindless Depth

Phase 163 fixes the three reported production failures without changing block
IDs, dependencies, the region format or the network schema.

## Confirmed Causes

- Merger admission reused a generic custody-recovery candidate for different
  inputs. An inline item could therefore win a turn intended for a side input.
- One exclusive route held the entire junction until the item had passed its
  outlet. Requiring the complete path to be empty compounded the artificial
  spacing between consecutive items.
- The old outlet probe was 0.78 m from the router center. A real neighboring
  linked endpoint does not provide the required handoff there; legacy isolated
  belt cells in the earlier tests concealed that mismatch.
- Distributor filters had no reserved priority for explicit Allowed matches.
  Left and Right also disagreed with the player's orientation convention.
- The custom prepass declared a binding array without enabling its WGSL
  extension itself. Depending on the prepass variant, removing unused stock
  imports also removed the only transitive extension directive. A failed depth
  pipeline could then leave terrain missing beneath water.

## Routing Changes

Input direction and already-admitted custody recovery are distinct. Real
waiting input heads receive round-robin turns; one input cannot claim another
input's turn. A head temporarily blocked by occupancy keeps its turn. An
approaching head reserves its input's next cycle until it reaches the hood
entry, preventing a closer inline input from filling several slots in advance.
Early approaches can queue and move continuously through the
hood, with substeps and existing spatial occupancy checks, instead of reserving
the complete downstream meter. Queues remain bounded to 16 routes per port and
64 actual route movements per update, with the existing global drop limit.

The handoff uses the actual selected target segment's drop center and span
identity. Item movement never jumps to that center. The transition gap remains
carried custody, exempt from proximity pickup and gravity; right-click pickup
remains supported.

Distributor outputs are Left, Forward, Right relative to the input direction.
An explicit matching Allowed filter reserves its output. Without such a match,
compatible general outputs rotate fairly. Thus three empty outputs rotate
Left -> Forward -> Right; with Red reserved on Left, other items rotate through
Forward and Right. A stopped or blocked reserved output does not leak its item
into an unrestricted fallback.

Restart recovery reconstructs both halves of a diagonal turn, restores exiting
heads front-to-back, and preserves already-selected output identity. A parked
head regains paused custody even when its output is blocked, stopped or
unloaded; it must not monopolize recovery selection for independent outputs.
Actual movement still requires live geometry, loaded sections, power, free
space and terrain clearance. Removed or changed geometry invalidates routes.

## Shader Changes

Both custom wet/color and imprint/prepass modules explicitly enable
`wgpu_binding_array` when Bindless is active. The directives follow imports as
required by WESL; the composed WGSL places enables before declarations.
The non-Bindless path remains available. No renderer dependency is patched.

The new composition regression checks twelve feature combinations using real
embedded Bevy PBR modules. Only the runtime-created storage mesh binding uses a
documented adapter with real Bevy types, because a GPU-free RenderPlugin does
not register that runtime module. A negative control removes the custom enable
and reproduces the original missing-extension condition after dead-code
elimination. Native GPU checks use the actual engine bindings, not the adapter.

## Validation

Final workspace result: **3369 passed, 0 failed, 8 ignored**. The new integration
fixtures use real owned belt spans, mounted hand cranks and the server kinetic
solver, not fabricated RPM values. All six linked-junction tests passed,
including strict alternating departures and the unchanged maximum 0.5 m
output-spacing assertion at 1 m/s for all four yaws. Actual Red/Blue/Yellow
mushroom-block filters, empty and partially filtered outputs, backpressure,
pickup custody and restart cases passed too.

An earlier full run exposed one remaining alternation failure. The waiting-turn
and approach-cycle correction above fixed it; neither its alternation nor its
spacing assertion was relaxed. The final full run also passed the twelve-variant
shader composition regression and original-error negative control.

Successful workspace validation:

```sh
cargo fmt --all --check
cargo check --locked --workspace --all-targets -j1
cargo test --locked --workspace --no-fail-fast -j1
cargo clippy --locked --workspace --all-targets -j1 -- -D warnings
cargo build --locked --workspace -j1
```

Following the behavioral test run, only documented local parameter-count lint
annotations were added; no behavior or test assertions changed. Final strict
Clippy, format, check and production builds passed with those annotations.

Native checks on **AMD Radeon RX 6950 XT / Vulkan / Mesa 26.2.5-arch3.1**:

- `--smoke-test-wet-opacity`: nine verified pixel phases with normal Bindless
  bindings and nine more with `TERRA_LEGACY_WET_BINDINGS=1`. Blend, opaque depth,
  Mask(0), real alpha discard, normals/motion and normal-map UV0/UV1 all passed.
  Samples for opaque and Mask(0) surfaces were `[255,255,255]`; real discard was
  `[0,0,0]`. Neither native log contains shader errors or panics.
- `terra-server --smoke-test`: successful headless startup, 27 sections and a
  successful exit.

Selected GPU captures and raw results:

- [Opaque depth, Bindless](wet-bindless/wet-opacity-3.png)
- [Mask(0), Bindless](wet-bindless/wet-opacity-4.png)
- [Normals/motion with UV1, Bindless](wet-bindless/wet-opacity-8.png)
- [Normals/motion with UV1, legacy bindings](wet-legacy/wet-opacity-8.png)
- [Final full workspace tests](teramic163-tests-final.log)
- [Native Bindless log](teramic163-native-bindless.log)
- [Native legacy-bindings log](teramic163-native-legacy.log)

The earlier phase's legacy-cell and filter-UI checks did not establish these
owned-endpoint and GPU depth-variant contracts. No new FPS or long-run stability
claim is made here, and no 10/15/30-minute native soak is introduced.
