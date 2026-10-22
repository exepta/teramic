# Phase 35: Frisch Generierte Chunks Einblenden

## Ursache Und Korrektur

Der entscheidende sichtbare Fehler war die Zeichenreihenfolge der
kamerazentrierten Horizonthuelle. Bevy sortiert transparente Meshes nach ihrem
Mittelpunkt. Der Mittelpunkt der Huelle liegt an der Kamera, sodass sie zuletzt
gezeichnet wurde und die transparent einblendenden Chunks vollstaendig
ueberdeckte. Erst beim Wechsel zum opaken Originalmaterial waren die Chunks
sichtbar. Bereits vorhandene opake Geometrie verdeckte die Huelle per Tiefentest,
wodurch Wiederholungspruefungen an fertigem Terrain den Fehler nicht zeigten.

Die Huelle erhaelt einen Sortier-Bias von -2 mal Kamera-Far. Ihre vorhandene
Material-Extension setzt den Raster-Bias anschliessend auf null und behaelt
Tiefenschreiben aus. So bleibt der reale Tiefentest der entfernten Huelle
erhalten und sie liegt auch hinter einblendendem Terrain, Pflanzen und Wasser.
Kein weiterer Renderpass, keine zusaetzliche Geometrie und kein GPU-Warten.

Zusaetzlich verbrauchte der erste gezeichnete Materialbatch eines Chunks die
Fadezeit aller Geschwister. Jeder Batch fuehrt nun seinen eigenen Startzeitpunkt
ab seinem ersten erfolgreichen Farb-Draw. Fertige Teile blenden bereits ein,
waehrend spaet hochgeladene Teile Alpha null behalten. Die gemeinsame Bewegung
der Section bleibt zusammenhaengend. Beim Entladen wird die erreichte Deckkraft
jedes Teils uebernommen; unsichtbare Teile blitzen dabei nicht auf.

Offscreen-Materialgrenzen halten fertige Animationen nicht unbegrenzt besetzt.
Sichtpruefungen erfolgen nur fuer noch nicht fertige Teile beim Abschluss;
Kameradaten werden dabei einmal pro Frame geteilt. Bereits identische
Alphawerte erzeugen keine Asset-Aenderung oder erneuten Uniformupload.

Ein vorhandener leerer Prop-/Wassercache zaehlt nun noch nicht als sichtbarer
Erstupload. Erst das tatsaechlich erzeugte Render-Mesh startet die Animation.
Wasserwurzeln besitzen dafuer eine explizite Visibility-Komponente.

Generierungs-/Meshing-Worker, Uploadbudgets, LOD-Distanzen, 96 aktive
Animationswurzeln und acht Neuaufnahmen pro Frame bleiben unveraendert.
Die Aenderungen befinden sich ausschliesslich in Darstellung und Pruefcode.

## Nachweise

Der native `--smoke-test-animations` beobachtet jetzt eine gerade generierte
Section bei ihrem ersten Upload, bevor sie jemals in einem Farb-Draw erschien.
Danach prueft er dieselbe Section erneut mit Slow Fade, Bounce, Rise und Glide.
Er verfolgt das ausgewaehlte sichtbare Material statt eines beliebigen Kindes.

Eine zusaetzliche Screenshotpruefung verlangt sichtbare Terrainfarben waehrend
der ersten Teildeckkraft. Sie beruecksichtigt RGBA-/BGRA-Kanalreihenfolge.
Die erste native Zwischenpruefung hatte zwar korrekte Alpha-/Drawwerte, aber
ein vollstaendig blaues Bild; die Aufnahme ist als
`captures/before-horizon-order.png` erhalten und wird nicht als Sichtnachweis
gewertet. Nach der Horizontkorrektur wurden 68623 Terrainfarbpixel gemessen,
siehe `captures/fresh-partial-fade.png`. Beide Aufnahmen wurden geoeffnet.
Die Pixelpruefung gilt fuer das Bild, nicht fuer eine ausschliessliche
Zuordnung einzelner Pixel zum ausgewaehlten Section-Mesh.

Fuenf neue Regressionstests pruefen:

- Verspaetete Materialteile bleiben unsichtbar und erhalten ihren eigenen Fade,
  waehrend fertige Geschwister bereits sichtbar werden.
- Nicht im Bild liegende Geschwister geben fertige Animationsplaetze frei.
- Entladen waehrend des Eingangs erhoeht keine bereits erreichte Deckkraft.
- Erste Propgeometrie nach leerem Cache wird korrekt eingereiht.
- Erste Wassergeometrie nach trockenem Cache wird korrekt eingereiht.

Am finalen Quellstand bestanden:

```text
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

597 Rust-Tests erfolgreich, ein vorhandener ignoriert. Cargo-Builds verwenden
lokal `CARGO_BUILD_JOBS=2`. Native Erstupload-/Wiederholungspruefung samt
Vulkan-/Shaderausfuehrung bestanden. Keine Dependency- oder Serveraenderungen.

## Streamingvergleich

RX 6950 XT, RADV Mesa 26.2.4, Vulkan, KDE Wayland, physisch 1920x1200,
VSync aus. Beide Binaries verwenden denselben Seed 42042, dieselben
32er-Konfigurationen aus Phase 34, vertikale Sichtweite 3, Fade 300 ms und
denselben 120-s-Flug ueber 224 m hin und zurueck. Die bisherigen LOD-Distanzen
sind fuer beide per `TERRA_BENCH_LEGACY_LOD=1` gleich gehalten. Keine
parallelen Builds, Tests oder eigenen GPU-Anwendungen im Messfenster.

Alle Laeufe laden am Ende 31868 Sections, 20418 Terrainwurzeln, 41433 Meshassets,
312135 logische Props in 4783 Batches und 89 Wassertiles. Der Endblick hat
14306 sichtbare Mesh-Entities ohne beziehungsweise 14307 mit Horizonthuelle.
Die Animation ist dann im Bild abgeschlossen:
active=0, queued_in_view=0. Offscreen-Wurzeln duerfen weiter unsichtbar warten.

### Isolierte Batch-Uhren

Beide Laeufe mit `TERRA_BENCH_DISABLE_HORIZON=1`; dieses Paar prueft den
zusaetzlichen Aufwand der Materialuhren und nicht die Horizontkorrektur.

| Messung | Vorher | Nachher |
| --- | ---: | ---: |
| Mittlere FPS | 32.85 | 32.86 |
| p95 Frame ms | 40.245 | 41.005 |
| p99 Frame ms | 51.334 | 61.316 |
| Initial bereit s | 35.891 | 36.572 |
| Terrain-Workerzeit gesamt s | 9.293 | 9.331 |
| Terrain-Kompositionsjobs | 60393 | 60144 |
| Prop-Workerzeit gesamt s | 3.157 | 3.215 |
| Peak-RSS KiB | 2452244 | 2420748 |
| Peak-DRM-VRAM KiB | 4762576 | 4762604 |

Mittlere FPS und Workerzeiten sind in diesem Paar nahezu gleich, p99 ist
schlechter. Daraus wird keine allgemeine Frame-Spitzen-Paritaet abgeleitet.
Die Zahl der LOD-Kompositionsjobs kann durch asynchrone Fertigstellungszeitpunkte
variieren. Summierte Workerzeit ist keine reine Generierungs- oder Ladezeit.
Peak-RSS und DRM-VRAM sind verschiedene Messgroessen.

### Vollstaendiger Fix Mit Aktivem Horizont

Dieses zweite Paar verwendet den finalen Stand inklusive korrigierter
Horizontsortierung. Gleiche Strecke, Einstellungen und Diagnoseoptionen;
`TERRA_BENCH_DISABLE_HORIZON` ist in beiden Laeufen nicht gesetzt.

| Messung | Vorher | Final |
| --- | ---: | ---: |
| Mittlere FPS | 30.70 | 31.66 |
| p95 Frame ms | 47.222 | 41.573 |
| p99 Frame ms | 102.639 | 48.636 |
| Initial bereit s | 37.143 | 36.033 |
| Terrain-Workerzeit gesamt s | 9.676 | 9.303 |
| Terrain-Kompositionsjobs | 59205 | 59559 |
| Prop-Workerzeit gesamt s | 3.384 | 3.188 |
| Peak-RSS KiB | 2440576 | 2460380 |
| Peak-DRM-VRAM KiB | 5063136 | 4508980 |
| Opaque-GPU-Pass Mittel ms | 2.210 | 2.190 |
| Transparent-GPU-Pass Mittel ms | 0.0446 | 0.0435 |

In diesem Paar keine gemessene Verlangsamung von mittlerer Bildrate,
Workerkomposition oder initialer Bereitschaft. Keine daraus abgeleitete
allgemeine Beschleunigung: insbesondere p99 schwankt deutlich zwischen den
Paaren. Peak-RSS liegt rund 19.3 MiB hoeher, VRAM niedriger; dynamische
Allokationsspitzen werden nicht als garantierte Speicherersparnis ausgegeben.
Die einzelnen GPU-Paesse sind keine Gesamt-GPU-Framezeit.

Die endgueltigen Mesh-/Section-/Propzahlen sind identisch. Der instrumentierte
letzte Frame meldet in beiden Laeufen dieselben 67 PBR-API-Aufrufe und einen
normalen UI-Draw. Das ist ein API-Snapshot, keine vollstaendige GPU-Subdrawzahl.
Beide Fluege beenden alle sichtbaren Fades und ausstehenden Meshjobs.

## Reproduktion

Vom Repository-Root aus, mit aktiviertem Horizont:

```bash
env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/rendering-streaming-2026-10-16/configs32 \
  TERRA_MENU_CONFIG_DIR=/tmp/terra-fresh-fade-benchmark-menu \
  TERRA_BENCH_SECONDS=120 TERRA_BENCH_LEGACY_LOD=1 \
  TERRA_MEASURE_GPU_MEMORY=1 TERRA_RENDER_DIAGNOSTICS=1 TERRA_DRAW_DIAGNOSTICS=1 \
  python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-lod
```

Fuer das isolierte Batch-Uhren-Paar zusaetzlich
`TERRA_BENCH_DISABLE_HORIZON=1` setzen. Die alten Binaries wurden vor dem Build
separat gesichert. Benutzerkonfigurationen wurden nicht ueberschrieben.

Native Erstuploadpruefung inklusive Screenshot-Pixeltest:

```bash
env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/fluid-recession-mushrooms-2026-10-16/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra-fresh-fade-native-menu \
  TERRA_CAPTURE_DIR=/tmp/terra-fresh-fade-captures \
  target/debug/terra-client --smoke-test-animations
```

Die historischen Phase-34-Fade-/32er-Kosten bleiben bestehen. Diese Korrektur
behebt den Erstuploadfehler; sie ist keine allgemeine Streamingoptimierung oder
Mehr-GPU-/Langzeitabnahme. Die sonstigen offenen Performancearbeiten bleiben in
der Haupt-TODO.
