# Landschaften Und Streaming (Phase 137)

## Stand

F40 ist implementiert und die CPU-/Streaming-Vergleiche sind gemessen.
Die nachfolgenden Zahlen zeigen **keinen grossen allgemeinen FPS-Gewinn**.
Ein Abschluss des seltenen Low-FPS-Problems wird nicht behauptet. Native
Landschaftsaufnahmen, Sichtpruefung und Headless-Test sind unten dokumentiert.

Keine reduzierte Sichtweite, Vegetationsdichte oder Wasserqualitaet und keine
neue Cave-, Wasser- oder Strukturengine als Ersatz fuer bestehende Systeme.

## F40 Landschaftsregionen

`generation/landscape_regions.rs` erweitert die bestehende Hoehenberechnung
fuer `world.landscape_revision = 2`. Die neue Formgebung liegt vor der vorhandenen
Ocean-, River-, Oasis- und Lake-Bearbeitung. Strukturen verwenden weiterhin
dieselben Terrain-/SurfaceColumn-Abfragen fuer Standorte und Fundamente.

Die fuenf kontinuierlich gemischten Landformen sind:

| Landform | Geometrie |
|---|---|
| Plains | Ruhige Ebenen mit stark reduziertem lokalem Relief |
| Hills | Breite, wechselnde Huegel und zusaetzliche weichere Ruecken |
| Plateau | Erhoehte Tafelflaechen und abgestufte, konfigurierbare Terrassen |
| Ridges | Zusammenhaengende Kamm-/Rueckenformen mit biomeabhaengiger Hoehe |
| Basin | Breite, tiefere Senken mit begrenzter Unterkante |

Ein grossskaliges, seedbasiertes und raeumlich verzerrtes Noise-Feld verteilt
die Regionen. Die Gewichte bilden eine glatte Partition, keine Auswahl je
Chunk oder Section. Dadurch koennen mehrere Chunks dieselbe Form tragen und
Nachbarregionen ohne kuenstliche Rasterkante ineinander uebergehen. Die
Skalen liegen im Bereich mehrerer hundert Meter bis grob eines Kilometers;
dies sind keine garantierten kreisfoermigen Regionsdurchmesser.

Seltene zusaetzliche Fels-/Mesa-Uplifts liegen innerhalb von Plateau- und
Ridge-Regionen. Sie veraendern das durchgehende Hoehenfeld und bleiben mit
dem Untergrund verbunden. Keine separaten schwebenden Inseln oder Entities
je Landform. Biomelevation und Hochland-Grundhoehe bleiben beteiligt;
Hill-/Mountain-Amplituden und `terrace_strength` steuern weiterhin das Relief.
Der Senken-Floor darf niedrige Referenzhoehen nicht kuenstlich bis ueber den
Meeresspiegel anheben, auch bei sehr kleinen Amplituden.

Das neue Regionsmodell verwendet fuenf feste zusaetzliche 2D-Noise-Abfragen
pro Hoehenabfrage. Section-Jobs nutzen weiterhin vorbereitete Saeulen.
Keine Nachbarschaftssuche oder neue Regionsgenerierung pro Voxel, kein neuer
Regionscache und kein weiterer Worker-/Streamingpfad. Die Zusatzkosten sind
nicht null und muessen im CPU-Vergleich separat ausgewiesen werden.

Diagnose-API: `TerraGenerator::landscape_region(x, z)` liefert
`LandscapeRegionSample { weights, landmark }`; `dominant()` dient nur der
Inspektion. Der Generator mischt alle fuenf Gewichte statt eines dominanten
Labels. Diese Diagnosefelder werden fuer Revision 0/1 nicht angewendet.

## Gespeicherte Welten

Neue Welten verwenden Landschaftsrevision 2. Ein fehlendes Feld in alten
Dateien bleibt Revision 0; gespeicherte Revisionen 0 und 1 werden nicht auf
2 angehoben. Die bisherigen Berechnungszweige bleiben fuer beide erhalten.
Der Dedicated-Resolver bevorzugt die gespeicherte Revision gegenueber einer
neueren globalen Vorlage. Welt-ID und bestehende Block-IDs bleiben erhalten.

`WorldConfig::same_terrain` beruecksichtigt die Landschaftsrevision.
PreparedColumn-Daten einer anderen Revision duerfen deshalb nicht fuer neue
Sections wiederverwendet werden. Die explizite Revision ist Bestandteil des
bestehenden Generatorkontrakts, keine automatische Retrogen-/Chunkmigration.
Wer eine bestehende Welt testen will, darf ihr Revisionsfeld nicht manuell
auf 2 umstellen; dafuer eine neue separate Testwelt anlegen.

## Terrain- Und Prop-Handoff

`meshing/upload.rs`, `props/jobs.rs` und `meshing/staging.rs` behalten bereits
vorbereitete Materialbatches im jeweiligen Job. Ein grosses Section-Ergebnis
wird ueber mehrere Frames vorbereitet, statt die ganze Section auf einmal
als unteilbare Ueberschreitung zuzulassen.

Die bestehende Upload-Admission trennt Section-Slots und Vertexkontingente:
Terrain nutzt bis zu 128.000, Props bis zu 96.000 zugelassene Vertices je
Frame-Domaene. Client-Limits begrenzen weiterhin die Anzahl der Ersetzungen.
Ein einzelner unteilbarer Materialbatch darf sein Kontingent allein
ueberschreiten, damit grosse Ergebnisse nicht fuer immer blockieren.
Dies sind Admission-Zaehler, keine gemessenen GPU-Uploadzeiten oder Bytebudgets.

Bis alle Batches bereit sind, bleibt die vorherige sichtbare Section erhalten.
Erst die vollstaendige, weiterhin gueltige Ersetzung publiziert den neuen Root
und entfernt den alten im gemeinsamen Command-Commit. Staging selbst erzeugt
keine sichtbare halbe Section. Bereits vorbereitete Meshes werden nicht noch
einmal vorbereitet; veraltete, abgebrochene oder entladene Ergebnisse geben
ihre privaten Staging-Assets wieder frei. Terrain- und Prop-Epochs, LOD und
Appearance-Revisionen werden vor dem Publizieren erneut beruecksichtigt.

Der vorhandene Fade-/First-Draw-Pfad bleibt Teil des Eintritts. Eine neuere
ungueltige Komposition darf einen gerade einblendenden, gleichwertigen Root
nicht erneut ersetzen. Crop-Growth beginnt erst beim abschliessenden Handoff,
nicht schon waehrend statische Materialbatches noch warten.

## Auswahl Und Warteschlangen

Die vorhandenen Terrain-Auswahlsweeps behalten nach der initialen Ladephase einen stabilen
Snapshot, statt beim naechsten Flug-/Fokuswechsel den verbleibenden Rest zu
verwerfen. Diese Auswahl hat ein Kandidatenlimit und ein weiches Zeitlimit;
neue Aenderungen fordern einen nachfolgenden Durchlauf an. Die initiale
Bereitschaft wird getrennt behandelt. Spieler-Edits bleiben priorisierbar.

Bei Props werden die in einem begrenzten Auswahldurchlauf bearbeiteten
Section-Positionen gesammelt. Der Dispatch-Backlog wird einmal gegen diese
Menge bereinigt und danach mit den neuen Ergebnissen erweitert. Kein erneuter
vollstaendiger Queue-Scan fuer jede einzelne ausgewaehlte Section.
Deduplication und die bestehenden LOD-/Epoch-Pruefungen bleiben erhalten.

## Dorfbewohner Auf Kanten

`creatures/villagers.rs` behandelt den Fall, in dem ein Koerper physisch auf
einer schmalen Kante steht, die strengere Navigationsprobe aber keinen
Startboden findet. Ohne Ziel wurde zuvor der Stalled-Route-Ausweg nie betreten.
Der ziellose Bewohner kann nun einen lokalen, geprueften Ausweg erhalten.
Sowohl erster Schritt als auch gesamter Weg werden mit der bestehenden
Koerper-/Kollisionspruefung abgesichert. Kein Teleport und kein Durchlaufen
von Moebeln oder Waenden als Ausweg. Die normale Routenlogik wird weiterbenutzt.

## Reproduktion Und Testumfang

Das bestehende Beispiel `terrain_regions` unterstuetzt:

```bash
target/debug/examples/terrain_regions --seed=42042 --revision=2 --landforms
target/debug/examples/terrain_regions --seed=71 --revision=2 --landforms
target/debug/examples/terrain_regions --seed=918273 --revision=2 --landforms
target/debug/examples/terrain_regions --seed=42042 --benchmark
```

`--landforms` sucht trockene starke Anker und gibt deren Koordinaten, Hoehen,
lokalen Hoehenbereich und Regionsgewichte aus. `--benchmark` vergleicht
Revision 1/2 bei sonst gleicher Konfiguration auf denselben positiven und
negativen Koordinaten: aufeinanderfolgende Warm-Cache-Messrunden fuer
`surface_height`, getrennt vollstaendige `surface_column`-Abfragen, mit
`black_box`, Pruefsummen, Zeit und Abfragen pro Sekunde. Kein Ersatz fuer
Section-Generierungs-, Meshing-, Streaming- oder native FPS-Messungen.

Angelegte Regressionen pruefen unter anderem Regionsdeterminismus,
negative Koordinaten, stetige Section-Grenzen, Formvielfalt, seltene
geclusterte Landmarken, unterschiedliche Makrogeometrie, Plateau-Steuerung,
kleine Amplituden und Oberflaechenschichten fuer Revision 0/1/2. Historische
Hoehen stammen aus dem vor der Aenderung gestarteten Revision-1-Programm;
deren Ausgabe hatte neun Nachkommastellen, nicht die Genauigkeit eines
vollstaendigen bitweisen World-Snapshots. Historische Terrace-/Cave-Vertraege
bleiben explizite Revision-1-Fixtures; Revision 2 hat zusaetzliche passende
Lush-/Luftvolumenpruefungen. Staging-Tests behandeln Wiederverwendung,
atomare Ersetzung, Abbruch/Unload, LOD-/Edit-Invalidierung und grosse Batches.
Der vollstaendige Workspace-Lauf besteht mit 2582 Tests und sieben ignorierten
Langzeit-/Spezialtests. Formatierung, All-Targets-Check, striktes Clippy und
Client-/Server-/Diagnosebeispiel-Build bestehen ebenfalls. Rohlogs liegen hier.

## Native FPS-Vergleiche

Ryzen 9 5900X, RX 6950 XT, Vulkan/RADV, Mesa 26.2.5, optimiertes Dev-Profil.
VSync aus, Sichtweite 20, hohe Schatten-/Wasser-/Wetterqualitaet. Keine
parallelen Compiler, Tests oder CPU-Benchmarks waehrend der nativen Laeufe.
Ausgangsbinary vor den Aenderungen: `/tmp/teramic137-baseline-client`.

Beide Versionen verwenden dieselben unveraenderten Landschaftsrevision-0-
Konfigurationen. F40 veraendert diese Szenen daher nicht. Der Hauptlauf hat
einen integrierten Server, echtes HUD/Minimap, echte Spielereingaben, Regen,
1920x1080 und 60 Sekunden Explorationsroute. Der zweite Seed 7 verwendet die
separate Offline-Renderer-Hin-/Rueckflugfixture, Regen, 1280x800 und 24 Sekunden.
Diese beiden unterschiedlichen Szenarien sind **nicht untereinander** A/B-
vergleichbar. Keine CPU-Probes oder Render-Timestamp-Instrumentierung im Paar.

| Szene / Stand | Mittel FPS | Min FPS | p95 ms | p99 ms | Schlechtester Frame ms | Laden s | RSS KiB |
|---|---:|---:|---:|---:|---:|---:|---:|
| Integrated vorher | 82.05 | 22.90 | 20.39 | 26.40 | 43.67 | 36.25 | 2317868 |
| Integrated nachher | 82.82 | 24.38 | 19.70 | 26.19 | 41.02 | 36.25 | 2366516 |
| Wasser vorher | 73.97 | 28.06 | 23.93 | 26.75 | 35.64 | 23.61 | 1602616 |
| Wasser nachher | 73.30 | 25.44 | 23.84 | 25.83 | 39.31 | 23.49 | 1723876 |

Der Hauptlauf verbessert sich nur gering; beim Wasserlauf sinkt p99, aber
der schlechteste Frame wird schlechter und die mittlere FPS bleibt praktisch
gleich. Einzelne kurze Paare belegen weder eine generelle Low-FPS-Loesung noch
eine abgeschlossene Langzeitstabilitaet. A05 bleibt offen. Keine neuen
10-/15-/30-Minuten-Laeufe. Ladezeit ist hier praktisch unveraendert.

Der Hauptlauf endet bei 13606 geladenen Sections, acht schattenwerfenden
Punktlichtern, 142271 geladenen Props und 22 Wasser-Tiles. Asynchron sichtbar:
5591/5596 Sections, 140382/140663 Props, 13364/13381 Mesh-Assets.
Der Wasserlauf endet beidseits bei exakt 15831 geladenen / 3756 dargestellten
Sections, 12989 Mesh-Assets, 27122 Props und 1681 Wasser-Tiles, ohne Punktlichter.
Szenenlasten sind vergleichbar, die asynchronen Hauptlauf-Enden nicht pixelidentisch.

Die maximale Admission einschliesslich Ladephase sinkt im Hauptlauf nur von
411061 auf 402968 Vertices, beim Wasserlauf von 60744 auf 56840. Ein einzelner
grosser Materialbatch darf weiterhin allein das Kontingent ueberschreiten.
Dies ist **keine harte globale Obergrenze** und kein gemessener GPU-Byteupload.
Der schlimmste alte Hauptlauf-Frame hatte zuvor null Uploads und null offene
Jobs. Staging wird daher nicht als Ursache oder Loesung aller Spikes ausgegeben.

Die geprueften Near-Schattenproxies werden nicht pauschal entfernt: Ihre
ersetzten Farbbatches tragen `NotShadowCaster`, sie sind also keine belegte
Doppelverschattung. Ausserdem bleiben sie waehrend des Chunk-Fades wirksam.
Eine ungepruefte Entfernung wuerde Schattenloecher oder mehr Microgeometrie
im Schattenpass riskieren.

### Reproduktion

Hauptlauf: `config/` als `TERRA_CONFIG_DIR`, frische getrennte temporaere
`TERRA_MENU_CONFIG_DIR` und `TERRA_WORLD_DIR`:

```text
TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 TERRA_BENCH_GRAPHICS=1
TERRA_BENCH_SECONDS=60 TERRA_BENCH_WEATHER=rain TERRA_BENCH_LOAD_SECONDS=180
TERRA_SOAK_ROUTE=explore WGPU_BACKEND=vulkan
target/debug/terra-client --smoke-test-benchmark
```

Wasserlauf: `config-water/`, `TERRA_BENCH_GRAPHICS=1`,
`TERRA_BENCH_SECONDS=24`, `TERRA_BENCH_WEATHER=rain`,
`TERRA_BENCH_LOAD_SECONDS=240`, `--smoke-test-lod`, ohne Integrated-/HUD-Flags.

## CPU-Kosten Von F40

Gleiche Koordinaten und sonst gleiche Config je Revision, ohne Renderer oder
parallelen Compiler. Je 458752 Hoehenabfragen und 1792 volle SurfaceColumn-
Abfragen in sieben Warm-Cache-Runden. Das sind keine kompletten PreparedColumn-
Jobs, Chunk-Meshing-Zeiten oder GPU-Framezeiten. Andere Reliefs beeinflussen
auch die nachgeschaltete Hoehlen-/Hydrologiearbeit der SurfaceColumn-Abfrage.

| Seed | Hoehenzeit Rev1 / Rev2 ms | Mehrzeit Hoehe | Mehrzeit SurfaceColumn |
|---|---:|---:|---:|
| 42042 | 454.686 / 529.344 | 16.42 % | 11.92 % |
| 71 | 454.110 / 523.698 | 15.32 % | 11.35 % |
| 918273 | 450.743 / 532.011 | 18.03 % | 4.10 % |

Beim Seed 42042 benoetigen 1792 Oberflaechenabfragen 9.091 / 10.175 ms.
F40 hat somit reale Zusatzkosten; eine kostenfreie Weltverbesserung wird
nicht behauptet. Die fuenf zusaetzlichen Noise-Abfragen bleiben auf der
Saeulen-/Hoehenebene statt pro Voxel.

## Zusaetzlicher Profiling-Lauf

Ein separater 60-s-Lauf aktiviert `TERRA_RENDER_DIAGNOSTICS=1`,
`TERRA_CPU_SCOPES=1`, `TERRA_STALL_TRACE=1` und
`TERRA_SOAK_REPORT_SECONDS=30`. Seine 77.67 FPS sind wegen der Instrumentierung
kein weiterer sauberer A/B-Wert. Raw-Log: `teramic137-profile.log`.

Im berichteten Messfenster: CPU-Main 11.469 ms, Render 10.214 ms je Wall-Frame.
Die Threads arbeiten parallel; diese Werte werden **nicht addiert**.
Exakte Body-Probes liegen im Mittel bei ca. 0.964 ms MeshAllocator,
0.861 ms Preprocess-BindGroups und 0.897 ms Terrain-Dispatch. Terrain-Upload
liegt bei 0.456 ms, Prop-Upload bei 0.112 ms. Fenster-/View-Target-Vorbereitung
liegen hier nur bei ca. 0.054 / 0.008 ms. Die 61-ms-Allokationsspitze liegt
vor dem Ladeabschluss, nicht in der folgenden FPS-Stichprobe.

GPU-Timestamps zeigen fuer den Haupt-Opaque-Pass 2.157 ms im Mittel
(p99 2.778 ms), fuer den fruehen Prepass 1.186 ms (p99 1.693 ms).
Die ausgegebenen Vertex-Invocations liegen im Mittel bei ca. 8.45 / 7.96 Mio.
Das sind einzelne instrumentierte Paesse, keine rekonstruierte gesamte
GPU-Framezeit. Insbesondere sind 0.021 ms `per_view_shadow_pass` keine
Summe saemtlicher Schattenansichten.

Der Messlauf lokalisiert weitere relevante Arbeit, liefert aber keinen
einzigen dominanten, bereits behobenen Grund fuer alle FPS-Ausreisser.
Enge naechste Versuche waeren korrekt begrenzte BindGroup-Wiederverwendung
bei stabilen Buffer-Identitaeten oder zuerst die Messung exakt doppelter
Vertexdaten im Worker. Dynamische Dispatchgroessen duerfen nicht eingefroren
werden; ein Nutzen der Vertex-Deduplizierung ist noch nicht nachgewiesen.

## Native Landschaftsaufnahme

Sechs normale generierte Regionen, je zwei Ansichten, mit Landschaftsrevision 2.
Keine gesetzten Testbloecke oder veraenderten Benutzerwelten. Die Probe prueft
reale Boden-/Luftzellen sowie Terrain-, Prop- und Wasserbereitschaft.
Aufnahmeaufloesung 1280x800, Sichtweite 8 und vertikale Spanne 4; das sind
Framing-Einstellungen der Fixture, nicht die FPS-Vergleichseinstellungen.
Alle sechs Fixtures enden erfolgreich; Uebersichten sowie Plateau-/Landmark-
Detailansichten wurden tatsaechlich betrachtet. Bilder unter `captures/`.

| Ansicht | Seed | Anker x / y / z |
|---|---:|---|
| Plains | 42042 | 2944.5 / 24.925 / -4095.5 |
| Plateau | 42042 | -3711.5 / 63.692 / -3071.5 |
| Landmark | 42042 | -447.5 / 181.318 / 64.5 |
| Hills | 71 | -2559.5 / 28.700 / 1920.5 |
| Basin | 71 | -767.5 / 14.701 / -4095.5 |
| Ridges | 918273 | 768.5 / 126.092 / 64.5 |

Die Fixture behaelt den Config-Seed und die explizite Landschaftsrevision.
`TERRA_BIOME_SCENE=landscape-plateau --smoke-test-biomes` waehlt die Region;
optional erzwingt `TERRA_LANDSCAPE_ANCHOR='[-447.5,64.5]'` einen gueltigen
Anker. Die Konfiguration `config-landscape/` ist fuer Seed 42042 gespeichert;
fuer die anderen Ansichten den Seed in einer separaten Kopie anpassen.
`TERRA_BENCH_GRAPHICS=1`, Vulkan und ein separates `TERRA_CAPTURE_DIR` verwenden.

Erste starre Kameraposen verdeckten einen Kamm durch den Vorderhang.
Die reine Testkamera sucht deshalb eine freie Terrain-Sichtlinie; in einer
umschlossenen Senke faellt sie auf eine begrenzte Draufsicht zurueck. Sie hebt
die erlaubte vertikale Sichtweite nicht ueber das Clientlimit an. Der angepasste
Gallery-Regressionstest besteht nach dieser Fixture-Aenderung gesondert.
Nebel, Baeume und echte Wolken begrenzen einzelne Ansichten, insbesondere die
hohe verschneite Landmarke. Kein behaupteter Komplettvergleich aller Biome
oder aller moeglichen Struktur-/Wasseranschluesse.

Headless: `env -u DISPLAY -u WAYLAND_DISPLAY ... terra-server --no-save
--bind 127.0.0.1:0 --smoke-test` besteht mit derselben Revision 2 und Seed 918273.
Keine Fenster-/GPU-Abhaengigkeit fuer die neue Generierung.

## Grenzen

F41 und F42 bleiben offen: Diese Aenderung liefert weder systematisch groessere
verbundene Seen noch ein neues System fuer wesentlich laengere Fluesse.
Vorhandene Fluesse, Baeche, Seen, Wasserfaelle, Cave-Verbindungen und
Struktur-Fundamente passen sich weiterhin an das Terrain an; ihre bestehenden
Algorithmen wurden nicht durch neue Engines ersetzt. Die Kontur einer
Oberflaechenregion ist keine garantierte begehbare Strecke oder freie Dorfparzelle.

Staging begrenzt die laufende Arbeit, beseitigt aber weder die Kosten eines
einzelnen sehr grossen Materialbatches noch alle CPU-/Treiber-/GPU-Stalls.
Die Messungen oben belegen keinen grossen allgemeinen FPS-Fix. Die native
Sichtpruefung ist eine begrenzte Stichprobe; weitere Landschafts-/Struktur-
Kombinationen bleiben Teil der allgemeinen Grafikabnahme A01.
