# Wasserplatzierung, Audio Und Renderbudgets - Phase 30

## Gameplay Und UI

Der gemeinsame `targeting::block_placement`-Vertrag liefert Ziel und erwartetes
Material fuer Clientvorschau und serverseitige Sichtpruefung. Fluid Water (27)
wird in seiner eigenen Zelle ersetzt, nicht als solide Bauunterlage verwendet.
Gewoehnliches Fluss-/Zellwasser belegt ohnehin Luftzellen: Bauen gegen den
Untergrund verdraengt es dort. Solid Water (14) bleibt absichtlich ein fester
Dekorationsblock. Reichweite, Inventarverbrauch, Revisionen und Koerperkollision
gelten weiter; beliebige feste Bloecke duerfen nicht direkt ueberschrieben werden.

Das bestehende Blockjournal entfernt den Quellanker/Fuellstand beim Ersetzen.
Nachbarwasser wird nicht magisch geloescht: Es bleibt endlich und reagiert auf
die neue feste Barriere. Reload und echte UDP-Replikation sind abgedeckt.
Keine neue Speicherung, kein neues Block-/Weltformat, Protokoll weiterhin 14.

Kategorie-Tooltips bevorzugen oben rechts ausserhalb ihres Bildtabs; Itemslots
behalten unten rechts. Beide weichen bei Platzmangel aus, ohne Slots zu bedecken
oder den Viewport zu verlassen. Ausschliesslich bestehendes TiltUI.

## Wassergeraeusche

Die ruhige Stimme verwendet nun sehr langsam gefiltertes, leises Plaetschern
statt des breiten Rauschbandes. Die Flussstimme ist etwas kraeftiger, aber mit
weniger Hoehen, weicheren Ripples und leiseren Blasen. Wasserfallrauschen bleibt
eine getrennte Kategorie. Natuerliche Fluesse erhalten nicht mehr gleichzeitig
die laute Flachwasser-Grundstimme. Die Gefaelleprobe benutzt beide Komponenten
der realen Flussrichtung. Einstellbare Lautstaerken und Unterwasser-Tiefpass
bleiben erhalten. Quellenzellen blockieren akustische Strahlen/Immersion nicht.

Automatische Spektral-/Energietests pruefen: still < Fluss < Wasserfall,
weniger Hoehen bei stillem Wasser/Fluss, begrenzte Peaks, wirksame Audiooptionen.
Native Audioausgabe ist ein technischer Nachweis; menschliche Mixabnahme bleibt
ausdruecklich separat.

## Darstellung Und Performance

Die ersten drei Punkte des bisherigen TODO-Abschnitts werden gezielt bearbeitet:

- Fertige Terrain-/Prop-/Wassermeshes erhalten getrennte Vertexbudgets pro Frame
  (128000 / 96000 / 32000) zusaetzlich zu bestehenden Job-/Uploadlimits.
  Fertige Tasks werden bei Vertagung nicht erneut gepollt oder verworfen.
  Ein einzelner uebergrosser Batch darf fortschreiten, damit keine Section
  dauerhaft haengen bleibt. Der alte Meshroot bleibt bis zum atomaren Handoff
  stehen. Diese Budgets sind keine harte Millisekunden-/GPU-Zeitgarantie.
- Wasseruploads werden nach Spielerentfernung statt HashMap-Reihenfolge sortiert.
- Baumkandidaten sind generatorlokal in maximal 512 horizontalen Spalten geteilt;
  vertikale Sections und LOD-Neumeshing verwenden dieselben Arc-Daten. Die teure
  Art-/Geometriezuordnung je Zelle/Material wird in einem festen 20-KiB-Raster
  zwischengespeichert, nicht in einer HashMap pro Vertex. Maximal 512 geteilte
  Sectionsnapshots behalten dieses Raster auch ueber LOD-Neumeshing; einzelne
  Cachewerte sind atomar, sodass parallele Worker keinen Mutexpfad pro Zelle
  benoetigen. Raster kosten zusammen hoechstens 10 MiB plus Kandidaten/Metadaten.
  Cachemisses erzeugen ausserhalb der Sperre; Welt-/Generatorwechsel erzeugen
  eigene Caches. IDs, Artenfarben und generierte Bloecke bleiben unveraendert.
- Fertige PNG-Mipketten werden nur fuer die Renderwelt gehalten, statt ihre
  Pixel dauerhaft auch auf der CPU aufzubewahren. Laden erkennt bereits
  publizierte Render-only-Images weiterhin korrekt. Keine Aufloesungsreduktion,
  keine Aenderung der Masken, RGB-Werte oder Coverage-erhaltenden Mips.

`MeshUploadStatus` zaehlt zugelassene Vertices und vertagte Batches. Das sind
keine Draw Calls oder GPU-Uploadzeiten. Ein separater optionaler Zaehler wird
mit `TERRA_DRAW_DIAGNOSTICS=1` aktiviert. Er umschliesst die bestehenden
DrawMaterial-/Prepass-/Shadowbefehle an ihren unveraenderten registrierten IDs
und zaehlt nur erfolgreiche direkte/indirekte **PBR-Draw-API-Aufrufe**.
Opaque, AlphaMask, Transparent, Transmissive, Prepass, Deferred und Schatten sind
getrennt. Ein Multi-Draw-Aufruf zaehlt als ein API-Aufruf; GPU-entschiedene
Subdrawzahlen werden explizit nicht geraten. UI, Debuglinien, Compute und
Fullscreenpasses gehoeren nicht zu diesem PBR-Zaehler. Keine Aenderung des
Produktions-Batchings, keine erzwungene Abschaltung indirekter Zeichnung.

## Gepruefte Erweiterungen Und Grenzen

Ein zusaetzlicher Nebelabschluss am Fernhorizont wurde implementiert und nativ
gemessen, dann vollstaendig verworfen: Die transparente Geometrie beseitigte
den Atmosphaerenboden-Farbstreifen nicht verlaesslich und brachte zusaetzliche
GPU-Arbeit. Die Zwischenlaeufe sind Experimente, keine finalen Messwerte und
kein visueller Erfolgsnachweis. Der eigentliche Horizontpunkt bleibt offen.

Instancing/Texture Arrays wurden gegen den vorhandenen Pfad geprueft. Bevy
verwendet bereits indirekte Multi-Draw-Batches, Props sind Section-Batches;
ein neues paralleles System oder Atlas beseitigt die gemessene CPU-Arbeit
nicht automatisch. Daher keine unbelegte Renderer-/Materialumstellung.

`TERRA_FOLIAGE_REFERENCE=1` aktiviert ausschliesslich in Offline-Renderfixtures
einen undurchlaessigen, unbewegten Kronenvergleich. Geometrie, RGB-Texturen,
Artenfarben, Rauheit, Culling, Seed und Props bleiben identisch. Ein separater
Fixtureplugin erzeugt dann gemeinsame Standardmaterialien; der normale
Vorbereitungspfad behaelt seinen lesenden Materialzugriff. Es gibt keine
persistierte Grafikoption und keine Aktivierung in Gameplay-/Netzwerksitzungen.

Offscreenreflexionen des Himmels nutzen bereits AtmosphereEnvironmentMapLight;
SSR bleibt fuer sichtbare Szenegeometrie. Dynamische Objekte ausserhalb des
Bildes benoetigen eine neue Reflectiontechnik und bleiben offen. Bevy bietet
FogVolume/VolumetricFog, doch ein permanenter neuer Pass wird nicht als kostenlose
FPS-Optimierung eingeschaltet. Der budgetierte Wasserfallspray bleibt erhalten,
echte lokale Volumetrik optional. Statischer Custom-Mesh-Import ist weiterhin
kein GLTF-Skinning-/Animationssystem. Gemeinsame menschliche Artabnahme laesst
sich nicht durch einen automatisierten Screenshot als abgeschlossen ersetzen.

## Pruefungen Und Messungen

Alle fuenf geforderten Cargo-Pruefungen und der separate Headless-Build/-Start
sind bestanden: **557 Tests**, ein bestehender ignorierter Soak, Clippy ohne
Warnungen. Der Workspace-Lauf benutzt
`RUST_TEST_THREADS=1`: Ein erster paralleler Lauf stiess auf eine TCP-/UDP-
Ephemeralport-Kollision im bestehenden Netzwerktest-Setup, nicht auf einen
Wasser-/Renderassert. Der serielle Lauf prueft weiterhin echte UDP-Verbindungen.
Der vorhandene 180-s-Soak bleibt bewusst ignoriert, keine Langzeitabnahme.

Native Integrated-/UDP-Quellenrunde: Block27/amount8/source=true repliziert,
105 Nachbarzellen nass, fuenf Props einzeln journal-entfernt, zwei echte
Wassermeshes/322 Dreiecke. Nach CreativeSet fuer Stein ersetzt eine bestaetigte
Editaktion die Quelle in [10,16,23], ohne Sourceflag oder versteckten Fuellstand.
Zwei ScreenshotCaptured-Readbacks, Exit 0, keine Warnungen/Shaderfehler. Die
Aufnahme mit Stein wurde persoenlich betrachtet. Ausgelaufenes Nachbarwasser bleibt
endlich vorhanden, statt beim Ersetzen global zu verschwinden.

Native Creative-UI-Runde mit echten Wayland/uinput-Mausereignissen: Titel oben
rechts wurde anhand der echten Slot-/Tooltiprechtecke geprueft und die Aufnahme
persoenlich betrachtet. Drei breite/kompakte Aufnahmen; Cursor-/Tabwechsel,
MouseTweaks und Aussenwurf ergeben weiter 65 Inventar + 63 WorldDrop = 128.
Exit 0, keine Warnungen. Der erste Start ohne die ausdruecklich benoetigte
TERRA_NATIVE_OS_POINTER-Variable wurde korrekt abgebrochen und wiederholt.

Native Audioausgabe bestand mit echtem AudioSink, Kopf statt Kamera als Hoerpunkt,
Unterwasserfilter, Kategorien/Mute/Master und Sessioncleanup. Der Wasserfall-
RMS lag im Pruefmoment bei etwa 0.084 trocken und 0.025 unter Wasser; das sind
DSP-Kontrollwerte, keine menschliche Hoer-/Lautheitsabnahme.

Native Drawdiagnose: letzter fester Blick **42 indirekte PBR-API-Aufrufe**:
5 Opaque, 7 AlphaMask, 5 Opaque-Prepass, 7 Mask-Prepass, 18 Schatten. Alle 42
verwendeten GPU-entschiedene Multidrawzaehler; daraus wird keine Subdrawzahl
abgeleitet. Dieser kurze Instrumentierungslauf ist kein Performancevergleich.

### Vergleichsbedingungen

Identische `configs/`, Seed42042, Sichtweite16/3, LOD96/160, Wasser High,
VSync aus, RX6950XT/RADV Mesa26.2.4 Vulkan, Wayland150%, 1920x1200 physisch.
Die Vorherbinary wurde vor den Renderaenderungen gesichert, ohne abhaengige
UI-Templates umzubauen. Kein anderer von uns gestarteter Cargo-/GPU-Prozess
lief waehrend der jeweiligen Performancefenster. 18 Sekunden stationaer,
24 Sekunden fuer zwei identische 224-m-Hin-/Rueckfluege ueber negative
Koordinaten und beide LOD-Schwellen. Offline ohne Controller-/Netzwerkphysik.

| Szene | FPS vorher | FPS nachher | p95 ms vorher -> nachher | p99 ms vorher -> nachher |
| --- | ---: | ---: | ---: | ---: |
| Fester Pilzparkblick | 154.081 | 150.553 | 7.916 -> 8.042 | 8.890 -> 9.221 |
| LOD-Flug, erster Vorherlauf | 139.846 | 133.299 | 10.029 -> 9.994 | 13.148 -> 11.897 |
| LOD-Flug, wiederholter Vorherlauf | 134.187 | 133.299 | 10.624 -> 9.994 | 13.552 -> 11.897 |
| Gepruefter Fluss-Aussenblick | 139.276 | 139.139 | 8.557 -> 8.520 | 9.361 -> 10.223 |

Kein pauschaler FPS-Gewinn: Pilzpark im Paar rund 2.3 Prozent langsamer;
der zeitnaehere Flugvergleich zeigt rund 0.7 Prozent weniger FPS, dafuer
rund 12 Prozent weniger p99-Zeit. Beide Vorherfluege bleiben sichtbar, keine
Auswahl nur des guenstigsten Laufs. Fluss-FPS nahezu gleich, dessen p99 dagegen
schlechter. Das sind kurze Einzelpaare mit beobachteter Laufstreuung, keine
statistisch abgesicherte Performanceparitaet.

Pilzpark behaelt 7807 geladene/4115 gerenderte Sections, 9636 Meshassets,
4546 sichtbare Meshentities sowie 92789 repraesentierte Props in 1306 Batches.
Terrain-Workerzeit 0.729 -> 0.708 s, Prozess-RSS 919516 -> 899288 KiB,
erster Readyzeitpunkt 3.089 -> 3.324 s. Die zusaetzliche Uploadbegrenzung kostet
hier rund 0.24 s Erstladezeit. Der Flug beendet 15493 echte LOD-Handoffs und
alle Meshjobs. Die Rastercaches kosten bewusst Speicher: Flug-RSS liegt mit
995836 KiB knapp ueber dem wiederholten Vorherlauf989920, aber unter dem ersten
1045344; keine generelle Speicherparitaet aus einem Einzelwert ableiten.

Der Fluss-Aussenblick wurde durch Luft-/Sichtstrahlpruefung gewaehlt und
persoenlich betrachtet: Kamera(35.5,114.161835,1580.5), 31.83-m-Fall.
Beide Versionen haben 9055/3528 Sections, 8007 Meshassets, 53075 Props/
966 Batches und 131 Wassertiles. Terrain-Worker0.607 -> 0.590 s,
RSS810660 -> 821944 KiB, Ready3.941 -> 3.997 s. Der vorher ebenfalls gemessene
enge Blick innerhalb einer Fallgardine ist kein Bewuchs-Aussenvergleich und
wird separat als solcher eingeordnet, nicht als passendere schnelle Messung.

GPU-Diagnostik nennt einzelne PBR-/Prepass-/Wasserzeiten, keine gesamte
GPU-Framezeit. Worker-Spannensummen sind keine serielle CPU-Generierungszeit.
Keine neue 4375-Section-Kaltstartmessung, kein Bewegungs-/Langzeit-/Mehrrechner-
Worst Case. Nachher-Rohdaten und PNGs sind unter `logs/` und `captures/` archiviert.

### Aktueller Masken-/Windlaub-A/B

Beide Varianten verwenden denselben aktuellen Build, dieselben Konfigurationen,
dieselbe Geometrie und dieselben Kameras. Nur die drei Blattmaterialien werden
in der Referenz auf gemeinsame undurchlaessige Standardmaterialien ohne
Windvertexshader gesetzt. Gras-/Buschwind bleibt unveraendert; dies isoliert
Windlaub/Maskierung, nicht nachtraeglich jede historische Pflanzenaenderung.
Vier neue 18-s-Laeufe, keine parallelen eigenen Cargo-/GPU-Prozesse.

| Szene / Kennwert | Opaque/still Referenz | Maskierung/Wind normal |
| --- | ---: | ---: |
| Pilzpark FPS | 155.432 | 151.120 |
| Pilzpark p95 / p99 ms | 7.584 / 8.483 | 7.740 / 8.432 |
| Pilzpark Opaque-GPU-Pass Mittel ms | 1.096 | 1.213 |
| Pilzpark RSS KiB | 896040 | 928268 |
| Fluss FPS | 142.475 | 140.023 |
| Fluss p95 / p99 ms | 8.377 / 9.509 | 8.448 / 9.263 |
| Fluss Opaque-GPU-Pass Mittel ms | 1.319 | 1.325 |
| Fluss RSS KiB | 821656 | 829452 |

Im Pilzpark kostet die kombinierte Blattmaskierung/-bewegung hier rund
2.8 Prozent mittlere FPS und rund 0.117 ms im Opaque-Pass, am Fluss rund
1.7 Prozent FPS. Speicherwerte streuen und enthalten Treiber-/Allocatorpools;
kein exklusives GPU-VRAM-Messgeraet. Der historische Flag `forest` zeigt heute
Mushroom Park, nicht den alten dichten Phase-20-Wald. Diese neuen Paare ersetzen
den belasteten Flussversuch als aktuelle reproduzierbare Einordnung, beweisen
aber keine Aufhebung der historischen Regression auf anderem Terrain.

Reproduktion nach dem Build, jeden Aufruf einzeln und ohne weitere Benchmarks:

```bash
env -u DISPLAY TERRA_CONFIG_DIR=tools/benchmarks/results/water-placement-performance-2026-10-16/configs \
  TERRA_BENCH_SECONDS=18 TERRA_RENDER_DIAGNOSTICS=1 TERRA_FOLIAGE_REFERENCE=1 \
  TERRA_RIVER_SPRAY=1 TERRA_RIVER_MIN_FALL=30 \
  python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-river-benchmark
```

Danach denselben Aufruf ohne `TERRA_FOLIAGE_REFERENCE` wiederholen. Fuer den
Pilzpark `--smoke-test-forest` und ohne beide TERRA_RIVER-Variablen verwenden.
Der Shader-/Horizontversuch und der joblokale erste Cacheversuch sind als
`*-experiment.log` gesondert archiviert, nicht als finaler Erfolgsnachweis.

### Nachweise

[Tests](logs/terra-phase30-tests.log), [Format](logs/terra-phase30-fmt.log),
[Check](logs/terra-phase30-check.log), [Clippy](logs/terra-phase30-clippy.log),
[Build](logs/terra-phase30-build.log), [Headless](logs/terra-phase30-headless.log),
[Server-Features](logs/terra-phase30-headless-features.txt).

[Quellenlog](logs/terra-phase30-source-native.log),
[Quelle durch Stein ersetzt](captures/source-replaced.png),
[UI-Log](logs/terra-phase30-inventory-native.log),
[Tabtitel oben rechts](captures/inventory-category-hover.png),
[Audiolog](logs/terra-phase30-audio-native.log),
[Drawdiagnose](logs/terra-phase30-draws.log).

[LOD vorher](logs/terra-phase30-lod-before.log),
[LOD vorher wiederholt](logs/terra-phase30-lod-before-repeat.log),
[LOD nachher](logs/terra-phase30-lod-after.log),
[Fluss-Aussenblick](captures/terra-phase30-river-bank-after.png),
[Windlaub-A/B Referenz](logs/terra-phase30-river-opaque-reference.log),
[Windlaub-A/B normal](logs/terra-phase30-river-masked-wind.log).
