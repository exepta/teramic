# UUID Profiles, LAN And Game Menus - 15.10.2026

## Connections And Identity

The client persists a UUIDv4 and display name in `configs/profile.toml`, using
the existing validated Config trait and atomic TOML writer. `TERRA_PROFILE_FILE`
can select a separate identity for a second player on the same machine. Missing
files are created; invalid files remain untouched and block session startup.
UUIDs survive server/client restarts. Existing numeric PlayerIds remain internal
session identifiers, not account IDs. World IDs, seeds, block IDs, generator
version 5 and saved section journals are unchanged. Protocol version is now 3;
rebuild/restart both endpoints together.

Clients join by address, either through the browser or `--connect IP:PORT`.
`--connect-token` / `--token-out` and the token-file form field are removed.
Legacy `servers.toml` entries are read compatibly; obsolete paths are omitted
on save, preserving saved server IDs and addresses. No user-configured key or
credential file is required. The proven Lightyear Netcode UDP encryption stays:
it necessarily retains an ephemeral internal server transport key. A bounded
TCP bootstrap automatically issues 120-second transport credentials, bound to
the requested endpoint and profile. They are not account credentials and are
never persisted to the server list or logged. The server binds Hello's UUID/name
to that bootstrap and rejects duplicate UUIDs and substituted/expired claims.

**This is local-profile mode, not verified online login.** No account/session
service exists in the repository. UUID ownership cannot be proven from a UUID
alone. Joins with unverified profiles are accepted only from loopback/private/
link-local peer addresses. LAN bootstrap is a trusted-network protocol, not an
Internet authentication or MITM-resistant login system; reverse proxies/port
forwarding can alter the peer trust boundary. `require_verified_accounts=true`
fails closed at startup without an account verifier. Integration points are
`gateway::respond(Request::Join)` and `Gateway::validate_claim`: future account
proof must be checked against the account service before issuing a transport
credential, never accepted merely because a caller supplies a UUID.

## Real Server Browser

Dedicated configuration adds title, description, `lan_discovery` and
`require_verified_accounts`. TCP status/bootstrap uses the same numeric port as
UDP gameplay. LAN probing uses IPv4 multicast 239.255.84.77:54545 plus loopback;
the listener joins both the default interface and loopback group. Reused server
discovery sockets support multiple local servers, while every browser uses its
own ephemeral socket. The private integrated singleplayer server is not advertised.
Replies use the actual bound server interface, including non-default loopback
addresses, rather than accidentally advertising an unreachable loopback alias.

Probes run every two seconds while the browser is open; responses must match the
nonce, magic, bounds and source class. Entries expire after seven seconds. Server
instance UUIDs deduplicate interface aliases and saved/discovered entries; local
loopback is preferred where available. Discovery does not create fake saved
entries. Selecting a LAN row allows direct Join or Add Server with its address
prefilled. Saved entries get real TCP status queries outside the render thread,
at most four concurrent queries and a five-second refresh interval. Actual
accepted players/max players, title, description and measured request/response
time are displayed. Incompatible versions are marked; unsuccessful queries show
Offline and their real error. Ping is application round-trip time, not ICMP.

The server worker is nonblocking, lifetime-managed and separate from simulation:
32 pending TCP requests, two-second deadlines, 2 KiB requests, 16 KiB client
response limit, 256 pending joins, bounded per-source rate state and expiry.
Startup TCP conflicts and missing account verifiers report errors without missing-
resource panics. Wildcard UDP binds accept the actual interface endpoint signed
by this process during the TCP bootstrap instead of requiring 0.0.0.0 in tokens.

For another LAN machine use `--bind 0.0.0.0:5000`; allow TCP+UDP 5000 and UDP
54545 in the firewall. A loopback-only bind intentionally stays local. The existing
direct IPv6 transport/address path is preserved but was not natively tested here;
IPv6-only listeners are not advertised by the IPv4-only discovery service.

## TiltUI Screens

New reusable pages under `src-ui/pages/`: `game-menu` and `loading-screen`.
They use the existing component catalog, shared state, CSS, buttons and icons.
No new UI framework, dependency upgrades or hand-spawned application UI.
Two already locked transitive dependencies, uuid 1.28.0 and socket2 0.6.5, became
direct dependencies for standard UUIDs and shared multicast sockets respectively.

ESC shows the dark green/gold Game Menu over the actual running world. Resume,
Options/Back, Save World, Save & Main Menu and Quit use the real session lifecycle.
Metadata includes world/server title, mode, seed, elapsed session time, name and
local-profile UUID. Save World requests an asynchronous journal flush on the
integrated server thread and reports completion/errors; no main-thread mesh or
disk work is added. Multiplayer save is disabled, and Leave is labelled Disconnect
& Main Menu because the client does not control dedicated persistence. Simulation
continues while the game menu is open, as before; gameplay input is blocked.

The loading screen reuses the existing valley bitmap. Connection/synchronization
states come from the actual session. Once measurable, phase-labelled terrain and
mesh progress comes from loaded/wanted sections and completed/pending/queued mesh
jobs. It is not a fabricated elapsed-time percentage or a unified ETA. Transition
to Playing additionally waits for initial generation/render readiness. Cancel,
failure and asynchronous shutdown retain the existing lifecycle.

The dropdown issue had two parts: selected-option default CSS overrode the light
text color, and initially hidden/reparented popup children retained zero-sized
Bevy text layouts. Explicit hover/checked/focus colors and a narrow compatibility
system resynchronizing the existing subtree on open fix both. Labels use one
intrinsic line; no duplicate text nodes or replacement dropdown system is added.
The native fixture asserts nonzero text layout and selects Creative by actual
Bevy pointer hit-testing. The native 960x640 popup was visually inspected.

## Validation

Workspace checks passed: fmt, check, **224 tests**, Clippy with `-D warnings`,
and workspace build. Logs are archived in `logs/`.
The separate dedicated build also passed; with DISPLAY and WAYLAND_DISPLAY unset,
its smoke test generated 27 sections without a window. The separate server feature
tree contains no bevy_render, bevy_winit, wgpu or TiltUI dependency.

Added regression coverage includes persisted profile reload/corrupt file handling,
legacy server-list compatibility, actual address-only two-client UDP joins, stable
UUID after rejoin, duplicate/substituted/expired UUID rejection, actual player
counts, two simultaneous scanners discovering multiple servers, malformed/foreign
announcements, TTL expiry, bounded bootstrap input, startup errors, missing account
verifier, LAN menu selection and popup hierarchy invalidation without new nodes.

Native Vulkan client fixtures use fresh temporary config/world roots and production
TiltUI/session code, not mock menus or simulated successful connections:

- Integrated session: loading, real host, rebound movement, authoritative edit,
  ESC Game Menu, explicit save acknowledgement, Options/Back, Resume, leave,
  host restart and journal reload.
- Small-window menu suite: actual dropdown open/selection, world CRUD, saved
  server CRUD, controls/conflicts/Escape, Apply/discard and 15-second rollback.
- LAN-only join: automatically discovered unsaved row, real dedicated handshake,
  initial terrain/render readiness, no second integrated host, clean leave.
- Explicit-interface LAN join: a server bound to 127.0.0.2 was discovered under
  that address, selected and joined without saved entries or credential files.
- Saved-server join: same automatic handshake after Add/Edit, no credential file.
- Stopped-server join: real connection failure, actual visible TiltUI error and
  return to Main Menu.
- Two simultaneous native clients: the existing network observer/companion fixtures
  joined by `--connect`, using separate persisted UUID files. The observer reported
  one remote player/one rendered avatar with acknowledged input and its capture
  visibly shows the companion. The moving companion completed with actual movement;
  its later capture has no remote because the observer had already exited.

Native test hardware: AMD RX 6950 XT, RADV Mesa 26.2.4-arch3.1, Vulkan, Wayland.
Pointer tests inject Bevy PointerInput at computed widget coordinates; they are
not physical-mouse manual tests. No performance speedup, Internet account login,
cross-machine LAN validation or GPU benchmark is inferred from these fixtures.
The earlier reports remain historical records of their respective implementations.

Visually inspected native captures (1280x800 and 960x640):
[Game Menu](session/game-menu.png), [small Game Menu](session-small/game-menu.png),
[Loading Screen](session/loading-screen.png), [small Loading Screen](session-small/loading-screen.png),
[mode dropdown](session/mode-dropdown.png), [small dropdown](dropdown-fixed/mode-dropdown.png),
[LAN browser](lan-join/lan-discovered.png), [connection failure](failure/connection-failed.png).
[Two-client observer](uuid-observer.png) was also visually inspected.
Only selected captures are kept to avoid archiving redundant full-screen bitmaps.

## Remaining Limits

- Online account ownership/login requires an external account/session service.
- Cross-machine LAN, router/firewall variants, IPv6 discovery and Internet tests
  remain open. Current transport/discovery checks use real local sockets.
- Dedicated disk save management remains a separate existing backend task.
- The old low-level internal test token helper remains for protocol fault fixtures;
  production menu/CLI joins obtain fresh credentials automatically by address.
- Very early connection phases have a status label rather than an invented bar.
- No new FPS, frame-percentile or GPU performance benchmark was run for this change.
