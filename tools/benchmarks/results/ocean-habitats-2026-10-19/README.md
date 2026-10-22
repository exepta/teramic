# Phase 79: Zusammenhaengende Meeresbereiche

## Umsetzung

Die bestehenden Ground-Props verwenden jetzt `OceanHabitat` im vorbereiteten
SurfaceColumn. Klima und ein 240-m-Feld mischen Coral Reef, Seagrass Forest
und Cold Ocean. Ein 21-m-Koloniefeld und breitere freie Kanaele bestimmen
zusammenhaengende Bestaende. Das alte unabhaengige 12x12-Riffgitter entfaellt.

Korallen bevorzugen warme, beleuchtete Felsruecken. Roehren stehen in dichten
Kolonien, Teller an geneigten Kanten; Anemonen an passenden Riffstellen.
Seegraswaelder stehen vorwiegend auf Sand, Clay und Mud. Seegurken und kleine
Steine besetzen lockerere Bodenbereiche. Kaltes Wasser erzeugt keine
tropischen Korallen. Uebergaenge der Habitatgewichte sind stetig, inklusive
negativer Koordinaten und Chunk-Grenzen. Spawnentscheidungen pro Pflanze
bleiben deterministisch; ein klarer Bestand ist kein regelmaessiges Raster.

Neue Welten erhalten `ocean_revision = 1`: mit dem Meeresboden verbundene
Riffvorspruenge, zusammenhaengende Stein-/Korallen-/Schlamm-/Kiesvorkommen
und Eisdecken mit offenen Wasserwegen. Eis reicht an derselben X/Z-Saeule
von seiner Unterseite bis zur Meeresoberflaeche; kein losgeloestes Haengeis.
Zwischen Grund und Eis bleibt Wasser frei. Seed, Generatorversion 5 und
alle bisherigen IDs bleiben erhalten. Alte gespeicherte Welten lesen das
fehlende Feld als 0 und behalten ihre bisherige Geometrie. Ihre generierten
Unterwasserprops verwenden trotzdem die verbesserte Verteilung.

Append-only Block 49 `teramic::ice`: eigener JSON-Eintrag, gemeinsame
Cube-Geometrie, solide Kollision und teilweise transparente 512px-Miptextur.
Kein GLB, kein eigener Renderer, keine Lichtemission, keine Glas-Holzrahmen.
Innere Eisfaces werden entfernt; benachbarte opake Flaechen bleiben sichtbar.
Eis ist platzierbar, abbaubar und ueber das bestehende Regionsjournal speicherbar.
Automatisches Schmelzen/Gefrieren und Rutschphysik sind nicht enthalten.

Seegras besitzt mehrstufige Halme mit versetzten, stumpfen Blattflaechen.
Vorlagen sind 2.15-3.35 m hoch, natuerliche Skalierung ergibt im Wald
mehrere Meter hohe Bestaende. Flache Wasserstellen und Eis begrenzen die
Hoehe. Die Wurzeln bleiben still, Spitzen schwingen sichtbar in einer
langsamen Wasserwelle. CPU-Picking und WGSL verwenden identische Bewegung;
Broadphase und Renderbounds beruecksichtigen deren volle Amplitude.

Die Anemone hat einen niedrigen weichen Fuss, einen Mundbereich und bis zu
28 versetzte, gekruemmte Tentakel mit blassen Spitzen. Die anderen
Korallenformen bleiben erhalten. Gemeinsame Templates und Section-Batches
bleiben bestehen; maximal 2400/800/160 Vertices pro Near/Middle/Far-Vorlage.
Es gibt keine neue Entity pro Pflanze oder Blatt. Unterwasser-Generierung
ueberspringt jetzt ungenutzte Landbewuchs-/Baumunterwuchs-Proben.

Pflanzenfreiheit wird ueber die gesamte Hoehe geprueft, meist anhand bereits
vorhandener gepaddeter Voxel. Ein in einen Halm gesetzter solider Block entfernt
nur betroffene Props, auch wenn die Wurzel in der Section darunter liegt.
Das vorhandene Tombstone-/Replikationssystem speichert diese Entfernung.
Aquatische Props koennen auch auf Korallenbloecken gepflanzt werden; Eis
ist kein Pflanzboden. Wasserquellen entfernen diese Pflanzen nicht.

Der Squid-Kopf wird in X/Z um 20% verkleinert; Y und Tentakel bleiben gleich.
Die gemeinsame meshbasierte Skalierung korrigiert auch Normalen.

Netzwerkprotokoll 44 wegen des erweiterten WorldConfig-Vertrags. Client
und Server gemeinsam neu starten. REG9/PLY3 bleiben unveraendert.

## Validierung

Gezielte Tests pruefen kontinuierliche Habitatfelder, vorhandene freie
Kanaele, alle sieben Pflanzenarten, hohe Seegrasbestaende, fehlende tropische
Korallen im kalten Meer, Eisverbindung und Wasserfreiheit, gepaddete versus
headless Sections, negative Nachbargrenzen, Editvorrang und alte Weltvertraege.
Die bestehenden Mesh-, Picking-, Interaktions-, Netzwerk- und Speichertests
werden im Workspace mit ausgefuehrt.

`--smoke-test-ocean-habitats` sucht drei echte generierte Bereiche und nimmt
sieben Ansichten auf. Dort werden keine Showcase-Props oder Bloecke gesetzt.
Der bestehende `--smoke-test-reef` prueft daneben gezielt einzelne Modelle,
alle sechs Unterwasserbloecke und die Unterwassereffekte mit expliziten Fixtures.
`TERRA_AQUATIC_INSPECT=1 --smoke-test-creatures` prueft den Squid im Modelllauf.

### Native Ansichten

Linux/Wayland, Vulkan RADV, RX 6950 XT, native 1920x1080. Echte Bevy-Fenster
und GPU-Screenshots, keine Web-Mockups. Alle folgenden Laeufe endeten mit
Exit 0; keine ERROR-/WARN-, WGSL- oder Slab-Allocator-Meldungen.

- `--smoke-test-ocean-habitats`: sieben Ansichten in 28.863 s, Peak-RSS
  885040 KiB, Peak-VRAM 2840300 KiB. Neue Weltvertraege mit Landschaft 1,
  Geologie 4, begrenzter Welt, Schluchten 1 und Ozean 1. Ohne platzierte
  Testobjekte wurden gefunden: Riff bei (5632.5, -2, -7903.5), Seegraswald
  bei (4672.5, -7, -8095.5) und kaltes Meer bei (4384.5, -16, -7327.5).
  Die geladenen Bereiche enthalten 6874 / 13454 / 5368 natuerliche Wasserprops
  bei 384 / 312 / 350 gesamten Prop-Batches. Im kalten Bereich wurden
  169948 Eisvoxel geladen, nicht ebenso viele Entities. Riffstufen,
  freier Boden, hohe Halme, Eisunterseite und offene Wasserwege wurden
  visuell kontrolliert. Zwei Bilder am selben Waldstandpunkt zeigen
  verschiedene Animationsphasen. Kein KI-/Multiplayer-Stresstest.
- `--smoke-test-reef`: 14 Modell-/Unterwasserbilder in 29.869 s, Exit 0,
  Peak-RSS 898080 KiB. Seegras und die neue Anemone wurden nah betrachtet;
  dieser Lauf verwendet explizit platzierte Props und Bloecke. Er lief vor
  der letzten Eis-Texturabstimmung und mit dem aelteren Landschaftsvertrag.
  Das danach frostigere, weniger glasige Eis ist in den finalen natuerlichen
  Habitatbildern geprueft. Die Tinten-/Wasserqualitaetspruefungen bleiben bestanden.
- `TERRA_AQUATIC_INSPECT=1 --smoke-test-creatures`: 13 Modell-/Posenansichten
  in 32.878 s, Peak-RSS 823708 KiB. Der schmalere Squid-Kopf wurde visuell
  kontrolliert, die Arme bleiben angeschlossen und unveraendert. Modellgalerie
  auf einer Testwiese, kein Nachweis von Schwimm-KI im Meer.

Der erste Habitatstart fand keine `server.toml` im isolierten Testordner;
diese Testkonfiguration wurde ergaenzt. Benutzerkonfigurationen und der
bereits laufende Benutzer-Server wurden nicht veraendert.

[Generiertes Riff](captures/habitat-0.png),
[Seegraswald](captures/habitat-2.png),
[zweite Bewegungsphase](captures/habitat-3.png),
[kalter Meeresboden](captures/habitat-4.png),
[Eisdecke und Wasserwege](captures/habitat-6.png),
[neue Anemone](captures/reef-05-sea-anemone.png),
[Squid-Kopf](captures/creatures-0.png).

### Vergleichsmessung

Ryzen 9 5900X, RX 6950 XT, 1920x1080, klare pausierte Welt um 10 Uhr,
Seed 42042, Sichtweite 8, vertikal 3, VSync aus, High-Wasser, keine
Chunkanimation. Je 30 s `--smoke-test-ocean`, identische Kamera und
`benchmark-configs/`. Fehlendes `ocean_revision` bedeutet in beiden
Laeufen den bisherigen Bodenvertrag: Geometrie der Welt bleibt vergleichbar,
Verteilung und Modellgeometrie der Props aendern sich absichtlich.
Das ist eine ruhende Ueberwasser-Meeresansicht, kein dichter Unterwasserwald.

Vorher: gesichertes Phase-78-Binary samt Asset-Snapshot unter `target/phase79/`.
Nachher: Phase-79-Code und aktuelle Assets. Keine gleichzeitigen eigenen
Cargo-/Client-Laeufe; Desktopprogramme und Benutzer-Server bleiben bestehen.
Kein isolierter Laborrechner und keine Aussage ueber andere GPUs.

| Messwert | Vorher | Nachher |
| --- | ---: | ---: |
| Durchschnitt FPS | 186.90 | 189.98 |
| p95 Frame, ms | 6.876 | 6.500 |
| p99 Frame, ms | 8.402 | 7.567 |
| Initiale Bereitschaft, s | 2.796 | 2.753 |
| Peak-RSS, KiB | 819688 | 827920 |
| Peak-VRAM, KiB | 1796940 | 1796940 |
| Peak-GTT, KiB | 69780 | 69780 |
| Geladene Sections | 2667 | 2667 |
| Gerenderte Sections | 615 | 615 |
| Meshassets | 3096 | 3107 |
| Repraesentierte Props | 1617 | 6263 |
| Prop-Batches | 284 | 295 |
| Terrain-Komposition, Worker-s | 1.061 | 1.054 |
| Prop-Komposition, Worker-s | 0.020 | 0.040 |
| GPU Opaque-Pass Mittel, ms | 0.456 | 0.448 |
| GPU Transmissive-Pass Mittel, ms | 1.555 | 1.566 |
| PBR Draw-API-Aufrufe, letzter Snapshot | 125 | 125 |

Das einzelne Paar zeigt keinen FPS-Einbruch in dieser Ansicht, aber rund
8 MiB mehr Peak-RSS und doppelte aufsummierte Prop-Kompositionsarbeit.
Es beweist weder allgemeine Beschleunigung noch Speicherparitaet. Workerzeiten
sind parallele Komposition, nicht reine CPU-Weltgenerierung; Bereitschaft
enthaelt Render-/Streamingarbeit. GPU-Passwerte sind keine gesamte GPU-Framezeit,
Draw-API-Aufrufe keine interne variable MultiDraw-Subdrawzahl.

Reproduktion: `tools/benchmarks/measure.py`, `TERRA_BENCH_SECONDS=30`,
`TERRA_RENDER_DIAGNOSTICS=1`, `TERRA_DRAW_DIAGNOSTICS=1`,
`TERRA_MEASURE_GPU_MEMORY=1`; `TERRA_CONFIG_DIR` auf `benchmark-configs/`
und getrennte `TERRA_MENU_CONFIG_DIR` setzen. Vorher zusaetzlich
`TERRA_ASSET_DIR` auf den alten Asset-Snapshot. Rohdaten in
`logs/ocean-before.log` und `logs/ocean-after.log`.

Zusaetzlicher Nachher-Lauf direkt im dichten Seegraswald, **kein Vorher-Paar**:
30 s mit `configs/`, `TERRA_HABITAT_BENCH=1`,
`--smoke-test-ocean-habitats --smoke-test-benchmark`. Der vorhandene
Benchmarkpfad uebernimmt Sampling/Ende, die Habitatprobe haelt nur den
echten Standpunkt. 182.76 FPS, p95 6.775 ms, p99 7.583 ms, Bereitschaft
2.936 s. 13454 dargestellte Props in 312 Batches, 4632 Meshassets,
2041 geladene / 1133 gerenderte Sections. Peak-RSS 802904 KiB,
Peak-VRAM 2847192 KiB. Terrain-Komposition 1.021 Worker-s,
Prop-Komposition 0.278 Worker-s. GPU Opaque-Pass 0.971 ms,
Transmissive-Pass 0.270 ms; 142 PBR-Draw-API-Aufrufe im letzten Snapshot.
Keine ERROR-/WARN-Meldungen. Diese Probe misst keine dauerhafte Bewegung,
keine Mehrspieler-KI und keine grossen Eisflaechen aus hoher Luftsicht.

### Automatisierte Tests

Alle vorgeschriebenen Cargo-Pruefungen mit unveraendertem Lockfile bestanden:

- `cargo fmt --all --check`
- `cargo check --locked --workspace`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo build --locked --workspace` fuer Client und Dedicated Server

`cargo test --locked --workspace --no-fail-fast`: **1249 bestanden,
0 fehlgeschlagen, 1 regulaer ignoriert**. Der ignorierte Test ist der
explizite 180-s-UDP-Soak, keine ausgelassene neue Meerespruefung.
Darunter 299 World-, 323 Render-, 105 Netzwerk- und 52 Speichertests.
Die World-Tests im Workspace enthalten auch die Bevy-Streamingtests.

Anfaengliche Fehler waren feste alte Material-/Soundanzahlen, eine falsche
Reihenfolgeannahme nach dem Prop-Reflow im neuen Reloadtest sowie der nach
visueller Abstimmung geaenderte Eis-Alpha-Wert. Der Reloadtest prueft jetzt
die echten Prop-IDs statt Vektorpositionen; keine Produktionsfunktion wurde
zum Umgehen eines Tests abgeschaltet.

Dedicated Server mit entfernten `DISPLAY`/`WAYLAND_DISPLAY`, eigenem Port
34579 und `--no-save --smoke-test`: Exit 0, 27 Sections bereit. Der normale
Dependency-Baum enthaelt weiterhin weder `terra-render`/`terra-ui` noch
`bevy_render`/`bevy_audio`/`wgpu`. Der bereits laufende Benutzer-Server blieb
unangetastet; Protokoll 44 wird dort erst nach bewusstem Neustart aktiv.

## Grenzen

Die Bilder leiten Form, Gruppierung und Farbe; keine pixelidentische Kopie.
Die Meeresausschnitte enthalten vorhandenes Terrain mit lokalen Riffvorspruengen,
keine neu erfundene globale Unterwasser-Arch-/Hoehlengenerierung. Anheften von
Korallen an senkrechte Waende und von Algen an Decken ist nicht implementiert.
Aktuelle Pflanzen bleiben am Boden und auf nach oben offenen Felsstufen.
Wasserbewegung ist eine visuelle Welle, keine physikalische Simulation jeder
Blattflaeche. Langzeitperformance auf weiteren GPUs bleibt separat zu pruefen.
