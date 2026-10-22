# Phase 34: Chunk-Animationen, LOD Und Renderkosten

16.10.2026, RX 6950 XT / RADV Mesa 26.2.4 / Vulkan / KDE Wayland.
Rust-/Dependency-Versionen, Generator 5, Weltformat 2, Protokoll 14,
Seed sowie Welt-/Block-IDs bleiben erhalten. Keine neue Prop-/Block-Pipeline.

## Umgesetzt

- Chunk-Eingang wird zuerst `Visibility::Hidden` eingereiht. Materialalpha bleibt
  null, bis der konkrete Farb-Renderbefehl erfolgreich ist. Erst danach laeuft
  die Animation. Ein laufender Eingang wird nicht mehr durch kosmetische
  LOD-Replacements ueberschrieben; echte Edits bleiben unmittelbar.
  Noch unsichtbar eingereihte Batches duerfen ihre LOD dagegen aktualisieren
  und werden wieder unsichtbar eingereiht. Nur die aktive Animation wird
  geschuetzt; sonst blieben zuvor verdeckte Fern-LODs beim Annaehern eingefroren.
- Warteschlange statt Instant-Fallback bei vollen 96 Plaetzen, acht neue Wurzeln
  pro Frame. Tatsaechliche Mesh-AABBs und Bevy-VisibilityRanges bestimmen die
  Sichtbarkeit, nicht nur eine grosszuegige Section-Kugel. Nicht gezeichnete
  Kandidaten geben ihren Platz wieder frei. Unsichtbare Ausgaenge brauchen
  keine Blend-Materialien; sichtbare Ausgaenge werden priorisiert.
- Optionen: Instant unveraendert als Default, Fade weiterhin 300 ms, Slow Fade 750 ms,
  Bounce 1000 ms mit Ueberschwingen, Rise und Glide je 900 ms. Bewegung bleibt
  rein visuell und endet exakt auf dem urspruenglichen Section-Transform.
  Die Uhr verbraucht hoechstens 1/30 s pro Frame. Kamera-/Chunk-Kollision bleibt
  auf unveraenderten logischen Zellen.
- Nur waehrend der Animation werden temporaere Materialien und eine eindeutige
  ungebatchte Draw-Zuordnung benoetigt. Anschliessend werden Originalhandles,
  Batching und vorherige Schattenflags wiederhergestellt. Keine Micro-Entities.
  Wartende schattenfaehige Batches benutzen ausschliesslich Sonnenlayer 62:
  unsichtbar fuer die Farbkamera, aber weiterhin echte Schattenwerfer. Keine
  zusaetzliche Entity; vorherige Layer/Sichtbarkeit werden exakt restauriert.
- Terrain-LOD und Blockreliefdistanz sind 50 Prozent groesser, einschliesslich
  Hysterese. Basis 96/160/12 ergibt effektiv 144/240/18 m; gespeicherte relative
  Praeferenzen bleiben erhalten. Blockrelief initial 48 m, Hysterese 54/42 m.
  Prop-Distanzen 80/144 m bleiben getrennt. Kein Material-/Generatorwechsel.
- Identische Eckpunkte innerhalb verbundener Quads werden geteilt; Dreiecke,
  Winding, harte Normalen und UVs bleiben gleich. Kein globales Welding ueber
  Material-/Texturkanten. Windlaub packt Normalen/Farben auf Workern: 28 statt
  48 Vertexbytes, unveraenderte Positionen/UVs/Indizes, Masken und Wind bleiben.
  HDR-Farbdecodierung ist layoutabhaengig, Float-Vorschauen/Abbauteile bleiben
  korrekt. Alte Float-Kapazitaeten werden vor dem Uploadwarten freigegeben.
- LOD-Scans verwenden horizontale Meter-Buckets statt subpixelgenauer
  Kamerabewegungen. Edits/Materialwechsel/abgeschlossene Eingaenge invalidieren
  weiter. Die Animationskandidaten werden maximal alle 100 ms neu sortiert;
  unveraenderte Fade-Transforms loesen keine laufenden Mesh-Transformupdates aus.
- Haze-Huelle mit aktueller linearer Fog-Farbe entfernt den harten planetaren
  Hintergrundstreifen. Tiefengeprueft hinter Terrain/Wasser, ohne Schatten oder
  Tiefenschreiben. Nebel aus schaltet sie ab; Unterwasserfarbe wird uebernommen.
  Nebel oberhalb 100 Prozent verschiebt seinen Beginn weiter nach hinten,
  endet aber immer vor der endlichen Weltkante. Kein toter Sliderbereich.
- Optionaler echter Wasserfall-Volumennebel ueber Bevy, kein eigener Renderer:
  `water_volumetric_mist`, Default false, nur High, maximal vier reale nahe
  Aufprallbereiche bis 40 m, 24 Schritte und geteilte weiche 16x16x16-Dichte.
  Low/Medium, fehlende Quellen und Aus entfernen Volumes/Kameraeffekt/Volumenlicht.

## Diagnose Und Messbedingungen

`TERRA_DRAW_DIAGNOSTICS=1` zaehlt erfolgreiche direkte/indirekte PBR-API-Aufrufe
sowie UI-Nodes/Text, Slicer und UI-Schatten. `TERRA_GPU_DRAW_DIAGNOSTICS=1`
liest zusaetzlich alle 60 Frames die echten indirekten Farbkommandos aus:
Opaque/Mask/Transparent/Transmission, GPU-Count getrennt von nichtleeren
Kommandos, eigene Frame-ID. Maximal ein asynchroner 8-MiB-Readback; kein Warten
auf die GPU im Renderthread. COPY_SRC und Readbacks haben eigene Diagnosekosten.
Prepass-/Shadow-Subdraws und alle Fullscreen-/Gradient-/Custom-UI-Draws werden
nicht als vollstaendig erfasst behauptet. Wiederverwendete Early-/Late-Buffers
lassen sich nicht korrekt aus einem einzigen End-of-frame-Snapshot ableiten.

Der Renderer deklariert die ohnehin im Client vorhandene Bevy-UI-Renderfunktion
nun explizit fuer diese Zaehler, damit isolierte Renderer-Tests nicht zufaellig
von Workspace-Feature-Unification abhaengen. Kein neues UI-Framework, keine
neue Paketversion; der Server haengt weiterhin nicht von terra-render ab.

`measure.py` meldet Peak-RSS/CPU-Zeit. Optional liest
`TERRA_MEASURE_GPU_MEMORY=1` alle 250 ms die DRM-FD-Infos des Kindprozesses,
dedupliziert DRM-Clients und meldet Peak-VRAM/GTT-Allokationen. Nicht verfuegbare
Treiberwerte bleiben null/None, nicht als Nullverbrauch ausgegeben.
VRAM-Allokation ist weder RSS noch ein Gesamtwert des gesamten Desktops.

Native Frames: physisch 1920x1200 aus logischen 1280x800 bei 150 Prozent,
VSync aus, FOV 90, Seed 42042, Baumdichte 0.8, vertikale Sichtweite 3.
Wald-/Pilzparkblick bei -622/53.88851/680, Streaming-Endblick
-622/58.73574/680. Die Region enthaelt einen echten dichten Mushroom Park,
keine handplatzierten Benchmark-Pilze. 32er-Laeufe laden am Ende 31868 Sections,
20418 Terrainwurzeln und 312135 nicht entfernte logische Props.

60-Sekunden-Standbilder und 120-Sekunden-Fluege werden getrennt ausgewiesen.
Der Flug absolviert zweimal hin/zurueck ueber 224 m. `TERRA_BENCH_SECONDS`
erlaubt nun bis 1800 s; laengere Fluege wiederholen die Strecke entsprechend.
Die Messung wartet anfangs auf sichtbare fertig eingeblendete Batches, am Ende
auch auf deren Beruhigung. Nur das eigentliche Messfenster liefert FPS/p95/p99;
Peak-RSS umfasst den ganzen Prozess. Screenshotnahe Frames werden ausgelassen.
Zwischen Aufnahme und Messung 500 ms Beruhigung. Keine parallelen eigenen
Build-/Test-/GPU-Laeufe im Messfenster. Desktop-Hintergrundlast bleibt real.

`TERRA_BENCH_LEGACY_LOD=1` und `TERRA_BENCH_DISABLE_HORIZON=1` sind nur fuer
Smoke-Fixtures wirksame Vergleichshilfen, keine Grafik-/Gameplayoptionen.
Sie halten beim technischen Vorher-/Nachherpaar die bisherigen Distanzen und
den bisherigen Hintergrund konstant. Das neue 50-Prozent-LOD/Haze-Bild wird
getrennt gemessen, nicht als gleiche Geometrie ausgegeben.
`TERRA_BENCH_UNPACKED_FOLIAGE=1` erlaubt denselben Wind-/Maskenpfad mit alten
Float-Vertices. Es schaltet weder Blaetter noch Pflanzen ab.

## Bereits Erfasste Zwischenwerte

| Lauf | FPS | p95 / p99 ms | Peak-RSS KiB |
| --- | ---: | ---: | ---: |
| Phase 33, dichter Pilzpark, 60 s | 150.39 | 8.506 / 10.022 | 894464 |
| Erste Pack-/Quadoptimierung, gleiche LODs, 60 s | 161.13 | 7.418 / 8.225 | 902380 |
| Phase 33, 32er-Fade-Flug, 120 s | 40.66 | 31.735 / 35.600 | 2457404 |
| Phase 34 vor Kandidatencache, 32er-Fade, 120 s | 31.82 | 39.857 / 44.417 | 2467504 |

Der letzte Zwischenlauf ist eine echte Regression, kein Erfolgswert. Dabei
warteten am Ende noch 1270 Kandidaten im Sichtbereich; 11125 statt 14762
Mesh-Entities waren sichtbar. Das ist keine Bild-/Workload-Paritaet.
Er motivierte Kandidatencache, unveraenderte Transforms und sofortige Freigabe
unsichtbarer Ausgaenge. Die abschliessenden Messungen stehen getrennt darunter.
Die groessten Zwischenlauf-Frames (bis 64.35 ms) traten auch mit null
Uploadvertices und null Generierungsjobs auf; reine Vertexuploadbudgets erklaeren
sie daher nicht. Der neue begrenzte Top-12-Bericht liefert den jeweils
vorhergehenden Job-/Upload-/Animationskontext, keine erfundene GPU-Stallursache.

Ein weiterer Zwischenschritt mit Kandidatencache erreichte 34.07 FPS,
p95/p99 39.737/46.049 ms, Peak-RSS 2436784 KiB. Ein fuenfsekundiger eigener
Binary-Hashcheck lief in diesem Messfenster; dies ist kein sauberer isolierter
Endvergleich. Ausserdem fehlten bei den vorherigen Fade-Staenden dauerhafte
Offscreen-Schatten, weil ausserhalb der Kamera wartende Wurzeln ganz verborgen
blieben. Dieser echte Qualitaetsfehler wurde durch den separaten Schattenlayer
behoben; die Zwischenlaeufe werden nicht als Bildparitaet verkauft.
Die anfangs auf 650/1250 ms verlaengerten Fadezeiten wurden nach Messung wieder
auf die bisherigen 300/750 ms gestellt. Renderbestaetigter Start, unsichtbare
Warteschlange und Mindestanzahl Animationsschritte bleiben erhalten.

Der erste alte 32er-Lauf hatte einen eigenen Cargo-Check in den ersten elf
Startsekunden, beendet lange vor dem 120-s-Messfenster. Seine initiale Ladezeit
ist deshalb kein sauberer isolierter Ladezeitvergleich.
Fruehe abgebrochene/fehlgeschlagene Animationstests und Streaminglaeufe bleiben
als solche im Logordner; insbesondere der Sichtbarkeits-Timeout wird nicht als
Benchmark gewertet. Die alte CPU-Uhr bzw. fehlende echte Draw-Rueckmeldung,
Schatten-/Frustum-Sichtbarkeit und separate Prop-Reichweiten wurden gezielt
auseinandergehalten, statt unsichtbare Batches nach einer Frist voll einzublenden.

## Abschliessende Messungen

| Lauf | FPS | p95 / p99 ms | Peak-RSS KiB | Peak-DRM-VRAM KiB | Initial bereit s |
| --- | ---: | ---: | ---: | ---: | ---: |
| Phase 33, erneuter ruhiger 32er-Fade-Flug, 120 s | 34.40 | 38.939 / 74.878 | 2378696 | 4424552 | 35.967 |
| Phase 34, korrigierter 300-ms-Fade, 120 s | 30.47 | 44.914 / 62.584 | 2375380 | 4762456 | 38.590 |
| Phase 34, neues 50-Prozent-LOD, Instant, 120 s | 35.78 | 35.030 / 40.380 | 2464868 | 4982296 | 33.511 |

Die ersten beiden Fluege verwenden die bisherigen LOD-Distanzen fuer den
technischen Vergleich. Der letzte verwendet bewusst die neuen, teureren
Qualitaetsdistanzen und ist kein Vergleich mit identischer Geometrie.
Alle drei nutzen DRM-Speichersampling, Render-/Drawdiagnosen, keinen Screenshot
im Flug und denselben Pfad. Die alten und neuen Fades sind funktional nicht
bildgleich: zuvor konnte die CPU-Uhr die Animation vor dem ersten sichtbaren
Draw verbrauchen. Phase 34 animiert wirklich; die alte Abkuerzung wird nicht
als gleichwertiger billiger Fade wieder eingefuehrt.

Der finale Fade-Flug ist rund 11.4 Prozent langsamer als der erneute alte Lauf,
bei besserem p99, aber schlechterem p95. Peak-VRAM liegt rund 330 MiB hoeher.
Am Ende sind alle sichtbaren Eingaenge abgeschlossen: active=0,
queued_in_view=0; 14820 ausserhalb der Sicht wartende Wurzeln werden nicht
vorzeitig eingefaedet. Sichtbare Mesh-Entities 14306 gegen 14762 im alten Lauf
sind ausdruecklich keine vollstaendige Bildparitaet. Logische Sections/Props
bleiben identisch. Der neue Flug vollzieht 15518 LOD-Wechsel.

Terrain-Komposition, gesamte Workerzeit: 9.768 -> 9.695 s bei 60727 -> 59309
Jobs; groesster Job 10.953 -> 15.759 ms. Prop-Komposition 3.220 -> 3.321 s
bei 10858 -> 10844 Jobs. Diese Summen parallel laufender Worker sind keine
Initialladezeiten. Der mittlere instrumentierte Opaque-GPU-Pass sinkt nur von
2.239 auf 2.182 ms, nicht die ganze GPU-Framezeit. Im neuen Flug treten trotzdem
einzelne CPU-Wallframes bis rund 91 ms auf, auch ohne Vertexuploads. Die
Ursache der verbleibenden Spitzen ist nicht vollstaendig nachgewiesen.

Auch die Wiederholung des alten Binaries variiert deutlich: 40.66 -> 34.40 FPS.
Deshalb keine allgemeine FPS-, p99- oder Speicherparitaet aus Einzelpaaren
ableiten. Die Fade-/32er-Restkosten bleiben offen, statt sie wegzudokumentieren.

### Isoliertes Windlaub-Paar

Gleicher neuer Renderer, gleiche Positionen, Indizes, Masken, Wind und LODs;
nur Float- gegen gepacktes Vertexlayout. 60 s Standblick, je ein Screenshot
vor dem ausgeschlossenen Beruhigungsfenster, DRM-Sampling in beiden Laeufen.

| Layout | FPS | p95 / p99 ms | Peak-RSS KiB | Peak-DRM-VRAM KiB | Opaque GPU ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Float, 48 Bytes/Vertex | 114.34 | 16.878 / 18.040 | 931436 | 2811980 | 1.238 |
| Gepackt, 28 Bytes/Vertex | 150.54 | 7.986 / 9.403 | 920264 | 2206660 | 1.223 |

Peak-VRAM 2746.07 -> 2154.94 MiB: 591.13 MiB beziehungsweise 21.5 Prozent
weniger in diesem Paar. Peak-RSS sinkt um 10.91 MiB. Beide haben 7809 geladene
Sections, 4118 Terrainwurzeln, 9688 Meshassets, 4596 sichtbare Mesh-Entities,
92784 logische Props, 1306 Propbatches und 5053016 mittlere Opaque-Vertexaufrufe.
Die grosse FPS-Differenz wird nicht vollstaendig dem Packing zugeschrieben:
der gemessene GPU-Pass aendert sich nur wenig, und es gibt reale Laufvarianz.
Dies widerlegt keine historische Gesamt-RSS-Regression gegen maskenloses Laub.

### GPU-Subdraw-Nachweis

Native kurze Standblicke liefern echte ausgelesene Farb-Subdraws, unread=0:
Wald Frame 601: Opaque/Mask/Transparent/Transmission 2762/134/1/0,
57980 gelesene Bytes; Ocean Frame 721: 1966/0/1/370, 46748 Bytes.
Submitted und nichtleere Kommandos stimmen in diesen Samples ueberein.
Der zuletzt gemeldete CPU-API-Zaehler gehoert nicht zwingend zur selben
Frame-ID; die beiden Werte werden nicht zu einem Gesamtframe vermischt.
Die kurzen Ocean-/Horizontblicke liefen im normalen fokussierungsabhaengigen
Winit-Modus und sind Sicht-/Shaderpruefungen, keine FPS-Benchmarks.

## Native Nachweise

- `--smoke-test-animations`: echte generierte Terrain-Section, unsichtbarer
  Wartestatus, Alpha null vor Farb-Draw, sichtbare Teilalpha und volle
  Wiederherstellung; FadeSlow, Bounce, Rise und Glide. Vier native Aufnahmen.
- `TERRA_RIVER_MIST=1 --smoke-test-rivers`: realer generierter Wasserfall,
  drei aktive Volumes, begrenzter Spruehbatch, Low schaltet beides ab, High
  stellt wieder her, Tauchen/Auftauchen restaurieren Fog/Environment. Native
  Shader/Vulkan-Ausfuehrung ohne Fehler. Keine Planarspiegelung behauptet.
- Ausgewaehlte Aufnahmen unter `captures/` wurden tatsaechlich geoeffnet,
  darunter Animation/Horizont und Wasserfall. Die Sichtabnahme ist technisch,
  keine gemeinsam mit dem Spieler abgeschlossene Art-Direction-Abnahme.
- `--smoke-test-menu-session`: echte TiltUI-Seiten/Dropdowns, Create World,
  Grafik-/Controlskonfiguration, Display-Rollback, Integrated-Server-Sitzung,
  autoritativer Edit, Save, Hostneustart und Journal-Reload bestanden. Native
  Optionsaufnahme unter `captures/menu-graphics.png`.
- Separat gebauter Dedicated Server startet mit entfernter DISPLAY- und
  WAYLAND_DISPLAY-Variable, erzeugt 27 Sections und beendet den Smoke-Test
  sauber. Sein normaler Dependency-Baum enthaelt weiterhin weder wgpu noch
  bevy_render, bevy_ui, TiltUI oder terra-render.

## Tests Und Reproduktion

Alle fuenf vorgeschriebenen Pruefungen am abschliessenden Quellstand bestanden:

```bash
cargo fmt --all --check
CARGO_BUILD_JOBS=2 cargo check --workspace
CARGO_BUILD_JOBS=2 cargo test --workspace
CARGO_BUILD_JOBS=2 cargo clippy --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=2 cargo build --workspace
python3 -m unittest discover -s tools/benchmarks -p test_measure.py
```

592 Rust-Tests bestanden, ein vorhandener Test ignoriert; ein Python-Test
bestanden. Darunter Material-/LOD-Seams, negative Koordinaten/Hysterese,
Queue-/Renderfeedback/Restaurierung, versteckte Terrain-/Prop-LOD-Wechsel,
gepackte Attribute, Quad-Indizes, Mist-Qualitaet und Konfigurations-Roundtrips.
Der abschliessende native Animationstest prueft wieder die finalen 300/750 ms,
nicht die zwischenzeitlich laengeren Fadezeiten. Build/Test-Logs liegen lokal
unter `logs/`; die vorhandene Gitignore-Regel fuer Logs bleibt unveraendert.

Beispiel fuer den technischen 32er-Fade-Flug, aus dem Repository-Root:

```bash
env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/rendering-streaming-2026-10-16/configs32 \
  TERRA_MENU_CONFIG_DIR=/tmp/terra-rendering-benchmark-menu \
  TERRA_BENCH_SECONDS=120 TERRA_BENCH_LEGACY_LOD=1 \
  TERRA_BENCH_DISABLE_HORIZON=1 TERRA_MEASURE_GPU_MEMORY=1 \
  TERRA_RENDER_DIAGNOSTICS=1 TERRA_DRAW_DIAGNOSTICS=1 \
  python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-lod
```

Fuer die Produktions-LOD-Messung die beiden Vergleichshilfen weglassen und
`configs32-instant` verwenden. Fuer lange Fluege `TERRA_BENCH_SECONDS` bis 1800
erhoehen; das ist eine verfuegbare Messfunktion, kein bereits erfolgter Soak.
Fuer das Laubpaar den 16er-Konfigurationssatz unter
`../fluid-recession-mushrooms-2026-10-16/configs`, `--smoke-test-forest`, 60 s
und dieselben Vergleichshilfen verwenden; nur im Floatlauf
`TERRA_BENCH_UNPACKED_FOLIAGE=1` setzen. Beide mit identischem Screenshotablauf
starten. Produktive Benutzerkonfigurationen wurden nicht ueberschrieben.

Binary-SHA256, erst ausserhalb aller Messfenster erfasst:

```text
Phase 33: 8dff2cd72055bce917b8629d6f9d3558eb04042579d855a4c17ddefb903d9744
Final:    d3b801d4db87f516cf72ce82c67bc7e66ece0b6b69cd427c6cb27b812401178a
```

Das isolierte Laubpaar wurde vor den letzten Queue-/Fadezeitkorrekturen
aufgenommen. Es verwendet Instant; diese nachfolgenden Aenderungen greifen
in diesem Pfad nicht. Der finale 300-ms-Flug und Animationstest verwenden
den abschliessenden Binary-Stand.

## Verbleibende Grenzen

Nur eine physische GPU verfuegbar. Keine Mehr-GPU-Paritaet und kein mehrstuendiger
Streaming-/VRAM-Soak behauptet. Die historischen Phase-20-Windwerte
114.20 -> 105.10 FPS / rund 100 MiB mehr RSS, Phase-31-Pilzwerte
145.47 -> 144.17 FPS / rund 53 MiB mehr Peak-RSS sowie die unterschiedlichen
Phase-30-LOD-/Fluss-p99-Ergebnisse bleiben historische Nachweise, keine durch
diese Laeufe rueckwirkend widerlegte Regression. Das dortige LOD-Paar hatte
13.552 -> 11.897 ms p99 bei 134.19 -> 133.30 FPS; der Fluss-p99 wurde dagegen
schlechter. Die neuen Laeufe ersetzen diesen Unterschied nicht durch eine
allgemeine Verbesserungsbehauptung.

Offscreen-Pruefung: vorhandene Atmosphaeren-Environmentmap/Fresnel ergaenzen SSR
bei fehlenden Bildschirmdaten, spiegeln aber keine unsichtbaren Weltobjekte.
Echte Objektreflexionen ausserhalb des Bildes benoetigen aktualisierte lokale
Cubemap-Probes oder weitere Planaransichten mit Geometrie-/Upload-/Passbudget;
ohne solchen Pfad wird keine entsprechende Option vorgetaeuscht.

Statische GLTF-Szenen bleiben gebatcht. Gelieferte Assets benoetigen keinen
Live-Skin-/Animationsrenderer. Animationen/Morphs/Skins werden weiter klar
abgelehnt. Eine Erweiterung braucht Clip-/Skinvertrag, Szeneninstanz-Lebenszyklus
und AnimationPlayer-/Graph-Integration an `blocks/loading.rs` und
`meshing/custom.rs`, nicht eine separate Cube-Registry. Das ist nicht umgesetzt.

Volle Frame-Drawabnahme und gemeinsame gestalterische Freigabe bleiben offen.
Konkrete Restarbeiten stehen in der Haupt-TODO; erledigte technische Arbeiten
werden mit diesen Messgrenzen in PATCH archiviert.
