# Phase 57: Umgebung, Schritte, Spuren, Hoehlen Und Landschaft

## Implementierung

Der vorhandene aufgenommene Soundscape-Mixer wurde erweitert, nicht ersetzt.
Vogel-Feldaufnahmen am Tag und Grillen bei Nacht werden nach Feuchte und
Forest-/Plains-/Mushroom-Bewuchs gewichtet. Geladene Hoehlendecken verdraengen
die Aussenkulisse. Die alten Wind-/Laubdateien sind archiviert, aber nicht mehr
im Lade-/Wiedergabebank. Vorhandenes Wasser-Audio bleibt separat erhalten.

Sechs Untergruende besitzen je zwei echte Aufnahmen: Gras/Laub-Foley, Sand,
Stein, Erde/Kies-Foley, Schnee und Holz. Die Materialzuordnung verwendet die
stabilen bestehenden Block-IDs; Erze klingen wie Stein, Planken/Logs wie Holz.
Quellen und CC0-Lizenzen: [Credits](../../../../assets/audio/CREDITS.md).
Reproduzierbare Bearbeitung: `tools/prepare-footstep-audio.sh`.
Keine Minecraft-/Mod-/Hytale-Dateien und keine neuen Dependencies.

Kontakte folgen der gelaufenen horizontalen Strecke, nicht FPS oder einem
abstrakten Geh-Timer. Flug, Wasser, fehlender Boden und Teleports werden
ausgeschlossen. Lokale PlayerBody-Kontakte und interpolierte Serverpositionen
von Mitspielern/Tieren verwenden dieselben Messages. Darauf reagieren Audio
und Renderbatches unabhaengig; ein fehlender Audioausgang verhindert keine
Spuren. Die kosmetischen Kontaktzeitpunkte werden nicht als neue autoritative
Netzwerkereignisse repliziert und koennen zwischen Clients leicht abweichen.

Ein gedaempftes Netzwerk mit acht Delay-Linien erzeugt den Hoehlen-Schritthall.
Es alloziert einmal pro Audiostream, nicht pro Schritt. Hallauslaeufe werden
weiter gemischt, Live-Mute und Unterwasserfilter wirken auch darauf. Dies ist
ein begrenzter algorithmischer Hall, kein Raytracing oder mod-eigenes Sample.
Der aktuelle Raumfaktor folgt dem Spielerkoerper; komplexe akustische Portale
zwischen getrennten Raeumen sind nicht simuliert.

## Spuren

| Untergrund | Spieler | Landtiere |
| --- | ---: | ---: |
| Sand | 10 s | 3 s |
| Schnee | 22 s | 5 s |

Eine einzige Mesh-Entity und ein geteiltes Material verwalten maximal 512
Kontakte. Aktualisierung/Fading mit maximal 20 Hz, maximal 8192 Vertices.
Bei Budgetueberschreitung werden die aeltesten Kontakte frueher verworfen.
Spieler zeigen getrennte Fersen-/Vorfussformen, Tiere gespaltene Hufpaare.
Die letzten 35 Prozent der Lebenszeit blenden aus. Bodenabbau, Ueberbau,
Unload und Sessionende verwerfen die betroffenen Spuren. Die Darstellung
liegt auf horizontalen vollen Blockoberseiten; keine echten Voxelvertiefungen,
Verformung von Spezialmodellen oder persistente Speicherung.

## Hoehlen

Ein gemeinsamer begrenzter Loaded-Cell-Deckenclassifier wird von Audio und
Renderer verwendet. Blattkronen und unbekannte Zellen werden nicht als echte
Hoehlendecken behandelt. Der Renderer prueft bis 64 m nach oben an neun Punkten
alle 250 ms, der Audioclassifier alle 500 ms. Die visuelle Umgebung folgt der
Kamera, der Audiohoerer weiterhin dem Spielerkoerper.

Himmels-Environmentlicht wird in Hoehlen reduziert, Ambientfuellung und
Belichtung bleiben lesbar, Nebel und Farbtemperatur wechseln weich. Der neue
`terra_caves.wgsl`-Pass rekonstruiert die Entfernung aus bestehender Prepass-Tiefe:
kuehlere Schatten, erhaltene warme Highlights und dezente Entfernungshaze.
Sky-Pixel bleiben unangetastet. Er ist vor Tonemapping eingeordnet und ausserhalb
von Hoehlen uebersprungen. Varianten fuer HDR/LDR und MSAA-/nicht-MSAA-Tiefe
verwenden denselben Code. Kein neuer Shadow-/Depth-Pass, keine GI oder echte
volumetrische Lichtstreuung; Hytale ist Stilrichtung, kein kopierter Renderer.

## Terrain Und Kompatibilitaet

Neue Welten verwenden den gespeicherten Landschaftsvertrag
`landscape_revision = 1`. Fehlendes Feld alter TOML-Konfigurationen ergibt 0;
kein automatischer Retrogen, keine Aenderung existierender Seeds/IDs und keine
Generatorversionsaenderung. Die Weltanlage setzt den neuen Vertrag auch bei
einer alten globalen Vorlage. Explizites nachtraegliches Umschalten existierender
Welten veraendert das Grundterrain und ist kein unterstuetzter Retrogen.

- Continentalness: vorhandenes Kontinentfeld erneut verwendet, kein zweites
  konkurrierendes Klimasystem.
- Erosion: eigenes ruhiges Feld reduziert Relief in abgetragenen Regionen.
- Peaks & Valleys: gefaltetes Niederfrequenzfeld fuer zusammenhaengende Ruecken/Taeler.
- Jaggedness: kleine raue Anteile nur an wenig erodierten Hochlandruecken.
- Surface Lake: eigenes Eignungsfeld und deterministische lokale Becken.

Die Felder werden in der horizontalen Spaltenvorbereitung verwendet, nicht pro
Voxel. Neue Seen bleiben in ihren 256-m-Kandidatenzellen; begrenzte Caches
speichern auch verworfene Kandidaten. Die vollstaendige nasse Beckenflaeche
wird auf Hoehenunterschiede und Konflikte mit Fluss/Oase geprueft. Geschlossene
Ufer und ein lokal konstanter Wasserstand verwenden vorhandene OasisSample-
Beckenformen und dieselben Wasserzellen/Meshes/Edits wie andere Wasserflaechen.
Keine unterirdischen Seen oder zusaetzlichen vertikalen Wasserintervalle.

WorldConfig ist Netzwerkbestandteil: Client und Server benoetigen gemeinsam
Protokoll 31. REG7/PLY3 und bestehende binaere Regions-/Spielerdaten bleiben
unveraendert. Der Dedicated Server bekommt keine Render-/Audio-Abhaengigkeiten.

## Validierung

907 Workspace-Tests bestanden, ein vorhandener expliziter 180-s-UDP-Soak bleibt
ignoriert. `CARGO_BUILD_JOBS=2`, `RUST_TEST_THREADS=1`. Alle fuenf geforderten
Pruefungen sind erfolgreich; die finalen Protokolle liegen neben diesem Bericht:

```sh
cargo fmt --all --check
cargo check --locked --workspace
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace
```

Abgedeckt sind Framerate-unabhaengige Kontakte, echte geladene Sand-Unterstuetzung,
Flug-/Teleportunterdrueckung, sechs getrennte Materialaufnahmen, Decoderenergie
aller 26 geladenen Clips, Hallimpuls/Decay, begrenzte Mixerwerte und Live-Mute.
Renderer-Tests pruefen Spurenindizes/Fade, Hoehlen-/Wasserkomposition und gueltige
Nebelintervalle auch bei kurzer Sichtweite/dichtem Nebel. Welt-Tests pruefen
deterministische neue Felder, geschlossene Seen samt Wasserstandsvertrag,
alte exakte Hoehenreferenzen mit Revision 0 und Grass/Dirt-Schichten mit beiden
Revisionen. Weltanlage und Dedicated-Neustart pruefen den gespeicherten Vertrag.
Ein Zwischenlauf traf eine belegte zufaellige TCP-Testadresse; der abschliessende
vollstaendige serielle Testlauf war erfolgreich.

Native Vulkan-Nachweise auf AMD Radeon RX 6950 XT / RADV, Mesa 26.2.4:

- `tracks/tracks-0.png`: Sohlen-/Hufspuren auf Sand und Schnee sichtbar.
  `tracks-1.png`: nach elf Sekunden nur noch Spieler-Schneespuren. Die Fixture
  prueft vier Kontaktgruppen und leere Daten nach 22.5 s (`native-tracks.log`).
  Sie injiziert bewusst Kontakt-Messages, keine echte Spielereingabe; die
  Kontaktproduktion ist separat mit geladenem Boden getestet.
- `natural-caves-msaa/`, `natural-caves-no-msaa/`: natuerliche Cheese-, Dripstone-,
  Spaghetti- und Salzhoehlenszenen, ohne das sonst verwendete Inspektionslicht.
  Jede Hoehlenaufnahme wartet auf einen aktiven Hoehlenfaktor; Logs zeigen 1.0.
  Standard-MSAA und `TERRA_CAVE_MSAA=off` verwenden ihre jeweiligen Tiefenpfade.
  Die finale Aufnahme ohne MSAA wurde nach dem zusaetzlichen kurzen Nebelguard
  wiederholt. Mehrere Bilder wurden visuell geprueft; kein Shader-/Renderfehler
  in diesen erfolgreichen Laeufen.
- `headless.log`: Server ohne DISPLAY/WAYLAND_DISPLAY und ohne Renderer gestartet,
  27 Sections erzeugt, sauber beendet. Der vorhandene Benutzer-Server blieb an.

Die nativen Starts verwenden `ALSA_CONFIG_PATH=/dev/null`, da hier kein
verlaesslicher Audioausgang verfuegbar ist. Keine akustische Abnahme behaupten;
Modstil/Hallmischung sind menschlich abzustimmen. Eine historische fest
koordinierte Giant-Cave-Fixture fand keinen passenden freien Standort. Der
erste Portalversuch entdeckte einen GPU-Upload leerer Spurenmeshes. Beides liegt
unter `diagnostic-attempts/`; die GPU-Ursache ist durch ein verborgenes gueltiges
Startmesh und Verbergen nach Ablauf behoben. Aktuelle Naturkammer-Fixtures
verwenden dynamisch gefundene Standorte, nicht injizierte Terrainbloecke.
Oberflaechenseen sind funktional, aber nicht separat nativ als neue Szene
abgenommen. Hytale-Referenzqualitaet oder volle GI wird nicht behauptet.

## CPU-Vergleich Der Landschaftsvertraege

Dasselbe aktuelle Worker-Binary, zuerst Revision 0, dann 1; kein Compiler oder
nativer Testclient parallel. Seed 42042, Zentrum Section (0,6,1), Radius 12/3,
vier Threads, ein Warmup und fuenf Messungen je Modus, 4375 Sections.

```sh
TERRA_BENCH_LANDSCAPE=0 TERRA_BENCH_CENTER=0,6,1 target/debug/examples/worker_benchmark
TERRA_BENCH_LANDSCAPE=1 TERRA_BENCH_CENTER=0,6,1 target/debug/examples/worker_benchmark
```

| Mittelwert / Geometrie | Revision 0 | Revision 1 |
| --- | ---: | ---: |
| Reine Generierung, Wall (s) | 0.331023 | 0.343923 |
| Render-Vorbereitung, Wall (s) | 0.583060 | 0.589530 |
| Generierte Props | 43952 | 53286 |
| Terrain-Quads | 617067 | 570294 |

Rohdaten: `workers-legacy.log`, `workers-landscape.log`. Rund 3.9 Prozent mehr
Generierungszeit und 1.1 Prozent mehr Render-Vorbereitung in diesem Gebiet.
Die neue Landschaft besitzt andere Hoehen, Vegetation und Blockmengen; dies
ist kein isolierter Noise-Mikrobenchmark. Render-Vorbereitung enthaelt auch
gepolsterte Voxels, Props, Wasser und Greedy-Meshing. Kein ECS-Pacing,
Audiogeraet, Netzwerk, GPU-Upload oder persistentes Schreiben enthalten. Die
historische 2.6-s-ECS-Baseline ist damit nicht direkt vergleichbar.

## Nativer Waldvergleich

Je ein 60-s-Lauf: vor Arbeitsbeginn vorhandenes Phase-56-Binary als
`target/debug/terra57-before`, danach das neue Client-Binary. Identische
Konfigurationen aus `configs/`, alte Landschaftsrevision 0, Seed 42042, pausierte
Zeit 10:00, klares Wetter, Sichtweite 8, VSync aus, physisch 1920 x 1080.
Kein Compiler oder anderer Testclient parallel; Benutzer-Desktop und bestehender
Server bleiben aktiv. Nach der Messung wurde nur der Nebelguard fuer kurze
Hoehlen-Sichtweiten ergaenzt; dessen Zweig ist in dieser Aussenansicht inaktiv.

```sh
env -u DISPLAY ALSA_CONFIG_PATH=/dev/null \
  TERRA_CONFIG_DIR=tools/benchmarks/results/soundscape-landscape-2026-10-18/configs \
  TERRA_BENCH_SECONDS=60 TERRA_RENDER_DIAGNOSTICS=1 TERRA_MEASURE_GPU_MEMORY=1 \
  python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-forest
```

| Messwert | Phase 56 | Phase 57 |
| --- | ---: | ---: |
| Durchschnittliche FPS | 210.89 | 211.20 |
| p95 Framezeit (ms) | 5.962 | 5.948 |
| p99 Framezeit (ms) | 7.009 | 7.092 |
| Initiale Ladezeit (s) | 1.847 | 1.962 |
| Abgetastetes Peak-RSS (MiB) | 770.96 | 755.02 |
| Abgetastetes DRM-Peak-VRAM (MiB) | 1966.14 | 1969.14 |
| Abgetastetes DRM-Peak-GTT (MiB) | 68.14 | 68.14 |
| GPU Opaque-Pass Mittel (ms) | 1.027 | 1.031 |
| GPU Early-Prepass Mittel (ms) | 0.346 | 0.345 |
| Terrain-Komposition, Worker gesamt (s) | 0.779 | 0.778 |
| Geladene / dargestellte Sections | 2041 / 875 | 2041 / 875 |
| Mesh-Assets / sichtbare Mesh-Entities | 3511 / 1745 | 3512 / 1745 |
| Repraesentierte Props / Prop-Batches | 25808 / 328 | 25808 / 328 |

Rohdaten: `before-forest.log`, `after-forest.log`. Ein zusaetzliches verborgenes
Startmesh gehoert zum gemeinsamen Spurenbatch. Die Szene besitzt keine
replizierte Tierherde oder aktiven Spuren und testet nicht die Kosten des aktiven
Hoehlenpasses. Nachher werden 26 statt 14 Aufnahmen decodiert; ohne Audiogeraet
ist dies kein Echtzeit-Audioausgabetest. Keine Streaming-/Mehrspieler-/Mehr-GPU-
Langzeitabnahme und keine FPS-/Speicherparitaet aus diesen Stichproben ableiten.
Insbesondere RSS-Abtastung und Treiber-Caching erlauben keinen belegten
Speichergewinn. GPU-Passwerte sind keine vollstaendige GPU-Framezeit;
Draw-/Subdrawzahlen und 512 gleichzeitige Spuren wurden nicht isoliert profiliert.
