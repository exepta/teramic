# Rendering, Schatten und GPU-Uploads (Phase 132)

## Umfang

Die drei freigegebenen Rendering-Schritte wurden implementiert. Collider,
Physikradien, Sichtweite, Vegetationsdichte und Schattenqualitaet bleiben
unveraendert. Dieser Bericht ist **keine Abnahme des gesamten FPS-Problems**.

- `vendor/bevy_render/src/slab_allocator.rs`: Alle Puffer-Verbreiterungen
  derselben Allocation-Transaktion verwenden einen gemeinsamen Command Encoder
  und hoechstens einen Queue-Submit statt eines Submits pro vergroessertem Slab.
  Die bestehenden Puffergroessen und der Wachstumsfaktor bleiben erhalten.
  Die vorhandenen Korrekturen fuer mehrfach extrahierte Mesh-IDs bleiben erhalten.
- `crates/terra-render/src/meshing/visibility.rs`: konservative Section-Portale.
  Nur vollstaendig geschlossene Grenzen aus bekannten opaken Vollbloecken sperren
  eine Verbindung. Unbekannte Nachbarn, Teilbloecke und transparente Materialien
  bleiben offen. Ladefronten und die Kameraumgebung sind Ausgangspunkte;
  angrenzende Wand-Sections bleiben sichtbar. Die Analyse laeuft asynchron,
  maximal viermal pro Sekunde, mit einer zeitbudgetierten Boundary-Vorbereitung.
  Oeffnungen/Edits/Teleport/Unloading invalidieren alte Resultate sofort.
- `meshing/jobs.rs`, `meshing/upload.rs`: sicher ausgeschlossene Sections
  erhalten keine neue Render-Komposition/GPU-Allokation. Ueberholte Upload-Jobs
  werden verworfen, bestehende ausgeblendete Render-Roots samt eigenen Meshes
  ueber den vorhandenen Lifecycle freigegeben. CPU-Weltdaten bleiben geladen;
  die Wiederfreigabe aktualisiert die Render-Auswahl ueber eine Revision.
- `meshing/shadows.rs`: opake Schatten-Proxys benoetigen keine zusaetzlichen
  rueckwaerts gewundenen Dreiecke mehr. Ein gemeinsames, beidseitiges opakes
  Schattenmaterial ersetzt diese Verdopplung (30 statt 60 Indizes pro Shell).
  Cutout-Laub, dessen UVs/Normalen und Windmaterialien bleiben separat erhalten.
  `staging.rs` konserviert den Materialtyp vor dem Entleeren der Vertexdaten.

Das Portalverfahren ist absichtlich konservativ: Es erkennt nicht jede
Verdeckung innerhalb einer Section. Verbundene Hoehlensysteme, insbesondere
an unbekannten Ladegrenzen, duerfen weiter sichtbar bleiben. Props und Wasser
verwenden weiterhin ihr bisheriges Culling. In den beiden unten gemessenen
offenen Weltszenen waren am Messende **0 Sections zusaetzlich ausgeblendet**.
Dafuer wird kein FPS-Gewinn behauptet.

## Native Messungen

RX 6950 XT / RADV Vulkan, Ryzen 9 5900X. Bestehendes optimiertes Dev-Profil,
1280x800, Sichtweite 12, unveraenderte Grafikoptionen. Keine parallel laufenden
Builds oder Unit-Tests waehrend der Messfenster. Keine Langzeittests.

| Szene | Build | FPS Mittel | p95 ms | p99 ms | Max. Frame ms | Laden s | RAM KiB |
|---|---|---:|---:|---:|---:|---:|---:|
| Regen-Flug, Seed 7, 24 s | vorher | 94.83 | 15.96 | 19.52 | 26.08 | 8.77 | 1274792 |
| Regen-Flug, Seed 7, 24 s | final | 96.28 | 15.75 | 18.65 | 32.74 | 8.90 | 1260812 |
| Dorf nachts, Seed 42042, 8 s | vorher | 73.24 | 17.63 | 19.62 | 22.27 | 23.27 | 1906084 |
| Dorf nachts, Seed 42042, 8 s | final | 69.83 | 18.43 | 21.12 | 23.37 | 22.55 | 1913752 |
| Dorf nachts, Seed 42042, 8 s | finale Wiederholung | 74.57 | 17.07 | 18.48 | 22.78 | 22.69 | 1865052 |

Rohdaten: `before-flight.log`, `final-flight.log`, `before-night.log`,
`final-night.log`, `final-night-repeat.log`. Im Flug bleiben 6574 geladene / 1336 gerenderte Sections,
7542 Mesh-Assets, 625 Wasser-Tiles und 10151 dargestellte Props erhalten.
Im Dorf bleiben 5625 geladene / 2878 gerenderte Sections, 8 schattenwerfende
Punktlichter und 80732 dargestellte Props erhalten.

Die kurzen Einzelmessungen belegen keinen allgemeinen FPS-Gewinn. Der schlechteste
Flugframe ist schlechter; die beiden Dorflaeufe liegen unter und ueber dem
Vorher-Wert. Es wird weder nur die bessere Wiederholung ausgewaehlt noch ein
zuverlaessiger FPS-Gewinn aus dieser Streuung abgeleitet.
Der integrierte Spielermotor verschiebt auch die endgueltige Kamerahoehe leicht
(38.81 vorher, 38.56 nachher); der Dorfvergleich ist deshalb kein pixelidentischer
isolierter GPU-Mikrobenchmark. Streaming-Ausreisser bleiben offen.

## Verworfenes Experiment

Die getestete Begrenzung auf 32-MiB-Mesh-Slabs wurde vollstaendig entfernt:
Im Dorf stieg die Zahl der Puffer von maximal 14 auf 73. Die Szene erreichte
nur 55.74 FPS (`after-night.log`), bei herkoemmlichen Puffergrenzen im gleichen
Zwischenbuild dagegen 69.38 FPS (`night-large-slabs.log`). Die neue Grenze
haette zwar grosse Einzelkopien begrenzt, aber das bestehende GPU-Batching
verschlechtert. `after-flight.log`, `flight-no-portals.log` und `after-night.png`
sind ebenfalls Zwischenstaende dieses verworfenen Experiments, nicht der finale
Build. `TERRA_BENCH_LARGE_SLABS` ist im finalen Code nicht mehr erforderlich.

Die erste Portalversion startete im 24-s-Flug 774 Analysen mit zusammen 5.33 s
Worker-Wallzeit. Zusammenfassen der Updates reduziert das auf 92 Analysen /
0.63 s; Main-Thread-Vorbereitung von 0.185 auf 0.029 ms pro Frame. Workerzeiten
sind parallel und nicht zur Framezeit zu addieren.

## Reproduktion

Fuer den Flug: `target/debug/terra-client --smoke-test-lod` mit
`TERRA_CONFIG_DIR=tools/benchmarks/results/tree-structures-performance-2026-10-25/configs-seed7`,
`TERRA_BENCH_GRAPHICS=1`, `TERRA_BENCH_WEATHER=rain`, `TERRA_BENCH_SECONDS=24`,
`TERRA_BENCH_LOAD_SECONDS=240`, `TERRA_CPU_SCOPES=1`, `WGPU_BACKEND=vulkan`.

Fuer das Dorf: `--smoke-test-benchmark` mit
`TERRA_CONFIG_DIR=tools/benchmarks/results/cherry-ai-light-2026-10-25/configs-night-lights`,
`TERRA_BENCH_MODE=integrated`, `TERRA_BENCH_UI=1`,
`TERRA_BENCH_CAMERA=-574,39,962`, `TERRA_BENCH_LOOK_AT=-574,33,970`,
`TERRA_BENCH_WEATHER=clear`, `TERRA_BENCH_SECONDS=8`,
`TERRA_BENCH_LOAD_SECONDS=240`, `TERRA_CPU_SCOPES=1`, `WGPU_BACKEND=vulkan`.
`TERRA_BENCH_NO_PORTALS=1` schaltet ausschliesslich in Smoke-Tests die neue
Portalpruefung fuer einen Kontrolllauf aus.

## Visuelle Pruefung

`TERRA_CAVE_LIGHT_INSPECT=1`, `TERRA_CAPTURE_DIR=<dieser Ordner>/caves`,
`--smoke-test-caves`: native geschlossene Hoehlenfixture, vier Ansichten
14 bis 17 inklusive Unterwasser- und Live-Clock-Beleuchtungspruefung erfolgreich.
`caves.log` enthaelt die Photometrie; Screenshots liegen unter `caves/`.
Die Ansicht 15 wurde zusaetzlich visuell kontrolliert: Waende, Boden und
Fackellicht sind vorhanden. Das ist keine umfassende Grafik-Abnahme aller Seeds.
Nach erfolgreicher Fixture meldet ein beim App-Exit noch offener Screenshot
einen geschlossenen Kanal; der Test endet mit Exit 0. Kein Shaderfehler oder
Slab-Use-after-free wurde in diesen Laeufen beobachtet.

## Validierung

- `cargo fmt --all --check`: erfolgreich; Vendor-Datei separat mit Rustfmt geprueft.
- `cargo check --locked --workspace`: erfolgreich, einschliesslich Dedicated Server.
- `cargo test --locked --workspace --lib`: 2399 bestanden, 6 ignoriert,
  darunter 623 Renderer-Tests. `terra132-workspace-tests.log`.
- `CARGO_TARGET_DIR=$PWD/target cargo test --locked --manifest-path
  vendor/bevy_render/Cargo.toml --lib slab_allocator::tests`: 5 bestanden.
  `terra132-slab-tests.log`. Das sind Lifecycle-/Allocation-Tests auf dem
  Noop-Geraet, kein GPU-Datenreadback. Ein Aufruf ueber `-p bevy_render` ist
  wegen der separaten Vendor-Workspace-Grenze nicht zulaessig; deshalb der
  explizite Manifest-Aufruf.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: erfolgreich.
- `cargo build --locked -p terra-client`: erfolgreich, nativer Build getestet.
- Native geschlossene Hoehlenfixture und beide Benchmark-Szenarien: Exit 0.

Neue Regressionen pruefen geschlossene und geoeffnete Hoehlen, negative
Section-Grenzen, unbekannte/teiltransparente Blockgrenzen, Freigabe vorhandener
GPU-Meshes bei erhaltener Weltdaten-Section, Verwerfen verborgener Upload-Jobs,
Wiederfreigabe nach Invalidierung, opake gegen Cutout-Schatten trotz geleerter
Staging-Attribute sowie gleichzeitiges Wachstum zweier Slab-Layouts mit
erhaltenen alten Allokationen. Keine neue 10-/15-/30-Minuten-Abnahme.
