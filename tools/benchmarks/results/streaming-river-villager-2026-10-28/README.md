# Streaming Uploads Flussabzweige Und Dorfbewohner

Flussabzweige und getragene NPC-Items sind implementiert und nativ geprueft.
Die Mesh-Aufbereitung wird billiger; der kritische Min-FPS-Fehler bleibt offen.

## Messbedingungen

Ryzen 9 5900X, RX 6950 XT, Vulkan/RADV, 1280x800, Sichtweite 20,
hohe Schatten-, Wasser- und Wetterqualitaet, X11-Fenster. Laufende Desktop-
Anwendungen wurden nicht beendet. Kurze native Hin-/Rueckfluege
mit gleicher Kamera und gleichem Chunk-Fokus; keine Compiler oder Tests
parallel zu den FPS-Fenstern. Keine 10-/15-/30-Minuten-Soaktests.

Die Performancepaare behalten Landschaftsrevision 1 und Flussrevision 1,
damit neue Hydrologie nicht die Vergleichsgeometrie veraendert. Konfigurationen
liegen in `configs/`, Rohlogs und Bilder in `native/`.
Die separate Flussgalerie verwendet `configs-river/` mit Flussrevision 2;
Baeume, Hoehlen und Strukturen sind nur dort fuer die freie Sicht abgeschaltet.
Alle Tests verwenden eigene temporaere Welten, keine Spielerstaende.

## Performancekorrekturen

- Props haben Dach-/Wasser-Exposure-Snapshots berechnet und deren Aenderungen
  als Mesh-Invalidierung behandelt, obwohl die Komposition diese Daten nicht
  konsumiert. Diese redundanten Snapshots, Epochs und Uploadsperren entfallen.
  Tatsaechliche lokale Surface-, Terrain-, Prop-, Support-, Wachstums- und
  Schneeaenderungen bleiben aktiv, einschliesslich Section-Halos.
- Die Lampensuche prueft zuerst die effektiven Section-Zellen auf vorhandene
  Emittermaterialien. Lampenfreie Sections vermeiden danach tausende
  Dictionary-Lookups. Journal-Edits und importierte Lampen bleiben enthalten.
- Props verwenden das vorhandene Worker-Packing nun in allen LODs:
  Normalen `Snorm8x4`, Farben `Unorm8x4` statt jeweils Float32-Vektoren.
  Typische bewachsene Layouts sinken von 68 auf 48 Byte pro Vertex;
  Positionen, UVs, Dreiecke, Wachstumsdaten und Distanzen bleiben gleich.
  Es sind quantisierte Attribute, keine verlustfreie Float-Kompression.
  Keine neuen Shaderformate, reduzierten Sichtweiten oder ausgeduennten Props.

Normales Spielen ignoriert den Testschalter. Nur Smoke-Tests koennen mit
`TERRA_PACKED_PROPS=0` Near/Middle fuer den kontrollierten Vergleich auf den
alten Attributpfad zurueckschalten. Far bleibt immer gepackt.

## Native Ergebnisse

| Szene und Stand | AVG FPS | Min FPS | p95 ms | p99 ms | Frames unter 32 FPS |
| --- | ---: | ---: | ---: | ---: | ---: |
| Klar 42042, 32 s, vorher mit Zeitspur | 49.62 | 23.15 | 32.22 | 36.19 | 106 / 1588 |
| Klar, Exposure/Lampen korrigiert, gleiche Zeitspur | 49.27 | 23.21 | 33.01 | 36.98 | 132 / 1576 |
| Klar, korrigiert, ungepackt ohne Zeitspur | 50.46 | 24.47 | 31.93 | 36.03 | 106 / 1614 |
| Klar, zusaetzlich gepackt ohne Zeitspur | 53.07 | 24.85 | 29.67 | 34.09 | 49 / 1697 |
| Klar, Produktionsdefault ohne Testschalter | 50.92 | 22.74 | 32.58 | 38.05 | 117 / 1629 |
| Klar, direkte Kontrolle gleiche finale Binary, Packing aus | 50.19 | 24.48 | 32.58 | 36.54 | 117 / 1606 |
| Klar, direkte Kontrolle gleiche finale Binary, Produktionsdefault | 52.53 | 23.68 | 31.26 | 35.62 | 85 / 1680 |
| Wasser/Regen Seed 7, 24 s, vorher | 57.57 | 24.13 | 29.73 | 34.87 | 43 / 1381 |
| Wasser/Regen, Exposure/Lampen korrigiert | 59.83 | 31.76 | 26.79 | 29.02 | 1 / 1436 |
| Wasser/Regen, zusaetzlich gepackt | 59.79 | 31.15 | 26.97 | 30.01 | 3 / 1435 |

Der erste klare A/B-Vergleich belegt KEINEN FPS-Gewinn durch Exposure/Lampen
allein. Die mittleren instrumentierten Intervalle fuer Lampenauswahl sinken
von 0.746 auf 0.606 ms, fuer Prop-Dispatch von 0.660 auf 0.572 ms. Die
tatsaechliche Zahl abgeschlossener Meshjobs wird dabei nicht kleiner;
Streaming-Reihenfolge und LOD-/Terrain-Revisionen bleiben weitere Ausloeser.

Im klaren Packing-Paar sinkt der exakte MeshAllocator-Body von 4.419 auf
3.707 ms pro Frame. Ladezeit 35.50 -> 35.42 s, RSS 2640656 -> 2652980 KiB,
repraesentierte Props beide 166183. Es gibt keinen belegten RSS-Gewinn.
Die erste Halbierung langsamer Frames wird im Produktions-Kontrolllauf nicht
stabil reproduziert. Der anschliessende direkte An/Aus-Vergleich derselben
finalen Binary zeigt einen kleineren Gewinn: 117 -> 85 langsame Frames,
50.19 -> 52.53 AVG FPS, aber KEINE Verbesserung der schlechtesten Einzelbilder.
Der MeshAllocator-Body sinkt dort erneut von 4.496 auf 3.677 ms.
Der Regenlauf ist durch Packing nicht nochmals schneller geworden.
Zeitspur-/CPU-Wallintervalle sind nicht additiv und keine GPU-Zeitmessung.

**32 FPS als feste Untergrenze sind weiterhin nicht erreicht.** Einzelne
Land-Flugframes benoetigen noch etwa 40 bis 44 ms. Der direkte Vergleich
belegt weniger langsame Frames, der Wiederholungslauf aber die verbleibende
Schwankung. Keine allgemeine Behebung saemtlicher Streaming-Drops,
keine Langzeitabnahme und keine GPU-/VRAM- oder 4K-Abnahme.

## Echte Flussabzweige

Die vorhandene regionale Hydrologie erzeugt bis zu zwei eigenstaendige
Abzweige pro Einzugsgebiet. Jeder beginnt exakt am Hauptfluss, besitzt einen
eigenen Kurs und eine getrennte validierte Meer- oder See-Muendung.
Gemeinsames Bed-/Bank-/Flussrichtungssampling und bestehende Wasserzellen
bleiben erhalten; keine zweite Fluessigkeitssimulation.

Der Split und beide Arme benoetigen mindestens 128 m offenen Landverlauf.
Der Seitenarm muss sich dabei tatsaechlich vom Hauptlauf trennen.
Segment-gegen-Segment-Pruefungen verhindern uebersehene Kreuzungen zwischen
Abtastpunkten. Ozeanarme duerfen keinen Zwischensee kreuzen und dadurch
ihren bereits abgesenkten Pegel wieder anheben.

Die erste native Fixture zeigte einen echten Testfehler: Meerwasser allein
erfuellte die Wassersaeulenpruefung und liess einen vollstaendig unterseeischen
Kurs bestehen. Produktion und Fixture verlangen deshalb nun explizit
exponierten Landgrund sowie die unabhaengigen Landabschnitte.

Reproduzierbarer finaler Seed 42042, Region [0, -1]:
Split [1041.514, 9.544, -1006.486], Hauptmuendung [1248, 8, -1248],
Seitenmuendung [1472, 8, -1696]. Native Wassersaeulen, Flussrichtung und
getrennte Muendung geprueft; Split- und Seitenlaufbilder tatsaechlich angesehen.

Neue Welten erhalten `river_terrain_revision=2`. Bestehende gespeicherte
Revisionen 0/1 und deren `rivers`-Wert bleiben erhalten, ohne Retrogen oder
neue Welt-IDs. Die Hoehen-/Biom-Basis des zurueckgesetzten Main-Terrains
bleibt unveraendert. Neue Dedicated-Beispielkonfiguration aktiviert Fluesse.

## Getragene Dorfbewohner Ausruestung

Echte Items aus demselben DropModels-/Material-/Mesh-Cache wie beim Spieler,
mit gemeinsamer Drittperson-Griffpose am animierten rechten Arm. Kein zweites
Waffenmodell im Arm, keine pro Frame neu erzeugten Tool-Meshes.
Schlaf blendet gehaltene Items aus; Bogen und Priesterstab bleiben Rueckendeko.

Die native Galerie prueft zehn Rollen und neun gehaltene Items, eindeutige
Armzuordnung, Laufen, Aktionspose und Schlaf ohne gehaltene Items. Fuenf
GPU-Screenshots mit Nichtleer-Pruefung, Nah- und Aktionsansicht tatsaechlich
angesehen. Das ist ein Renderfixture, keine neue Berufsarbeits-KI.

Beide Galerien warten vor dem Exit auf bestehende Preview-Readback-Queues;
die fruehen Exit-only-Screenshotfehler werden nicht im Enginecode unterdrueckt.

- [Flusssplit](native/river-distributary-split.png)
- [Eigenstaendiger Seitenlauf](native/river-distributary-branch.png)
- [Ausruestung Nahansicht](native/villagers-1.png)
- [Aktionspose](native/villagers-4.png)

## Validierung

- `cargo fmt --all --check`: bestanden.
- `cargo check --locked --workspace --all-targets -j1`: bestanden.
- `cargo test --locked --workspace -j1 -- --test-threads=2`: 2780 bestanden,
  7 bestehende ignoriert, keine neuen ignorierten Tests.
- `cargo clippy --locked --workspace --all-targets -j1 -- -D warnings`: bestanden.
- `cargo build --locked --workspace --bins -j1`: Client und Dedicated Server gebaut.
- Native finale Kontrollfluege ohne Testschalter und korrigierte Galerien
  ohne Warnungen, Renderfehler oder Panics abgeschlossen.

Die ersten Gesamtrunden zeigten zwei bisherige Float32-only Farbfixtures
sowie zwei historische Geometriefixtures ohne ausdrueckliche Flussrevision.
Farbtests vergleichen nun exakt quantisierte Bytes bzw. dieselbe Shader-
Dekodierung; keine pauschal erweiterten Toleranzen. Historische Koordinaten
verwenden ausdruecklich ihren alten Contract 1, mit zusaetzlicher neuer
Contract-2-Spawn-Regression fuer echte erhoehte Fluss-/Abzweig-Wasserzellen.
Die abschliessende gesamte Suite bestand; Zwischen- und Abschlusslogs liegen
in `validation/`. Assertions und Produktionsgeometrie wurden nicht fuer die
historischen Testkoordinaten abgeschwaecht.

Klaren Flug mit frischem temporaerem Testpfad wiederholen:

```bash
env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET WINIT_X11_SCALE_FACTOR=1 \
  TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/streaming-river-villager-2026-10-28/configs" \
  TERRA_WORLD_DIR=/tmp/teramic-streaming-control-new \
  TERRA_BENCH_GRAPHICS=1 TERRA_BENCH_SECONDS=32 TERRA_BENCH_LOAD_SECONDS=120 \
  TERRA_CPU_SCOPES=1 WGPU_BACKEND=vulkan \
  target/debug/terra-client --smoke-test-lod
```

Die ungepackte Smoke-Kontrolle ergaenzt `TERRA_PACKED_PROPS=0`.
Flussgalerie: `configs-river/`, `TERRA_RIVER_DISTRIBUTARY=1`,
`TERRA_CAPTURE_DIR=/tmp/teramic-river-gallery-new`, `--smoke-test-rivers`.
NPC-Galerie: `TERRA_SMOKE_VILLAGERS=1`, eigener Capture-/Testweltpfad,
`--smoke-test-creatures`. Keine dieser Fixtures ist eine neue Gameplay-KI.
