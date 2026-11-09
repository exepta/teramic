# Train steam, cab geometry and saved-world building — 2026-11-09

Phase 221 addresses the reported conditional-system warnings, engine autosave,
wide cab/chimney geometry, exhaust, discrete gears and building workload.
Client/server protocol is **123**; train save index is **ID17**, with an explicit
ID16 migration and retained older migrations. Rebuild both executables together.

## Behavior

- Fresh conditional Bevy systems receive an initial change tick that preserves
  first-run `Added`/`Changed` detection without the artificial maximum-age
  sentinel. Existing system ticks and warning filters are untouched.
- Stationary fuel engines (material 218) retain coal and residual burn time
  through dismantling, saving and loading. The save validator now accepts their
  furnace state, as it already does for ordinary furnaces.
- The wide building guide raises the driver's seat above the boiler, provides
  a full floor and steps, outward-facing thin walls and a taller roof. Thin
  wall placement uses the clicked edge to choose the inside/outside face.
  Existing saved body layouts are preserved; use Montage and the updated guide
  to rebuild an existing cab.
- Boiler/chimney/dome geometry uses black iron, brass details, a red guide frame,
  and a bounded visible saddle below each chimney/dome to close attachment gaps.
  The renderer explicitly permits that saddle without changing solid collision.
- Moving, fueled locomotives emit world-space chimney exhaust. Nearby vehicles,
  emission frequency, mesh updates and puff count are bounded.
- Gears select 4/10/16/20/30/50 m/s before the existing 50% fully supported
  ballast bonus. Gears 4/5 require a continuous boiler of at least 4/6 modules.
  Gear 6 additionally requires an engine, valve, gauge, water tank, water and
  at least 8 bar. Real burning fuel heats the boiler; water is consumed.
  Reverse is restricted to two gears relative to the driver's seat direction.
- Tank filling is authoritative and revision-checked: one real water bucket
  adds 1000 units and returns an empty bucket; creative fills the 8000-unit tank.
  Steam state persists and travels in compact motion updates. Dismantling
  returns whole water buckets; fractional boiler water and pressure vent.
- Rail surface profiles are immutable shared results keyed by actual shape,
  wood and model/material identity. Concurrent requests share a single cold
  build. The cache is limited to 128 entries and 32 MiB of accounted vertex/index
  payload; that budget is not a measurement of total allocator/process memory.

This is the first working water/pressure path for F53. Pipe filling, kinetic
coupling and remaining derailment rules are still open in TODO.md.

## Verification

The final full workspace library/binary suite passed **4488 tests, zero failures,
10 ignored**, across 12 suites. Tests cover delayed first-run detection,
fueled-engine persistence, legacy save migration, bounded model attachments,
shared profile isolation, raised cab clearance, complete automatic assembly,
steam resources, owner/revision checks, bucket transactions and gear direction.
A focused authority rerun also covers topping up a partially full creative tank.

Native workshop/model checks use real rendering and DOM geometry with an explicit
synthetic vehicle. Native cargo checks use actual pointer picking and controlled
acknowledgements. They are distinct from authoritative driving tests.
The user's `Test` save is loaded only from fresh copies for native integrated
server building measurements; the original files are hash-checked unchanged.

## Measurement scope

Performance numbers below describe these runs on this machine. They are not a
universal bound on intermittent streaming stalls and are not a valid matched
before/after comparison. Early baseline attempts did not exercise the intended
track area successfully and are excluded from comparisons.


## Diagnosis and isolation

The first successful rail churn run in the saved track area reproduced a
201.86 ms frame; an exact-body trace reproduced 219.47 ms. Parallel bounds and
material-specialization systems had simultaneous approximately 194 ms wall
spans. The integrated server and client shared Bevy's ComputeTaskPool.
Synchronous integrated-server schedules now execute on their existing dedicated
OS thread. Explicit generation, collider-cooking and background-save jobs keep
their async execution. Dedicated server scheduling is unchanged.

With isolation alone, the same rail case had a 34.01 ms worst frame. Server tick
work still averaged 61.86 ms and edit acknowledgements averaged 203.51 ms.
Stack samples identified repeated full-volume liquid-space subtraction in the
rail connection search. Connection checks now subtract only the two touching
face masks. The final suite compares this result against full-volume subtraction
for 15,360 randomized fluid/solid/direction combinations and 18 actual rail-face
cases. Liquid containment and rendered volume continue to use their authored
solid boxes. Exhaust emission also uses the displayed, interpolated train pose.

The raw diagnostic runs were used to identify remaining work rather than
presented as successful final performance. One deep instrumentation attempt
ended on an authoritative Occluded rejection and is excluded from statistics.
Native debugger captures perturb execution and are not FPS measurements.

The UI/model fixtures use synthetic vehicles; the saved-world cases use the real
integrated authority, normal placement/mining messages, real replication and
three independent copies of the original Test save (37 saved rail modules).
Profile identifiers in copied logs are redacted; user configuration and world
files are not included in these artifacts.


## Final saved-world workloads

All three runs completed at **2560×1440**, full native TiltUI, render distance
20, uncapped presentation, AMD RX 6950 XT/Vulkan, and a 25-second sample window.
Each run starts from a fresh copy of Test. No compiler ran during sampling.

| Case | Frames | Mean FPS | P95 / P99 frame (ms) | Worst frame (ms) | Acknowledged edits |
| --- | ---: | ---: | ---: | ---: | ---: |
| Block placement/removal | 2108 | 84.29 | 15.98 / 17.64 | 27.23 | 218 |
| Track placement/removal | 1756 | 70.23 | 19.67 / 21.66 | 28.62 | 214 |
| Real-input chunk streaming | 2124 | 84.94 | 15.81 / 18.79 | 25.16 | — |

Final rail acknowledgements averaged **48.28 ms** (P95 64.02 ms, maximum
154.12 ms); integrated server ticks averaged **3.89 ms** in that sample.
The block case averaged 35.33 ms acknowledgement latency. Both cases restored
the edited source cell to air. Rail module count returned to the original 37.
Normal grass-to-dirt conversion occurred beneath repeatedly placed stone in its
copy. The last rail acknowledgement preceded its final render upload; the
remaining sample continued idle. Acknowledgement latency is not render latency.

The streaming case climbed about 69 m through the normal controller and moved
about 237.32 m horizontally at a requested 20 m/s. The 1000 m route in the log is
its planned full leg, not the distance completed in this 25-second sample.
Loaded sections grew from 12,616 in the stationary area to 15,868. No reported
acknowledgement backlog, disconnection, warning or error occurred in the final
three cases. Every original Test file still matches its starting SHA256 digest
(8 files checked). See [metrics](workload-metrics.json) and
[original-save check](source-world-check.json).

These short samples reproduced and then removed the large frame stalls under
this workload. They do not prove a universal performance limit for every save,
hardware configuration, session length or future streaming path. FPS values are
CPU wall-frame observations, not individual GPU pass timings. Earlier baseline
attempts and debugger runs are not included in the final workload statistics.

## Reproduction and captures

Build and checks:

```sh
cargo test --locked --workspace --lib --bins --jobs 3 -- --test-threads=4
cargo clippy --locked --workspace --all-targets --jobs 3 -- -D warnings
cargo build --locked --workspace --jobs 3
cargo fmt --all -- --check
```

The three `native-*-run.json` files record the exact native commands and opt-in
environment overrides. They refer to local copied worlds/configuration in
`target/live221`; private configuration and world data are deliberately absent
from this report. Force X11 scale factor 1 for the requested physical resolution.
For an exact PostUpdate body investigation, combine `TERRA_CPU_SCOPES=1`,
`TERRA_SCOPE_TIMELINE=1` and `TERRA_SYSTEM_SCOPES=1`; the body probe preserves
existing graph nodes, dependencies, conditions, resource access and deferred
commands and has its own regression test. It is disabled during ordinary play.

| View | Native capture |
| --- | --- |
| Updated wide locomotive and chimney exhaust | [Model](locomotive-exhaust.png) |
| Workshop, 1080p / 720p | [1080p](train-workshop-1080.png), [720p](train-workshop-720.png) |
| Driving controls, 1080p / 720p | [1080p](train-driving-1080.png), [720p](train-driving-720.png) |
| Fuel / storage clicks | [Engine](engine-filled.png), [Storage](storage-filled.png) |
| Actual Test track area / streaming | [Track](test-world-rail.png), [Streaming](test-world-streaming.png) |

The changed 75 JSON/glTF documents parse. The CPU asset lint validates all
85 train block previews and 417 recipes. The Core rail source provenance remains
`71666c4c7c1195666f68054d39692fcbc7a907ea19b04718fe2a078d596afd9c`.
