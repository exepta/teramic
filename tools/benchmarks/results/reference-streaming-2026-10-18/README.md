# Phase 65: Referenzmodelle Und Streaming

## Umfang

- Zwei profilgebundene Avatarvarianten, `--gender=f|m`, ueber denselben
  lokalen/entfernten Renderpfad und das bestehende Portraet gerendert.
- Neu modellierte 3D-Tierprodukte und Werkzeuge im gemeinsamen Itemcache.
- Ueberarbeitete kachelbare Terrain-/Laubmaterialien, Kies (42) und
  Strukturstein (43) als weitere normale Cube-Definitionen.
- Weniger ueberfluessige Cache-Invalidierungen und CPU-Arbeit im Streaming.

Bestehende Welt-/Block-IDs, Generatorversion, Regionen und Spielerdateien bleiben
erhalten. Netzwerkprotokoll 38 transportiert die Avatarvariante; Client und Server
gemeinsam aktualisieren. Keine Dependency-Aenderungen.

## Streamingkorrekturen

Das Entnehmen eines fertig generierten CPU-Meshes und Wasserpayloads markierte
bisher den gesamten `ChunkStore` als geaendert. Dadurch wurden unter anderem
Wasser-/Kollisions-/Renderabfragen erneut angestossen, obwohl sich kein Voxel
geaendert hatte. Diese rein internen Transfers verwenden nun gezielt
`bypass_change_detection`; echte Generierung, Edits und Entladung behalten ihre
Aenderungssignale.

Der Prop-Upload entfernte ueber `retain` selbst in ruhigen Frames nichts, markierte
die Renderregistry aber trotzdem geaendert und invalidierte so seinen eigenen
Idle-Fastpath. Er entfernt jetzt nur tatsaechlich entladene Eintraege.

Terrain-/Prop-Dispatcher waehlen nur die fuer freie Jobs benoetigte naechste
Teilmenge und sortieren diese. Veraltete Section-Epochen werden vor der Auswahl
ausgeschlossen, damit ein naher veralteter Kandidat keinen weiter entfernten
aktuellen Auftrag blockiert. Bei voller Terrainqueue entfaellt die erneute Suche.
Leere Sections ueberspringen die Suche nach Custom-Modellen.

Die lokale Lichtauswahl leiht Blockdaten ohne Journal-Overrides statt sie zu
kopieren. Ungeaenderte leere Emittercaches benoetigen keine Nachbarabfragen.
Nachbarwechsel pruefen nur bekannte Lichtquellen; neue Sections bleiben budgetiert,
direkte Platzierung/Entfernung und geaenderte Epochen werden weiterhin verarbeitet.

## Messbedingungen

Linux/KDE Wayland, AMD RX 6950 XT, RADV Mesa 26.2.4, Vulkan. Optimierter Dev-Build,
1280x800 logische Fensteraufloesung, 150 Prozent DPI (1920x1200 physisch), VSync aus.
Seed 42042, Tageszeit 10:00 pausiert, Sichtweite 16, vertikaler Radius 3.
`--smoke-test-lod` bewegt die Renderkamera zweimal 224 m hin und zurueck, einschliesslich
negativer Koordinaten. Der 60-s-Lauf entspricht etwa 14.9 m/s, der 24-s-Lauf 37.3 m/s.
Keine Builds oder weiteren von uns gestarteten Clients parallel zu Messlaeufen.
Bestehende Benutzerprogramme und der bereits laufende Dedicated Server blieben an.

Die Vergleichslaeufe verwenden dieselbe vor Aenderungen gesicherte Assetkopie.
Damit werden Codekosten von den neuen Texturen getrennt. Ein separater Lauf mit
aktuellen Assets wird entsprechend gekennzeichnet. Initiale Bereitschaft ist
nicht in den FPS-/Perzentilwerten enthalten; Peak-RSS/VRAM schliessen sie ein.

Dies ist ein kurzer nativer Renderer-Streamingtest, kein mehrstuendiger
Integrated-/Multiplayer-Flugtest und kein Nachweis fuer andere GPUs. Die berichteten
Einbrueche auf unter 30 FPS wurden in dieser Ausgangsszene nicht in dieser Staerke
reproduziert. Deshalb wird keine vollstaendige Behebung aller Flugspitzen behauptet.

## Gemessener Codevergleich

| Messwert | 60 s vorher | 60 s nachher | 24 s vorher | 24 s nachher |
| --- | ---: | ---: | ---: | ---: |
| Durchschnitt FPS | 110.41 | 124.88 | 90.16 | 105.09 |
| p95 Frame ms | 14.444 | 11.566 | 18.173 | 12.868 |
| p99 Frame ms | 21.273 | 14.066 | 24.997 | 14.551 |
| Schlechtester Frame ms | 42.260 | 117.337 | 38.669 | 25.413 |
| Erste Bereitschaft s | 5.228 | 5.249 | 5.251 | 5.277 |
| Peak RSS KiB | 1249496 | 1204960 | 1264016 | 1181180 |
| Peak VRAM KiB | 3725256 | 3984416 | 3567612 | 3566020 |
| Terrain-Kompositionen | 35605 | 36260 | 26447 | 27989 |
| Terrain-Workerzeit gesamt s | 15.825 | 15.784 | 14.236 | 14.518 |
| Prop-Kompositionen | 6188 | 6219 | 6160 | 6158 |
| Prop-Workerzeit gesamt s | 2.421 | 2.353 | 2.375 | 2.317 |

Alle vier Laeufe beenden die Strecke mit 7809 geladenen/4118 gerenderten Sections,
6351 sichtbaren Mesh-Entities und 110304 logisch dargestellten Props in 1289 Batches.
Mesh-Assets 10777 -> 10783; die sechs zusaetzlichen Handles gehoeren zum zweiten
geteilten Avatar. GPU-Passzeiten stehen einzeln in den Rohlogs, nicht als falsche
Summe einer vollstaendigen GPU-Framezeit. Draw-Submissions wurden hier nicht aktiviert.

Die schnellere Strecke verbessert p99 und den schlechtesten Frame. Im 60-s-Lauf
verbessern sich die Perzentile, aber ein einzelner 117.337-ms-Frame bei 24.588 s
bleibt auffaellig. Seine vorherige Frameprobe hatte keine Jobs oder Uploads;
das beweist weder einen GPU-Stall noch eine OS-Ursache. Der VRAM-Peak steigt dort
um 253 MiB. Keine vollstaendige Spitzen- oder Speicherparitaet behaupten.
Worker-Summen messen parallel geleistete Komposition, nicht CPU-Generierungswallzeit;
eine neue isolierte reine Weltgenerierungsbaseline wurde nicht erhoben.

Ein zusaetzlicher 24-s-Diagnoselauf mit `TERRA_CPU_SCOPES=1` erreicht 105.41 FPS,
p99 15.012 ms. Einschliesslich Start: Terrain-Dispatch maximal 8.205 ms (ein Aufruf
ueber 8 ms), Lichtauswahl 5.007 ms, Prop-Dispatch 6.768 ms, Terrain-Upload 1.298 ms,
Prop-Upload 2.052 ms. Timer sind in den vier Vergleichslaeufen deaktiviert.
Das hilft bei weiteren Untersuchungen, ist kein vollstaendiger CPU-Frameprofiler.

Der abschliessende 60-s-Lauf mit den **neuen Assets** erreicht 122.42 FPS,
p95 11.666 ms, p99 13.993 ms, schlechtester Frame 25.015 ms. Erste Bereitschaft
5.515 s, Peak-RSS 1167908 KiB, Peak-VRAM 3722276 KiB. Sections, sichtbare Meshes
und Props entsprechen weiterhin dem Vergleich. Das ist ein eigener kompletter
Art-/Code-Lauf, kein isolierter Beweis fuer Texturkosten. Er zeigt keinen zweiten
117-ms-Ausreisser, widerlegt den vorher gemessenen Ausreisser aber nicht.

## Native Ansichten

Die Unterordner `avatars`, `items`, `hands` und `blocks` enthalten echte Bevy/Vulkan-Aufnahmen,
keine generierten Mockups. Die Rendererfixtures verwenden die produktiven
Modellgeneratoren/Materialien. Sie ersetzen keine vollstaendige Inventar-Spielabnahme.
Einzelansichten der Referenz definieren nicht alle Rueckseiten oder Animationen;
eine pixelidentische 1:1-Uebereinstimmung wird nicht behauptet.

Waehrend der Pruefung korrigiert:

- Der Avatar-Smoke-Pfad benoetigte selbst das ausgewaehlte Profilresource.
- Screenshot-Farbpruefung wertete BGRA faelschlich als RGBA aus; jetzt Bevy-Konvertierung
  und Formatregression statt Entfernen der Inhalts-/Clippingpruefung.
- Runtime-Resize des Itemstudios kollidierte unter 150-Prozent-DPI mit dessen MSAA-
  Depthtarget. Das Studio behaelt die Startaufloesung und verwendet MSAA Off.
- Materialfixture bei sehr grosser Sichtweite lief in ihr Zeitlimit. Die isolierte
  radius8-Konfiguration ohne Eintrittsanimation prueft alle 43 GPU-Vorschauen,
  gesetzten Bloecke und verbundenes Glas erfolgreich.

Finale Ansichten: zweimal fuenf Avatarbilder mit erkanntem Blinzeln und Bewegung
aller vier Gliedmassen, fuenf Itemstudioansichten, sieben Materialansichten.
Zusaetzlich bestehen fuenf Held-Item-Ansichten (Block, Pickaxe, Schwert, Schlag,
Third Person). Die Pickaxe und der Third-Person-Griff wurden angesehen. Der erste
kalte Held-Item-Lauf waehrend der Testkompilierung erfasste zu frueh ein schwarzes
Bild; der unveraenderte Wiederholungslauf bestand. Die feste Drei-Sekunden-Wartezeit
dieser aelteren Fixture ist weiterhin kein verlaesslicher GPU-Pipeline-Ready-Test.

Einzelberichte: [Charaktere](CHARACTER.md), [Items](ITEMS.md), [Bloecke](BLOCKS.md).

## Validierung

Erfolgreich auf dem finalen Quellstand:

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo test --workspace --no-fail-fast`: **991 bestanden**, kein Fehler;
  ein bestehender dreiminuetiger UDP-Soak bleibt ignoriert.
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --workspace`
- Headless-Server mit entferntem `DISPLAY`/`WAYLAND_DISPLAY`: 27 Sections und
  erfolgreicher Exit. Der bereits laufende Benutzerserver wurde nicht beendet.
- Native GPU-Ansichten wie oben beschrieben; falsches `--gender=x` wird vor
  Fenster-/GPU-Initialisierung abgelehnt.

Regressionen pruefen alte Profile, persistierte Auswahl, echte UDP-Avatarreplikation,
Mesh-Indizes/Normals/Bounds, gemeinsame Materialien, geschlossene Blockhuellen,
negative Koordinaten, LOD-Eviction, fehlende Idle-Selbstinvalidierung und Priorisierung
bei einer nahen veralteten Section. Die breite Axtspitze wurde nach ihrer Neigung
gleichmaessig in die Einheitsgrenzen eingepasst; der Test ist jetzt strenger als zuvor.
Eine alte PNG-Anzahlannahme wurde durch genaue Dateinamen plus prozedurale
Materialzuordnungen ersetzt, nicht ersatzlos entfernt.

## Reproduktion

```bash
cargo run --locked -p terra-client -- --gender=f
cargo run --locked -p terra-client -- --profile tester-b --gender=m
TERRA_CAPTURE_DIR=/tmp/terra-items target/debug/terra-client --smoke-test-items
TERRA_SCREENSHOT=/tmp/avatar.png target/debug/terra-client --gender=f --smoke-test-avatar
TERRA_SCREENSHOT=/tmp/materials.png target/debug/terra-client --smoke-test-materials
TERRA_BENCH_SECONDS=60 TERRA_RENDER_DIAGNOSTICS=1 TERRA_MEASURE_GPU_MEMORY=1 \
  python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-lod
```

`TERRA_CPU_SCOPES=1` ergaenzt optionale CPU-Scope-Summen am Benchmarkende.
Sie schliessen den Start ein; verschachtelte Spannen sind nicht addierbar.
Normales Gameplay startet keine Scope-Uhren und schreibt keine Scope-Logs.
