# Character Reference Implementation

## Appearance And Rig

The previous green ranger outfit is replaced by two model variants following the
supplied male/female reference: cream sleeveless shirt, brown trousers, rolled
cuffs, drawstring belt and bare feet. The female variant has a narrower torso
and shoulders, longer side locks and a raised hair bun. Eyes contain white,
brown iris, pupil and highlights; short intermittent blinks cover the complete
eye. Hair consists of overlapping, differently tinted voxel locks. Cloth and
skin surface patches share the actual faceted mesh surface instead of floating
coplanar detail planes.

Both variants retain the existing six-part articulated rig: body, head, two arms,
two legs. Detail is baked into those meshes once. The pair has 27,320 body
vertices in the inspected native capture, 12 cached body meshes and one shared
eyelid mesh. All parts share one rough vertex-coloured PBR material. This is a
mesh/vertex count, not a measured GPU draw-call count. Each rendered avatar uses
six body entities plus its eyelids, with no entity per detail voxel.

Existing walk, head-look, mining, breaststroke, upright treading-water and blink
animation paths remain connected. A small outward rest angle relaxes the arms;
it fades out during swimming and right-arm strikes. The held-item grip moves
from -0.43 to -0.54 metres below the shoulder to match the new palm. The local
physical capsule and first-person camera are unchanged.

## Selection And Networking

Start with `cargo run --locked -p terra-client -- --gender=f` or `--gender=m`.
The split form `--gender f` also works. Invalid, missing or repeated values fail
before opening the client window. Selection is stored as `gender = "female"`
or `gender = "male"` in the existing player profile. Without the flag, the last
saved value is used. Old profiles without a gender field default to male while
retaining their UUID and name. `--profile NAME` keeps independent selection in
`configs/profiles/NAME.toml`; `TERRA_PROFILE_FILE` uses the same loading path.

The selected profile is used by main-menu joins, direct joins and integrated
sessions. The server accepts the typed enum through the existing profile
handshake and publishes it with player snapshots, including teleport/game-mode
snapshots. Remote model roots are replaced if a snapshot changes the variant.
Protocol version is **38**, so both client and dedicated server must be rebuilt.
World IDs, UUIDs, generation and region/player save formats are unchanged.
Inventory portraits use the same selected model as the world avatar. Their
one-shot cache is created after startup profile loading; there is currently no
in-session gender selector.

## Validation And Limits

The geometry tests validate both variants' indices, normals, finite coordinates,
player bounds and vertex budget. Profile tests cover CLI validation, legacy
profile loading and persisted changes without identity changes. The real UDP
two-client join test additionally checks female gender replication. Existing
walk, attack, swimming and blinking tests cover the preserved animation paths.

`--smoke-test-avatar` explicitly loads the selected profile. It shows the local
variant next to the opposite remote variant and captures idle, walking, open-eye
and closed-eye portraits. Its remote tracks are a renderer fixture, not a claim
of a real network connection. `TERRA_SCREENSHOT=/path/male.png` selects the
capture basename. The native window is BGRA while portrait targets are RGBA;
validation now uses Bevy's format-aware RGBA conversion. A regression test checks
both formats, blank images and clipping detection.

Both final native runs passed on 2026-10-18: male at 17:19:59 and female at
17:20:25. Each produced five captures, observed a blink and verified motion in
all four limbs. The logs are `terra65-avatar-m-verified.log` and
`terra65-avatar-f-verified.log` beside this report. Inspection of the updated pair and portrait
images under `avatars/` confirms complete bodies, the two clothing/hair
silhouettes, bare feet and an unclipped portrait including the female hair bun.
The earlier pixel-format check failure was corrected before these successful
runs. An invalid gender argument also exited with status 1 before renderer
startup. Workspace-wide build/test results are recorded in the top-level
report, separately from these native avatar checks.

The reference is a static illustration, not a supplied 3D asset or animation
set. This implementation follows its design but is visibly more angular and
uses the existing six-part rig. It does not reproduce hidden geometry, fingers
with separate joints, elbow/knee skinning or exact illustrated lighting. Literal
one-to-one identity and new performance parity are not claimed.

## Streaming Review And Measured Limits

The 60-second matched-old-assets renderer comparison reported these values:

| Metric | Before | After |
| --- | ---: | ---: |
| Mean FPS | 110.412 | 124.876 |
| p95 frame time | 14.444 ms | 11.566 ms |
| p99 frame time | 21.273 ms | 14.066 ms |
| Initial readiness | 5.228 s | 5.249 s |
| Peak sampled RSS | 1,249,496 KiB | 1,204,960 KiB |
| Peak sampled VRAM | 3,725,256 KiB | 3,984,416 KiB |

This run improves frame throughput and tail frame times while sampled peak RSS
decreases by 43.492 MiB. Sampled peak VRAM **increases by 253.086 MiB**; GPU-memory
parity is not established. Initial readiness is effectively unchanged, not an
improvement. These figures belong to the complete code comparison with old
assets, not a benchmark isolating character meshes. They cannot establish that
the new character geometry caused the VRAM increase, nor that the user's
occasional sub-30-FPS flight stalls are completely fixed. The top-level report
contains the conditions and additional faster streaming comparisons.

Read-only review confirmed that render-only snapshot consumption bypasses
change detection while real generated data, edits and unloads retain their
signals. Stale section epochs are filtered before the bounded top-k job
selection; otherwise stale near candidates could starve eligible farther jobs.
Late uploads still validate section membership and epochs before replacement.
The empty-section custom-model fast path is safe because those sections contain
no local blocks or support faces. Boundary block edits invalidate neighboring
mesh epochs, allowing the light cache to re-evaluate affected sources.

One pre-existing lighting limitation remains: replacing one emissive block type
with another at the same position in a single received update can leave the old
light definition on an existing light entity. This was identified during review
and is not attributed to the streaming optimization.

The opt-in CPU scope counters check `TERRA_CPU_SCOPES` once through `LazyLock`.
With profiling disabled, individual scopes create no timer, acquire no mutex,
allocate no memory and write no log. There is still a cached enable check and
the empty guard/drop path, so zero overhead is not claimed. Benchmark-end
`snapshot()` acquires the counter mutex once even when empty. Enabled scopes
sample clocks and lock a shared counter map on drop; those measurements include
instrumentation costs and nested scope sums must not be added together.
