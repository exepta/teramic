# Phase 77: Karten, Nullgeometrie Und Frame-Spitzen

Keine Dependency-Upgrades, Seed-/Generator-/Block-ID-, Protokoll- oder
Speicherformataenderung. Client/UI bleiben vom Headless Server getrennt.

## Ursache Und Korrektur

Der gemeldete `bevy_render::slab_allocator`-Fehler ist konkret nachvollzogen:
Bevy 0.19.1 ueberspringt in `bevy_render/src/mesh/allocator.rs` die GPU-Allokation eines
Null-Vertex-Meshes, fuehrt danach aber weiterhin die Vertex-/Indexkopien aus.
Die zwei Kopien verursachen genau das Fehlerpaar mit unallocated key.

Zwei eigene Erzeuger lieferten solche Meshes: der neue Fernterrain-Proxy,
wenn alle benoetigten Spalten schon echt geladen waren, und der Unterwasser-
Regenaufprall bei noch fehlenden/offenen Wasseroberflaechen. Wetter laedt bei
null Quads jetzt nichts hoch. Ein bestehender gueltiger Mesh bleibt verborgen
und wird mit derselben ID reaktiviert. Ein ECS-Test prueft explizit leer ->
befuellt -> leer -> befuellt, Indizes/Geometrie, Assetanzahl und Uploadzaehler.
Weitere eigene Splash-/Spray-/Spuren-/Kontakt-/Abbaupfade besitzen bereits
Nullgeometrie-Guards. Kein Loggerfilter oder Bevy-Dependency-Patch.

## Karten

Hoehlen sind immer 2D: kein schraeges Relief und keine Miniaturkamera, auch
bei Auto-Wechsel oder programmatisch angefordertem 3D. Ein zentraler Guard
steuert Kamera, Gameplay-Layer, Himmelslicht und Schattenanpassung zusammen.
Der 3D-Schalter wird deaktiviert und seine Anforderung beim Cave-Wechsel
zurueckgesetzt. Die Oberflaechen-3D-Ansicht bleibt mit Originalmodellen erhalten.

Der rot markierte Vorhang stammt aus groben erkundeten Hoehen-/Farbkacheln,
nicht aus dem Originalterrain. Eine kartierte Oberflaeche kann eine Baumkrone
sein; diese Hoehen zu einer groben Terrainwand zu verbinden ist kein valides
Terrain-LOD. Der gesamte unzuverlaessige Ersatzbatch und Hoehlen-Cutaway sind
entfernt, einschliesslich ihrer Worker-/Upload-/Sichtbarkeitsarbeit.
3D ist damit auf echte geladene Geometrie begrenzt. Entfernte Erkundung bleibt
in 2D/2.5D; vollstaendige entfernte Originalmodelle sind weiterhin offen und
werden nicht als geloestes Problem behauptet. Keine Erhoehung des Streamings.

Links stehen sieben gleich grosse Symbolbuttons (64px, kompakt 44px) mit echten
TiltUI-Hovertooltips. Eine kleine Hoehe kann die Werkzeugleiste scrollen lassen.
Die Ebenenauswahl beider Karten teilt genau einen Cache: normale Zell-/Streaming-
und Editaenderungen werden bis maximal 200 ms zusammengefasst; grosse Spruenge,
Welt-/Moduswechsel und Teleports invalidieren sofort. Erzwungene Cave-/Surface-
Modi benoetigen keinen Dachscan, die Cave-Schnitthoehe folgt dem aktuellen Y.
Bei geoeffneter FullMap verwendet auch ihr Raster ausschliesslich die zentral
aufgeloeste Ebene; ein zweiter abweichender Auto-Probe kann die 3D-Sperre nicht
mehr fuer einen Frame umgehen. Die Probe benutzt die bestaetigte Spielerposition,
nicht die eventuell noch interpolierte Position vor einem Teleport.

## Wetterarbeit

Vorher entstand auf jedem Niederschlagsframe eine Missing-Liste samt Sortierung
ueber bis etwa 2500 Roofzellen. Revisionsaenderungen berechneten ausserdem fuer
jede 1-m-Zelle dieselbe Section-Signatur erneut. Jetzt bleibt die nahe-zuerst
geordnete Arbeitsqueue bestehen; nur echte Invalidierungen planen sie neu.
Signaturen werden je horizontaler Section wiederverwendet: beim getesteten
Ultra-Feld 16 statt 2500 Berechnungen mit jeweils sechs Section-Lookups.
Vertikale Sectionwechsel invalidieren die Hoehenabhaengigkeit ebenfalls.
Beide Signaturen enthalten jetzt auch `EditJournal::water_mesh_epoch`:
Fluessigkeits-Aenderungen aktualisieren die Aufprallflaechen, obwohl sie die
Terrainrevision absichtlich nicht veraendern. Eine Regression prueft Fuellstand
7 -> 3 -> 0 bei unveraenderter Terrainrevision, auch in negativen Sections.
24 echte Dachabfragen je Frame und 125-ms-Uploadtakt bleiben unveraendert.
Kein Verlust der raeumlichen Schneestreuung, der Regenaufpralle oder der Qualitaet.

## Validierung Und Messung

### Native Karten

Der 168.721-s-Lauf `--singleplayer --smoke-test-minimap` besteht alle 52 Stufen:
echter replizierter zweiter Spieler, Mobmarker, Hoehlenteleport, gesperrtes 3D,
M/Escape, Pan/Zoom/Zentrieren, Wegpunkt-CRUD, Iconauswahl, TOML-Reload,
UI-Skalierung und echter gehaltener Boost-Flug mit Hoehen-/Richtungsaenderung.
Der Test misst sieben gleich grosse quadratische Toolbarbuttons und verlangt
einen tatsaechlich gerenderten Tooltip nach Picking-Hover, nicht bloss ein
vorhandenes Template. Der fruehere fehlschlagende Testdriver schrieb nur den
separaten CursorMoved-Kanal; Bevys Picking benoetigt WindowEvent::CursorMoved.
Diese Testkorrektur bleibt in `logs/maps-hover-driver.log` nachvollziehbar.

Nativ angesehen: [Tooltip und flache Hoehle](captures/map-6.png),
[3D auch nach direkter Anforderung in Cave gesperrt](captures/map-20.png),
[Originalminiatur ohne groben Ersatzvorhang](captures/map-38.png).
Peak-RSS 1252872 KiB, DRM-VRAM 3108072 KiB; keine Vergleichsmessung mit gleicher
alter Geometrie. Kein Slab-/unallocated-key-Fehler im Kartenlauf.
Der synthetische Pan-/Resize-Test protokolliert drei separate Wayland-Meldungen,
dass die Cursorposition nur bei gesperrtem Cursor gesetzt werden kann; die
Kartenassertionen bestehen trotzdem. Dies ist kein behaupteter fehlerfreier
Terminal-/Betriebssystemlauf.

### Streaming-Messbedingungen

AMD RX 6950 XT, RADV/Mesa 26.2.4, Vulkan, Ryzen 5900X. Bestehender Dev-Build
(opt-level 1), echte native 1920x1080-Ausgabe, VSync aus, View 8 / vertikal 3,
Seed 42042, Rivers an, Tageszeit 10:00 pausiert, klares Wetter, keine Chunk-
Animation, Erz-/Geologiepolitik der Fixture mit ores.revision = 1, gleiche
224-m-Hin-/Rueckflugstrecke. Je 60 s nach Szenenbereitschaft,
Screenshot-nahe Frames ausgeschlossen; Render-/Draw-Diagnostik und DRM-Sampling
in beiden Builds an. Der alte Build wurde vor Aenderungen als
`target/phase77/terra-client-before` aufbewahrt.

```bash
env TERRA_CONFIG_DIR=/tmp/terra73-native-configs \
  TERRA_MENU_CONFIG_DIR=target/phase77/before-state \
  TERRA_BENCH_SECONDS=60 TERRA_MEASURE_GPU_MEMORY=1 \
  TERRA_RENDER_DIAGNOSTICS=1 TERRA_DRAW_DIAGNOSTICS=1 \
  python3 tools/benchmarks/measure.py target/phase77/terra-client-before --smoke-test-lod
```

Nachher identisch mit `target/debug/terra-client`. Beide ersten Laeufe enden
mit 2035 geladenen / 869 gerenderten Sections, 3531 Meshassets, 24583 geladenen
und repraesentierten Props in 320 Batches sowie 1638 sichtbaren Meshentities.
69 indirekte PBR-API-Aufrufe und ein UI-API-Aufruf sind keine vollstaendige
GPU-Subdrawzahl; Total-GPU-Framezeit ist nicht gemessen.

| Erster Durchgang | Vorher | Nachher |
| --- | ---: | ---: |
| Frames / 60 s | 8316 | 8974 |
| Durchschnittliche FPS | 138.59 | 149.55 |
| p95 Framezeit | 10.428 ms | 9.744 ms |
| p99 Framezeit | 12.395 ms | 11.648 ms |
| Schlimmster Sampleframe | 29.412 ms | 26.294 ms |
| Initialladen | 2.630 s | 2.657 s |
| Peak-RSS | 887700 KiB | 848888 KiB |
| DRM-VRAM-Peak | 3309664 KiB | 3322732 KiB |
| DRM-GTT-Peak | 69780 KiB | 69780 KiB |
| CPU Terrain-Komposition, Worker-Summe | 13.526 s | 13.233 s |
| CPU Prop-Komposition, Worker-Summe | 1.377 s | 1.273 s |

Rohdaten: `logs/before-flight.log`, `logs/after-flight.log`. Die beiden Laeufe
liegen 17 Minuten auseinander; im Nachherlauf lief zusaetzlich ein kurzer
Headless-Smoke (0.14 s) samt Dependencyabfrage. Deshalb folgt ein direktes
Wiederholungspaar ohne weitere eigene Cargo-/Nativprozesse. VRAM und Initialladen
sind schon im ersten Paar leicht hoeher, einzelne Frame-Spitzen bleiben.
Klare Offline-Streamingfluege messen nicht die Wirkung des Niederschlagscaches
oder eine volle integrierte Mehrspielerlast. Keine allgemeine Beschleunigung
oder vollstaendige Behebung aller Nutzerspitzen aus diesem Kurzpaar ableiten.

### Direktes Wiederholungspaar

Vorher und finaler Nachherbuild unmittelbar nacheinander, keine parallelen
eigenen Cargo-/Nativprozesse. Der bereits vom Nutzer in einem RustRover-
Terminal gestartete Dedicated Server sowie Desktopprogramme liefen waehrend
beider Paare unveraendert weiter; kein exklusiv isolierter Benchmarkhost.
Diese fremden Prozesse wurden nicht beendet.
Dieselben Endzaehler fuer Geometrie, Props und Draw-API
wie oben. Alle vier Rohlaeufe bleiben erhalten, keine Auswahl nur guter Werte.

| Direkte Wiederholung | Vorher | Nachher |
| --- | ---: | ---: |
| Frames / 60 s | 8968 | 8926 |
| Durchschnittliche FPS | 149.47 | 148.77 |
| p95 Framezeit | 9.721 ms | 9.679 ms |
| p99 Framezeit | 11.332 ms | 11.406 ms |
| Schlimmster Sampleframe | 26.967 ms | 26.914 ms |
| Initialladen | 2.702 s | 2.703 s |
| Peak-RSS | 873868 KiB | 872888 KiB |
| DRM-VRAM-Peak | 3060592 KiB | 3060496 KiB |
| DRM-GTT-Peak | 69780 KiB | 69780 KiB |
| CPU Terrain-Komposition, Worker-Summe | 13.340 s | 13.236 s |
| CPU Prop-Komposition, Worker-Summe | 1.313 s | 1.290 s |
| GPU Opaque-Pass, Mittel | 1.251 ms | 1.244 ms |
| GPU Early-Prepass, Mittel | 0.439 ms | 0.436 ms |

Rohdaten: `logs/before-flight-repeat.log`, `logs/after-flight-repeat.log`.
Der unveraenderte alte Build steigt bereits von 138.59 auf 149.47 FPS;
die erste scheinbare Verbesserung ist daher kein belastbarer Codegewinn.
Im direkten Paar sind die FPS um 0.47% niedriger und p99 um 0.074 ms hoeher,
bei praktisch gleichen Speicherpeaks. Daraus weder allgemeine Beschleunigung
noch allgemeine Speicherparitaet ableiten. Einzelne etwa 27-ms-Frames bleiben.
Die offensichtliche redundante Wetter-/Kartenarbeit ist entfernt und durch
Queryzaehlertests begrenzt, aber ihr Effekt unter aktivem Niederschlag und in
langen integrierten Mehrspielerfluegen ist mit diesem klaren Offline-Paar
nicht quantifiziert. Die generellen Nutzerspitzen sind damit nicht als
vollstaendig behoben abgenommen; siehe TODO.md.

### Native Wetter-Regression

`--smoke-test-weather` besteht vor und nach der finalen Fluessigkeits-
Invalidierung alle Assertions: Rain, Low/Off, fuenf Schneestufen, fuenf
Wolkendecken, Tag/Nacht-Verdeckung, Gewitter, Sturm-/Ruhewellen, Uferwash,
Unterwasser-Regenmaske und sichtbare Oberflaechenaufpralle, Schneedecke,
Entzuendung/Loeschen. Final 120.985 s, Peak-RSS 920684 KiB, DRM-VRAM 2839732 KiB.
Dies ist ein Funktionstest mit wechselnden kurzen Phasen, kein FPS-Paar.
`logs/weather.log` und `logs/weather-final.log` enthalten weder WARN/ERROR
noch Slab-/unallocated-key-Meldungen. Die finalen Fluglaeufe ebenfalls ohne
WARN/ERROR; im Kartenlauf verbleiben nur die oben eingeordneten drei
Wayland-Cursor-Testmeldungen.

Nativ angesehen: [Regenaufpralle von unter Wasser](captures/rain-impacts-seen-from-underwater.png).
Der finale Unterwasserbatch besitzt 142 Quads in genau einem Batch; keine
fallenden Partikel im Wasser und kein Null-Vertex-Upload waehrend des Warmups.
Keine Behauptung einer menschlichen Audioabnahme oder mehrstuendigen GPU-Pruefung.

### Cargo Und Headless

Alle folgenden Befehle erfolgreich, Rust-/Dependencyversionen unveraendert:

```bash
cargo fmt --all --check
env CARGO_BUILD_JOBS=1 cargo check --locked --workspace
env CARGO_BUILD_JOBS=1 cargo test --locked --workspace
env CARGO_BUILD_JOBS=1 cargo clippy --locked --workspace --all-targets -- -D warnings
env CARGO_BUILD_JOBS=1 cargo build --locked --workspace
```

1201 Tests erfolgreich, ein schon vorhandener expliziter 180-s-UDP-Soak
ignoriert. Neue Regressionen pruefen insbesondere Cave-/Command-Guards,
negative Koordinaten, gemeinsame Probe-Deadlines mit gleichzeitigem Streaming,
Quadratbuttons/echte Templateparser, konstante Wetterquerybudgets, gruppierte
Sectionrevisionen, reine Fluessigkeitsaenderungen und Nullgeometrie-Lebensdauer.
Rohprotokolle: `logs/fmt.log`, `logs/check.log`, `logs/tests.log`,
`logs/clippy.log`, `logs/build.log`.

Der Dedicated-Server-Smoke startet ohne DISPLAY/WAYLAND_DISPLAY und verarbeitet
27 Sections. `cargo tree -p terra-server --edges normal` enthaelt weiterhin
keine `bevy_render`/`bevy_pbr`/`wgpu`/`winit`/`terra-ui`/`terra-render`/TiltUI-
Abhaengigkeit. Siehe `logs/headless.log`, `logs/server-dependencies.log`.
