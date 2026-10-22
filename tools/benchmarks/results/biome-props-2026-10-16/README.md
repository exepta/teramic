# Phase 36: Prop-Gruppen Und Biomfarben

## Vergleich

RX 6950 XT, RADV Mesa 26.2.4, Vulkan, KDE Wayland, 1920x1200 physisch,
VSync aus. Beide Binaries: Seed 42042, Sichtweite 16/3, identischer fester
Waldblick und 60 Sekunden Messfenster. Konfigurationen stammen unveraendert
aus `../fluid-recession-mushrooms-2026-10-16/configs`.

Der Vorher-Build wurde vor den Aenderungen separat gesichert; die zugehoerigen
Assets inklusive altem Wassershader ebenfalls. Keine parallelen eigenen
GPU-Anwendungen, Builds oder Tests in den beiden gueltigen Messfenstern.
`forest-discarded-overlap.log` ist ein vorzeitig beendeter, ungueltiger Vorlauf
mit gleichzeitig laufender Sichtpruefung und wird nicht ausgewertet.

| Messung | Vorher | Nachher |
| --- | ---: | ---: |
| Mittlere FPS | 82.751 | 79.025 |
| p95 Frame ms | 18.968 | 18.973 |
| p99 Frame ms | 24.440 | 21.219 |
| Initial bereit s | 5.003 | 5.450 |
| Terrain-Komposition Worker s | 1.2325 | 1.2704 |
| Prop-Komposition Worker s | 0.3116 | 0.5165 |
| Geladene Sections | 7809 | 7809 |
| Terrain-Renderwurzeln | 4118 | 4118 |
| Meshassets | 9696 | 9633 |
| Repraesentierte logische Props | 92784 | 156348 |
| Prop-Batches | 1306 | 1294 |
| Sichtbare Mesh-Entities | 6246 | 6245 |
| Peak RSS KiB | 918768 | 938236 |
| Peak DRM-VRAM KiB | 2701460 | 3641176 |
| GPU Opaque-Pass Mittel ms | 1.5651 | 1.7517 |
| GPU Early-Prepass Mittel ms | 0.7527 | 0.9423 |
| Opaque Vertexshader-Aufrufe/Frame | 6898255 | 8686023 |

Mehr Pflanzen und detaillierteres Nahgras sind messbar teurer: rund 4.5 Prozent
weniger mittlere FPS, 0.45 Sekunden laengeres initiales Bereitstellen,
hoehere Prop-Kompositionszeit und hoehere GPU-Kosten. Peak RSS steigt rund
19 MiB, der gemessene DRM-VRAM-Peak rund 918 MiB. Unterschiedliche dynamische
Allokationsspitzen sind kein exaktes Mass nur fuer Pflanzenpuffer; eine
Speicherparitaet wird ausdruecklich nicht behauptet.

Die bessere p99 dieses einzelnen Paars ist kein allgemeiner Nachweis gegen
Frame-Spitzen. Workerzeiten sind summierte Render-Kompositionszeiten, keine
separat gemessene CPU-Weltgenerierungszeit. Die GPU-Paesse sind keine komplette
GPU-Framezeit. Repraesentierte Props sind nicht frustum-sichtbare Einzelpflanzen.
Der letzte Draw-Snapshot hat in beiden Laeufen 43 indirekte PBR-API-Aufrufe und
einen normalen UI-Draw; GPU-Subdrawzahlen sind hier nicht ausgelesen.

Der Waldblick enthaelt kein sichtbares Wasser. Er misst weder die neue
Wasserfarbe noch Fluss-/Langzeitstreamingkosten. Die nativen Wasseransichten
sind Funktions-/Sichtpruefungen, keine Leistungsmessungen.

## Reproduktion

```bash
env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/fluid-recession-mushrooms-2026-10-16/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra-biome-props-benchmark-menu \
  TERRA_BENCH_SECONDS=60 TERRA_MEASURE_GPU_MEMORY=1 \
  TERRA_RENDER_DIAGNOSTICS=1 TERRA_DRAW_DIAGNOSTICS=1 \
  python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-forest
```

Rohwerte: `forest-before.log`, `forest-after.log`. Vorher verwendete das
gesicherte Binary `/tmp/terra-biome-props-before` und
`TERRA_ASSET_DIR=/tmp/terra-biome-props-before-assets`.

Native Wasseransichten:

```bash
env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/biome-props-2026-10-16/configs-rivers \
  TERRA_MENU_CONFIG_DIR=/tmp/terra-biome-props-native-menu \
  TERRA_BIOME_SCENE=water-mushroom \
  TERRA_CAPTURE_DIR=tools/benchmarks/results/biome-props-2026-10-16/captures \
  target/debug/terra-client --smoke-test-biomes
```

Weitere Szenen: `water-desert`, `water-highland`, `water-transition`, `oasis`.
Die Biom-Smokes verwenden absichtlich ihr festes Seed-42042-Referenzprofil;
die vier `water-*`-Szenen aktivieren zusaetzlich dessen Fluesse. Kein Einfluss
auf normale Spielstarts oder gespeicherte Benutzerwelten.

Gras-/Pflanzen-Nahansichten verwenden `--smoke-test-botanical` und
`TERRA_SCREENSHOT=.../captures/botanical.png`. Der Test prueft die reale
Zielauswahl, HUD-Daten und windbewegte Batch-Geometrie, nicht nur Bilddateien.

## Validierung

`cargo fmt --all --check`, `cargo check --workspace`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings` und
`cargo build --workspace` erfolgreich. 618 Rust-Tests bestanden, ein bestehender
Langzeittest bleibt ignoriert. Der Dedicated Server startet im Smoke-Test ohne
DISPLAY/WAYLAND_DISPLAY und ohne Fenster (`server.log`).

Die Tests pruefen unter anderem Viererbelegung inklusive echter UDP-Platzierung,
Ablehnung der fuenften Pflanze, einzelne Entfernungen und unveraenderte Nachbarn,
alle Tombstones beim Ueberbauen mehrerer Pflanzen sowie echtes Section-Unload/
Reload. Grassilhouette, freie Zwischenraeume bei Wind, gemeinsame Picking-
Geometrie, negative Section-Kanten, LOD-Farbinterpolation und Wetterwechsel
sind separat abgedeckt.

Der native Botanical-Test ist mit sieben Aufnahmen abgeschlossen
(`botanical.log`); Gras und Blumen-Nahaufnahme wurden tatsaechlich geoeffnet.
Die alte Nahkamera fuer grosse Einzelblumen wurde an die wirkliche Bluetenhoehe
der neuen Eckpflanzen angepasst. Es wurde kein Produktions-Picking vereinfacht,
um den Test zu bestehen.

Alle vier nativen Wasserpruefungen bestanden mit finalem Shader, jeweils mit
Uebersicht und Nahaufnahme. Alle acht Bilder wurden geoeffnet: Pilzpark
helllila, Wueste heller blau, Hochland kuehler/dunkler blau und ein echter
Plains-/Desert-Uebergang ohne sichtbare Section-Farbnaht. Unterwasserboden
bleibt in flachen Bereichen erkennbar. Logs: `water-mushroom.log`,
`water-desert.log`, `water-highland.log`, `water-transition.log`.

Die erste native Pilzparkpruefung deckte eine braune Tiefenfarbe auf: Zu geringe
Blau-Extinktion unterdrueckte gleichzeitig das blau gewichtete Streulicht.
Der finale Shader blendet deshalb abweichende Biomfarben weich auf neutrale
Extinktion um; reine Forest-/Plains-/Ocean-Farben behalten exakt ihre bisherigen
Koeffizienten. Diese letzte Korrektur betrifft nur WGSL und wurde in allen vier
GPU-Laeufen erneut kompiliert und geprueft. Sie ist keine zusaetzliche
Wasser-Leistungsmessung; der Waldvergleich enthaelt ohnehin kein Wasser.

Aufnahmen: [Gras](captures/botanical-grass.png),
[Pilzpark](captures/water-mushroom-close.png),
[Wueste](captures/water-desert-close.png),
[Hochland](captures/water-highland-close.png),
[Uebergang](captures/water-transition-overview.png).

## Grenzen

Ein GPU-Modell, ein statischer 60-s-Vergleich, keine Langzeit-/Mehr-GPU-Paritaet.
Der Referenzstil wurde geometrisch angenaehert; keine pixelgenaue 1:1-Wiedergabe
von Concept-Art-Beleuchtung oder vollstaendige gestalterische Abnahme.
Nahgras hat 552/644 Vertices, Middle 160, Far 80. Vierergruppen bleiben
Section-Batches, keine Einzel-Entities. Wasser fuegt ein RGB-Attribut mit
12 Byte/Vertex hinzu, aber keinen Pass und kein Material pro Biom.
Die Wasserfarbe bleibt von Boden, Licht, Blickwinkel und Tiefe abhaengig;
Unterwassernebel ist weiterhin nicht biomabhaengig. Die Konturen der echten
Bodenmaterialien und ihre Seiten bleiben voxelbasiert; gemischt werden die
Oberseitenfarben, nicht Block-IDs oder Texturgeometrie.
