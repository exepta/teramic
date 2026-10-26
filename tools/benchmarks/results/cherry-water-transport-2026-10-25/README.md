# Kirschholz, Wassertransport Und Renderkosten

## Implementierung

- Preprocessing-Bindings verwenden die originale oeffentliche Bevy-Funktion,
  aber nur fuer Views mit geaenderten Buffer-IDs, Bindegroessen, Layouts,
  Unpacking-Auftraegen oder Tiefenpyramiden. Retained-View-IDs statt kurzlebiger
  Shadow-Entities; maximal zwei Zustaende fuer die wechselnden Buffersaetze.
  GPU-Daten und dynamische Schatten werden weiterhin jeden Frame aktualisiert.
- Grosse Terrain-/Prop-Batches werden im Worker auf maximal 128.000/96.000
  Vertices geteilt. Uploads laufen innerhalb derselben Frame-Budgets ueber
  mehrere Frames; der bisherige Abschnitt bleibt bis zum vollstaendigen Ersatz
  sichtbar. Veraltete Resultate werden vor dem Upload verworfen. Dreiecke,
  Materialdaten und Wachstumsbereiche bleiben erhalten.
- Entfernte, stabile und undurchsichtige Terrain-Batches gleichen Materials
  koennen in 32-m-Gruppen zusammengefuehrt werden. Hoechstens zwei Worker,
  maximal 64.000 Vertices pro Gruppe und 64 MiB zusaetzliche CPU-Quelldaten.
  Aenderungen stellen die Quellmeshes sofort wieder her. Windlaub, Wasser,
  Nahdetails und transparente Materialien sind nicht betroffen.
- Leere Punktlichtseiten lassen nur ihre unnoetigen Compute-Dispatches aus.
  Alle sechs Tiefenflaechen werden weiterhin geloescht und gerendert.
- Kirschholz ergaenzt IDs 134-140: Stamm, Planken, Treppen, Slabs, Zaun,
  Tuer und Fenster. Eigene Rinden-/Hirnholzmuster, rosa Holz, bestehende
  Ausrichtungen, Kollision, kombinierte Slabs, animierte Oeffnungen, Sounds,
  Brennstoff und sechs gemeinsame Crafting-/Too-Many-Items-Rezepte.
  Ecology 2 verwendet die neuen Staemme. Gespeicherte Revisionen 0/1 behalten
  ihre alten Materialien und Baumpositionen. Bestehende IDs bleiben gleich.
- F13: Eisen-Eimer mit Werkbankrezept, gefuellte Variante, Benutzen/Platzieren
  zum Aufnehmen und Ausgiessen echter Quellen. Der Server prueft Zielstrahl,
  Reichweite, Baurechte, Welt-/Inventarrevision und gehaltenen Slot. Derselbe
  Slot wechselt zwischen leer und voll; das braucht keinen freien Zusatzslot.
  Quellen nutzen die vorhandenen Fluidzellen, Fluss-Updates und Regiondateien.
  Explizite trockene Zellen verhindern die Rueckkehr entnommener natuerlicher
  Quellen beim Nachladen. Protokoll 75 lehnt alte inkompatible Clients ab.
- Wasserfall-Fix: Explizite Fluidzellen ersetzen die natuerliche Reservoir-
  Oberflaeche statt einen zweiten koplanaren Wasserspiegel zu erzeugen.
  Ganzzahlige Wasserstaende und explizite trockene Zellen sind beruecksichtigt.

## Messungen

RX 6950 XT / RADV / Vulkan, optimierter Dev-Build. Keine Cargo-Prozesse
waehrend der FPS-Fenster, unveraenderte Sichtweite, Dichte und Schattenqualitaet.
CPU-Wall-Frames, keine gemessenen GPU-Passzeiten. Die jeweiligen effektiven
Konfigurationen, Aufloesungen, Kameras und Wetterzustaende stehen in den Logs.

### Direkter Nachtvergleich

Seed 42042, Strukturrevision 5, Ecology 0, pausiert 22 Uhr; integrierter Server,
echte HUD, 1280x800, Sichtweite 12, acht schattenwerfende Punktlichter.
Je acht Sekunden nach dem Laden. Derselbe Zielblick bei [-574, 39, 962];
die physikalische Kameraposition stimmt bis auf Rundung ueberein.

| Messwert | Vorher | Nachher |
| --- | ---: | ---: |
| Mittlere FPS | 62,45 | 64,07 |
| p95 Framezeit | 19,424 ms | 18,666 ms |
| p99 Framezeit | 20,604 ms | 19,768 ms |
| Hoechste zugelassene Uploadmenge/Frame, inklusive Ladephase | 395.700 Vertices | 220.932 Vertices |
| Vollstaendige Szenenladezeit, inklusive Teleport | 26,21 s | 40,56 s |
| Prozess-RSS am Messende | 1.901.960 KiB | 2.086.168 KiB |
| Mesh-Assets | 11.372 | 11.500 |
| Dargestellte Props | 80.732 | 80.732 |
| Prop-Batches | 1.188 | 1.255 |
| Preprocess-Helfer, eigentlicher Funktionskoerper | 1,471 ms/Frame | 0,994 ms/Frame |
| Zusaetzliche Cache-Signaturpruefung | - | 0,252 ms/Frame |

Rohdaten: `terra129-before-night.log`, `terra129-tuned-night.log`.
Der FPS-Unterschied betraegt nur rund 2,6 Prozent in kurzen Fenstern, nicht
einen pauschalen Durchbruch. Die Uploadspitze ist um 44,2 Prozent niedriger.
Ladezeit, RSS und Batchanzahl sind ausdruecklich **nicht** verbessert.
Die atomare Staffelung erhoeht die Wartezeit auf vollstaendige Abschnitte;
der Binding-Cache und Fernbatches halten begrenzt zusaetzliche Ressourcen.
43 Fern-Gruppen ersetzen 122 sichtbare Quellbatches, mit 2,37 MB CPU-Quelldaten.
Das sind Batchzahlen, keine gemessenen Draw Calls oder VRAM-Werte.

Kleinere erste Uploadstuecke mit 64.000/48.000 Vertices erzeugten 1.359
Prop-Batches und keinen belastbaren FPS-Gewinn. Die finalen 128.000/96.000
vermeiden einen Teil dieser unnoetigen Zerteilung. Keine Qualitaetsreduktion.

### Weitere Kurze Szenarien

| Szene | Seed | Fenster | FPS | p95 / p99 | RSS am Ende |
| --- | ---: | ---: | ---: | ---: | ---: |
| Hin-/Rueckflug, Wasser/Regen, 1280x800, Sichtweite 12 | 7 | 24 s | 97,99 | 15,456 / 17,859 ms | 1.253.784 KiB |
| Dorf/Regen, 3840x2160, Sichtweite 20, Ultra-Wetter, volle HUD | 42042 | 8 s | 51,63 | 29,858 / 37,840 ms | 2.671.868 KiB |
| Erzwungener Schnee 3, 1280x800, Sichtweite 12 | 104729 | 8 s | 143,01 | 9,090 / 10,017 ms | 1.357.504 KiB |

Fuer diese Szenen wurde vor dem Umbau kein neuer gepaarter Ausgangslauf
aufgenommen. Sie sind Funktions-/Lastpruefungen, keine behaupteten FPS-Gewinne.
Die beiden Offline-Szenen haben keinen integrierten Server und keine volle HUD;
ihre FPS duerfen nicht mit dem Dorf gleichgesetzt werden. Der Regenflug hat
625 Wassertiles und 2.425 Regenquads; er kehrt zur Startregion zurueck und
arbeitet alle Mesh-Jobs ab (5.451 abgeschlossene LOD-Wechsel). Schnee 3 ist
wirklich gerendert: 10.404 Wetterquads, kein Test saisonaler Schneepersistenz.

Rohdaten: `terra129-tuned-flight-seed7.log`,
`terra129-tuned-rain-4k-borderless.log`, `terra129-tuned-snow-104729.log`.
Die 4K-Szene bleibt teuer; langsame Frames treten auch ohne Uploads oder Jobs
auf. Keine Behauptung, den allgemeinen FPS-Kollaps endgueltig behoben zu haben.
Der 4K-Lauf benoetigt 133,18 s bis zur vollstaendig geladenen Teleport-Szene.

Ein vorheriger 4K-Fensterlauf wurde vom Windowmanager auf 3840x2124 begrenzt
und deshalb vom Test korrekt ohne FPS-Ergebnis verworfen
(`terra129-tuned-rain-4k.log`). Der gueltige Nachlauf verwendet die eigene
`configs-4k/graphics.toml` im Borderless-Modus und validiert echte 3840x2160.
Eine Monitor-Auswahlwarnung beim Erzeugen dieses Fensters blieb ohne
Aufloesungs-/Renderfehler. Nicht als schnellerer Renderer durch einen anderen
Praesentationsmodus ausgeben.

## Funktionspruefung

- Der komplette Workspace-Testlauf besteht mit 2.512 Tests; sieben vorhandene
  opt-in/ignorierte Tests wurden nicht ausgefuehrt. Auch der Bucket-Authority-
  Test, reale Regiondatei-Roundtrips, beide Shadow-Schedule-Tests, gestaffelter
  atomarer Ersatz, Fernbatch-Unload und Cherry-Revisionserhalt bestehen.
- Nativer Wasserfalltest: zwei kontrollierte Aufnahmen in `waterfalls/`;
  echte Fluidzellen ueber Stufen, Teilbloecke und Section-Grenzen. Der Zulauf
  wird anschliessend geschlossen, die Zahl geladener bewegter Zellen sinkt
  von 959 auf 477. Kein statischer Shader-Wasserfall als Ersatz.
- `cherry/` zeigt wirklich generierte Baeume bei [-2192, 44, -3200], Ecology 2.
  Die Uebersicht mit neuen Staemmen und die Kronen-Nahaufnahme wurden geoeffnet.
- Headless-Start ohne DISPLAY/WAYLAND_DISPLAY, `--smoke-test --no-save`:
  27 Sections erfolgreich, keine GUI-/GPU-Abhaengigkeit des Servers.
- Die finale Materialgalerie besteht mit 140 Bloecken und 140 GPU-Vorschauen.
  `materials-cherry-wood.png` wurde betrachtet: Stamm, Planken, Treppe, Slab,
  Zaun sowie die tatsaechlich platzierten Tueren/Fenster mit ihren gemeinsam
  animierbaren Teilen. Der Test legt echte Objektzustaende und beide Tuerhaelften
  an, einschliesslich einer vertikalen Section-Grenze; keine isolierten Icons
  als Ersatz fuer einen Test der Weltmodelle.
- Die Handansicht besteht in allen 18 Stufen, einschliesslich leerem und
  gefuelltem Eimer (`hands/hands-16.png`, `hands/hands-17.png`, betrachtet).
  Die gehaltene Geometrie bleibt an der unteren Bildkante verankert. Die
  Wasserflaeche bleibt innerhalb der Eimerwaende; ein eigener Geometrietest
  prueft Radius und nach oben gerichtete Flaechen. Neutrales Testlicht, keine
  Aenderung der eigentlichen Spielbeleuchtung.
- Ein frueherer Wasserfall-Lauf meldete beim Beenden noch einen verspaeteten
  Screenshot-Rueckruf auf einen geschlossenen Kanal. Die Testszene wartet nun
  nach der Fluidpruefung auf Vorschaujobs und alle Screenshot-Entities. Ein
  Regressionstest sichert diese Abschlussbarriere ab. Der finale native Lauf
  beendet sich ohne Warnungen oder Fehler; die Wasserpruefung wird nicht
  uebersprungen und der Logger nicht gefiltert.
- Nach den FPS-Laeufen wurden nur noch die Material-Testszene und die innere
  Wasserflaeche des Eimermodells korrigiert. Die gemessenen Render-/Streaming-
  Pfade blieben unveraendert; Eimer waren in den FPS-Szenen nicht ausgeruestet.

### Build Und Testbefehle

Alle erfolgreich, jeweils auf dem vorhandenen Workspace ohne Dependency-Upgrade:

```bash
cargo fmt --all --check
cargo check --locked --workspace
cargo test --locked --workspace --no-fail-fast -- --test-threads=2
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace
git diff --check
```

Finale Rohlogs: `terra129-tests-verified.log`, `terra129-check-verified2.log`,
`terra129-clippy-verified2.log`, `terra129-build-verified2.log`,
`terra129-fmt-verified2.log`, `terra129-headless-final.log`,
`terra129-materials-final.log`, `terra129-hands-verified.log`,
`terra129-waterfalls-final.log`. Kein Langzeittest und kein unbelegter
allgemeiner FPS-/Leak-Abschluss.

## Reproduktion

Aus dem Repository, nach `cargo build --locked --workspace`:

```bash
env TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/cherry-ai-light-2026-10-25/configs-night-lights" \
  TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 \
  TERRA_BENCH_CAMERA=-574,39,962 TERRA_BENCH_LOOK_AT=-574,33,970 \
  TERRA_BENCH_WEATHER=clear TERRA_BENCH_SECONDS=8 TERRA_BENCH_LOAD_SECONDS=240 \
  TERRA_CPU_SCOPES=1 WGPU_BACKEND=vulkan \
  target/debug/terra-client --smoke-test-benchmark
```

Der Flug verwendet `../tree-structures-performance-2026-10-25/configs-seed7`,
`TERRA_BENCH_GRAPHICS=1`, `TERRA_BENCH_WEATHER=rain`,
`TERRA_BENCH_SECONDS=24` und `--smoke-test-lod` (ohne feste Kamerakoordinaten).
Fuer 4K `configs-4k` mit Kamera `974,74.8,-557`, Ziel `974,74.1,-562`,
Wetter `rain` verwenden. Fuer Schnee `configs-snow`, Kamera
`-1143.5,90,1176.5`, Ziel `-1148,85,1160`, Wetter `snow 3`,
`TERRA_BENCH_GRAPHICS=1` und keinen integrierten Modus verwenden.

Die Materialgalerie laeuft mit `configs-cherry`, `--smoke-test-materials`
und `TERRA_SCREENSHOT=/tmp/materials.png`; die Handansicht mit
`--smoke-test-hands` und `TERRA_CAPTURE_DIR=/tmp/held-items`.
`--smoke-test-waterfalls` nutzt ebenfalls `TERRA_CAPTURE_DIR`.
Alle `configs-*` hier beziehen sich auf diesen Berichtsordner.

## Grenzen

- Im aktuellen Auftrag war kein neues Referenzbild angehaengt. Kirschholz
  folgt dem bestehenden Modell-/Texturstil, ohne behaupteten 1:1-Bildabgleich.
- Pumpen sind die in F13 ausdruecklich spaetere Ausbaustufe. Rohrnetze und
  Ventile bleiben F27; es gibt keine zweite Fluessigkeitssimulation.
- Wasserpatches behalten ihre bisherige Ausnahme fuer einen einzelnen
  uebergrossen Upload. Die neue atomare Staffelung gilt fuer Terrain/Props.
- Fern-Buendelung behaelt Quellmeshes fuer uploadfreies Zurueckschalten und
  benoetigt deshalb begrenzten zusaetzlichen Speicher. Sie reduziert sichtbare
  Batch-Entities, nicht automatisch die Gesamtzahl aller Mesh-Assets.
- Kurze Tests ersetzen keine allgemeine FPS-/VRAM-/Langzeitabnahme. A05 bleibt
  offen; keine erneuten 10-/15-/30-Minuten-Laeufe.
