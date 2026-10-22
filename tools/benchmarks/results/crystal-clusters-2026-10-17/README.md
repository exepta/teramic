# Phase 54: Kristallgruppen Und Gerichtetes Salz

## Darstellung

Die Referenz dient als Form-/Farbvorlage: kantige, breite Hauptkristalle,
kleinere Begleitspitzen, flache farbige Einschluesse und Risse im geschlossenen
Steintraeger. Diamant ist cyan, Gold warmgelb, Eisen warmgrau, Kohle dunkel,
Rubin rot und Salz warmweiss. Ein gemeinsames 512er-Mineralmuster mit Mips
erzeugt versetzte breite Kristallflaechen. Die UV-Projektion folgt der jeweiligen
Facettennormalen, damit lange Seiten nicht dieselbe flache Texelzeile abbilden.
Die schon vorhandene Steintextur bleibt erhalten.

Near/Middle/Far behalten bei normalen Erzen 4/3/2 Kristalle oben und 2/1/1
an den anderen Seiten. Die vierseitigen Prismen haben zusaetzliche Endfacetten.
Der volle, sechsseitig exponierte Nicht-Salz-Erzblock benoetigt maximal
288/168/108 Dekorationsdreiecke statt 384/204/156 in Phase 53. Der Steintraeger
ist darin nicht enthalten. Das sind Geometriebudgets, keine FPS-Messungen.

Salz waechst von der Decke nach unten und vom Boden nach oben. Ein stabiler
Positionshash waehlt ein Achtel der geeigneten horizontalen Traegerflaechen
fuer grosse Cluster aus. Deren Hauptspitze ist am Boden 2 bis 3.85 m hoch,
an der Decke 2 bis maximal 10 m lang. Kleinere Nebenarme sind kuerzer.
Andere Flaechen tragen kleine Kristalleinschluesse. Die Annahme zur Anfrage
ist: "oben mindestens zwei Bloecke" beschreibt die Hoehe des grossen Clusters.

Eine begrenzte senkrechte Freiraumabfrage verhindert Durchdringen von Terrain
und gesetzten Bloecken. Gegenueberliegende Cluster teilen engen Raum; ohne
mindestens zwei Meter Platz bleibt nur der kleine Einschluss. Alle Querachsen
bleiben innerhalb der Traegerzelle. Lange Cluster haben 60/36/24 Dreiecke,
kleine Einschluesse 12/8/4. Die grosse Silhouette bleibt in allen LODs.

## Integration Und Grenzen

Freiraum wird auf den vorhandenen Section-Workern gegen Terrain und EditJournal
berechnet. Nur positive Laengen werden als kleine sparse Liste im BlockMesh
gespeichert und mit dessen Epoch-/LOD-Cache wiederverwendet. Keine neue Entity,
kein Material pro Kristall, keine Lichtquelle, kein transparenter Zusatzpass.

Picking und Rendering verwenden dieselben Dreiecke und dieselbe Freiraumregel.
Die Auswahl langer Spitzen findet ihren Traeger auch ueber Section-Grenzen.
Journal-Snapshots erfassen zusaetzlich die senkrechte Clearance. Nur Rendercaches
mit Salzflaechen verfolgen zusaetzlich die bestehenden Blockrevisionen der
beiden vertikalen Nachbarsections. Nach einer Aenderung wird die Salzlaenge auf
dem Mesh-Worker neu geprueft. Es werden weder zusaetzliche Voxelgenerationen noch
Kollisions-Rebuilds ausgeloest. Normale Sections ohne Salz behalten ihre bisherige
Invalidierung. Das Neu-Komponieren betroffener Salzbatches hat dennoch Kosten.
Die Bewegungskollision bleibt der volle Traegerblock: Kristallarme sind
anklickbare, mit dem Traeger abbaubare Dekoration, keine eigenen Physikkoerper.
Schwerkraft, abgetrennte fallende Spitzen und Aufschlagschaden sind nicht enthalten.

Neue Welten verwenden Geologievertrag 3: Salzregionen bekommen die begrenzten
Kristallgruppen statt der alten bis zu 15 m langen massiven Kegel. Normale
Tropfsteinformationen bleiben bestehen. Alte Vertraege 0/1/2 bleiben beim Laden
unveraendert; insbesondere werden alte Salzkoerper nicht nachtraeglich geloescht.
Die neuen Erzfacetten und Salzdekorationen sind auch auf alten Traegern sichtbar.
Fuer die neue Gesamtform der Salzhoehlen eine neue Welt erstellen.
Generator 5, Seed, Block-IDs und Region-/Spielerdateiformate bleiben bestehen.
Protokoll 28 verhindert gemischte alte/neue Clients mit unterschiedlichem Picking.

## Validierung

Alle folgenden Befehle sind erfolgreich, ohne Dependency-Upgrades:

```bash
cargo fmt --all --check
CARGO_BUILD_JOBS=2 cargo check --locked --workspace
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 cargo test --locked --workspace
CARGO_BUILD_JOBS=2 cargo clippy --locked --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=2 cargo build --locked --workspace
```

869 Tests bestanden; ein bereits vorhandener langer UDP-Soaktest bleibt ignoriert.
Die serielle Testausfuehrung vermeidet konkurrierende Netzwerk-Testports.
Neue Regressionstests pruefen LOD-Geometriebudgets, stabile Spitzen, begrenzte
Laengen, Hindernisse, unbekannte Nachbarzellen, Picking ueber negative/vertikale
Section-Grenzen und Renderinvalidierung ohne zusaetzliche Kollisionsinvalidierung.
Logs: `tests.log`, `clippy.log`, `build.log`.

Native Vulkan-Laeufe mit der finalen Binary erzeugten sechs Erzansichten und
sechs Geologieansichten, ohne Shader-/Validierungsfehler. Die finalen Ansichten
von Diamant, Salzgruppe, Salzdecke und Salzhoehlenboden wurden visuell geprueft:
[Diamant](captures/ore-diamond.png), [Salzgruppe](captures/ore-salt_crystal.png),
[Salzdecke](captures/geology-salt-ceiling.png), [Salzhoehle](captures/geology-salt.png).
Es sind natuerlich generierte Vorkommen mit zusaetzlichem Diagnose-Inspektionslicht,
keine nachtraeglich eingesetzten Testbloecke. Das vorhandene blaue Hoehlenlicht
beeinflusst weiterhin die Farben; identische Referenzbeleuchtung oder eine
vollstaendige Art-Abnahme wird nicht behauptet.
`--smoke-test-ores` und `--smoke-test-geology` beendeten sich erfolgreich.

Der Dedicated Server wurde ohne `DISPLAY` und `WAYLAND_DISPLAY` mit
`--no-save --smoke-test` gestartet: 27 Sections, sauberer Exit, kein Fenster
(`server-headless.log`). Der Smoke-Modus bindet absichtlich einen freien Port.

## CPU-Messung

4375 Sections, Seed 42042, Section-Zentrum `(1050, -3, -1122)`, vier Worker,
ein Warmup und fuenf Wiederholungen je Modus. Median in Sekunden:

| Stand | Geologie | Headless-Generierung | Render-Vorbereitung | Quads | Block-Checksumme |
| --- | --- | ---: | ---: | ---: | ---: |
| Vorher | 2 | 0.490155 | 0.879671 | 859984 | 54827165 |
| Nachher | 2 | 0.507281 | 0.903830 | 859984 | 54827165 |
| Nachher | 3 | 0.497792 | 0.892672 | 859232 | 54799239 |

Alle Varianten haben 573 Props. Bei gleichem Terrainvertrag steigt die gemessene
Vorbereitungszeit um rund 2.75 %, die Headless-Zeit um 3.49 %. Keine CPU-Paritaet
behaupten. Geologie 3 hat anderen Inhalt und ist kein reiner Optimierungsvergleich.
Render-Vorbereitung umfasst gepolsterte Voxeldaten, Greedy-Meshing, Props, Wasser
und jetzt Salz-Clearance; sie ist keine isolierte Meshing- oder GPU-Uploadmessung.
Rohdaten: `worker-before-geology2.log`, `worker-after-geology2.log`,
`worker-after-geology3.log`; Werkzeug: `terra-world/examples/worker_benchmark.rs`.

## Nativer Framevergleich

Ein ruhendes Wald-Messpaar, jeweils 60 Sekunden nach Initialladen. Radeon RX 6950 XT,
Vulkan/Mesa 26.2.4, Wayland, physisch 1920 x 1080 (logisch 1280 x 720 bei 1.5 Scale).
Seed 42042, alter Geologievertrag 0, Erzrevision 1, Zeit 10 Uhr pausiert, Sichtweite
8/vertikal 3, FOV 65, VSync aus, klares Wetter, Wasser High und Reflexionen an.
Identische Kamera `(-622, 53.88851, 680)`. Die eingefrorene Vorher-Binary verwendet
ebenfalls eingefrorene Vorher-Assets. Compiler, CPU-Benchmark und weitere eigene
Spielprozesse liefen nicht parallel.

| Messwert | Vorher | Nachher |
| --- | ---: | ---: |
| Mittlere FPS | 149.12 | 158.19 |
| p95 Framezeit (ms) | 11.314 | 9.943 |
| p99 Framezeit (ms) | 12.523 | 12.104 |
| Initialladen (s) | 1.964 | 1.846 |
| Peak RSS (KiB, externer Prozessmonitor) | 765016 | 728836 |
| Peak DRM VRAM (KiB) | 2279852 | 2278444 |
| Peak DRM GTT (KiB) | 69780 | 69780 |
| Geladene / gerenderte Sections | 2041 / 875 | 2041 / 875 |
| Mesh-Assets / sichtbare Mesh-Entities | 3487 / 1745 | 3487 / 1745 |
| Repraesentierte Props / Prop-Batches | 25808 / 328 | 25807 / 328 |
| Terrain-Kompositionen / summierte Workerzeit (s) | 885 / 0.810 | 894 / 0.757 |

Der Abschluss-Snapshot unterscheidet sich um ein Prop und neun asynchron
abgeschlossene Kompositionsjobs; keine exakt identische Arbeitsmenge behaupten.
Die Framezeiten sind CPU-Wall-Frames, keine GPU-Gesamtframezeit. Separate GPU-Pass-
Zeitreihen stehen in den Logs; vollstaendige Draw-/Subdrawzahlen wurden hier nicht
erfasst. Der Lauf enthaelt kein sichtbares Wasser und ist insbesondere kein
dichter Salzhoehlen-GPU-Benchmark. Ein Messpaar beweist weder eine allgemeine
FPS-Verbesserung noch Langzeit-/Multiplayer-/Multi-GPU-Paritaet.

Rohdaten: `before-forest.log`, `after-forest.log`; Messung mit
`tools/benchmarks/measure.py`, `TERRA_BENCH_SECONDS=60`,
`TERRA_RENDER_DIAGNOSTICS=1`, `TERRA_MEASURE_GPU_MEMORY=1`,
`TERRA_CONFIG_DIR=tools/benchmarks/results/crystal-clusters-2026-10-17/configs`.
Beide Builds liefen mit `env -u DISPLAY ALSA_CONFIG_PATH=/dev/null` und
`--smoke-test-forest`. Der bewusst fehlende Audioausgang verhindert einen lokalen
ALSA-POLLERR-Loop; Spiel-Audiooptionen und Audiocode wurden nicht geaendert.

Verworfene Vorversuche bleiben getrennt dokumentiert: `discarded-audio-errors.log.gz`
hat Millionen ALSA-Fehlerzeilen; `discarded-workarea-before.log` und
`discarded-workarea-after.log` haben unterschiedliche physische Fensterhoehen.
Diese Laeufe sind nicht Bestandteil der obigen Zahlen. Die finale 720er-Konfiguration
passt in die verfuegbare Desktop-Arbeitsflaeche und ergibt in beiden Logs 1920 x 1080.
