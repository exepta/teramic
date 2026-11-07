# Streaming-Ausreisser Und Hoehlendaecher (Phase 136)

## Ergebnis

Der reproduzierte Sonnenlicht-Durchtritt durch vorhandene Hoehlendaecher ist
behoben und nativ fuer Near, Middle und Far geprueft. Ein allgemeiner Abschluss
des Low-FPS-Problems wird **nicht** behauptet. Der seltene 118-ms-Ausreisser
trat im ersten Diagnoseflug auf, aber auch die unveraenderte Ausgangsversion
wiederholte ihn im zweiten Lauf nicht. Aus seinem Ausbleiben nachher folgt
kein nachgewiesener Fix seiner Ursache. A05 bleibt offen.

Keine reduzierten Sichtweiten, Lichtbudgets, Schattenseiten, Vegetationsdichten
oder Wasserqualitaeten. Keine neuen Langzeittests und keine Bevy-/Vendor-Patches.
Die bestehende zehnsekundige F1-Warmup-Regel bleibt unveraendert; reale spaetere
Stalls werden nicht aus den Session-Extrema herausgefiltert.

## Code

- `scene/caves/lights.rs`: Der begrenzte Punktlichtpool behaelt die Entities
  passender Quellen und verwendet freie Plaetze fuer neue Quellen wieder.
  Bevy kann damit deren sechs Schattenansichten weiterverwenden. Verkleinern,
  Vergroessern und Weltende raeumen weiterhin korrekt auf. Der Regressionstest
  tauscht alle acht Quellen aus und prueft erhaltene Entity-IDs und Parameter.
- `meshing/upload.rs`: Bereits identische einblendende Ergebnisse werden vor
  Budgetreservierung und `Assets<Mesh>::add` konsumiert. Zuvor erfolgte dieser
  Check erst nach dem Staging. Identitaet schliesst nun LOD und Micro-Detail ein,
  damit echte Detailwechsel nicht versehentlich verworfen werden. Tests pruefen
  acht Wiederholungen mit freiem/erschoepftem Budget, unveraenderte Assetzahlen,
  abgearbeitete Jobs sowie echte LOD-/Detailwechsel mit Freigabe des alten Roots.
- `weather/surfaces.rs`: Die vorhandene Luftseite einer Hoehlendecke ist fuer
  Sonnenlicht eine Rueckseite. Normales PBR-Backface-Culling verwarf sie auch im
  Schattenpass. Near-Schattenproxies existieren nicht fuer alle Materialien,
  insbesondere bei gemeinsam mit Modellen benutzten Slots; Middle/Far besitzen
  ohnehin keine solchen Proxies. Opaque Terrain-Materialien rendern deshalb
  jetzt **nur im Tiefen-/Schattenpass** beidseitig. Der Farbpass bleibt gecullt.
- Trockenes Terrain benutzt eine bindingfreie `TerrainDepthExtension` ueber
  Bevy's oeffentliche Material-API: unveraenderte Stock-PBR-Shader und das
  StandardMaterial-Bindless-Layout, ein wiederverwendeter Handle je Materialslot.
  Es bleibt ein eigener Materialtyp/Batch-Pool, kein gemeinsamer Pool mit allen
  StandardMaterial-Entities. Nassmaterialien bleiben im bestehenden Wet-Pfad.
  Far-Batching, Fade-Clones, Draw-Readiness, Restore und Freigabe sind integriert.
  Tests pruefen Descriptor-Paritaet, alle LODs, Basisupdates ohne neue Handles
  und 100 Fade-/Restore-/Freigabezyklen.
- Diagnose optional: `TERRA_CPU_SCOPES=1 TERRA_STALL_TRACE=1` meldet einzelne
  Render-Stage-/Systemaufrufe ab 32 ms, darunter Window-Acquire, View-Targets und
  Lichtvorbereitung. Wall-Time ist keine gemessene GPU-Framezeit.

## Native FPS-Vergleiche

Ryzen 9 5900X, RX 6950 XT, Vulkan/RADV, Mesa 26.2.5; optimiertes Dev-Profil,
1280x800, VSync aus, Sichtweite 20, High-Schatten/Wasser/Wetter. Keine parallelen
Compiler oder Tests waehrend der Messfenster. Separate temporaere Welten und
Menues; keine gespeicherte Benutzerwelt veraendert.

Hauptlauf: Seed **5866727212944175842**, integrierter Server, echtes HUD und
Spielereingaben, Regen, 60-s-Explorationsroute mit Hoehenwechsel und Chunk-Streaming.
Beide abschliessenden Vergleichslaeufe ohne CPU-Probes und Soak-Reports:

| Stand | FPS Mittel | Min FPS | p95 ms | p99 ms | Schlechtester Frame ms | Laden s | RSS KiB |
|---|---:|---:|---:|---:|---:|---:|---:|
| Ausgangsversion | 85.51 | 27.94 | 18.79 | 24.26 | 35.79 | 34.68 | 2328512 |
| Finaler Code | 86.09 | 30.65 | 18.76 | 24.78 | 32.63 | 34.67 | 2368920 |

Das ist weitgehend gleiche Gesamtleistung bei korrekterer Abschirmung, kein
belastbarer grosser FPS-Gewinn. Insbesondere p99 ist nicht verbessert. Beide
Laufenden haben 13606 geladene Sections, acht schattenwerfende Punktlichter,
142271 geladene Props und 22 Wasser-Tiles; asynchron dargestellt 5597/5566
Sections und 141937/140825 Props. Endeigenschaften sind nicht pixelidentisch.
Die Zeitreihen dieser Smoke-Messungen sind nicht identisch mit F1-Session-Extrema.

Zweiter Seed **7**, wasserreicher 24-s-Hin-/Rueckflug mit Regen, dieselbe
Aufloesung und Sichtweite, ohne CPU-Probes; hier Offline-Renderer statt
integriertem Server/HUD:

| Stand | FPS Mittel | Min FPS | p95 ms | p99 ms | Schlechtester Frame ms | Laden s | RSS KiB |
|---|---:|---:|---:|---:|---:|---:|---:|
| Ausgangsversion | 72.80 | 24.89 | 23.82 | 26.14 | 40.18 | 23.50 | 1613220 |
| Finaler Code | 77.30 | 24.78 | 22.19 | 24.10 | 40.35 | 23.43 | 1688024 |

Hier verbessern sich Mittelwert und p95/p99, nicht der schlechteste Frame.
Beide Enden haben exakt 15831 geladene / 3756 dargestellte Sections, 12989
Mesh-Assets, 27122 Props und 1681 Wasser-Tiles, keine Punktlichter. Daraus wird
kein Lichtpool-Gewinn in einer Szene ohne Punktlichter abgeleitet. Kleine
asynchrone Unterschiede der laufenden Kompositionsjobs bleiben vorhanden.

### Diagnose Und Verworfene Ansaetze

Der erste Diagnoseflug mit CPU-Probes und 30-s-Soak-Reports erreichte 82.31 FPS,
p95 19.85 ms, p99 26.00 ms und einmal **118.55 ms = 8.44 FPS**. Das zweite
30-s-Intervall enthielt einmal 88.27 ms in `PrepareViews`, waehrend `CreateViews`
unter 0.9 ms blieb. Die Lichtvorbereitung laeuft in `CreateViews`; sie ist damit
nicht als direkte Ursache dieses Peaks belegt. Window-Acquire kann auf GPU/
Swapchain warten, aber der alte Lauf hatte dafuer noch keine Einzelprobe.
Grosse Mesh-Pufferkopien lagen mehrere Sekunden vorher und sind nicht als
Ursache desselben Frames nachgewiesen. Auch eine Kausalitaet des Soak-Reports
wird nicht aus dem aehnlichen Zeitstempel abgeleitet.

Drei verworfene Zwischenstaende sind zur Nachvollziehbarkeit gespeichert:

| Diagnosevariante, gleicher Hauptlauf | FPS Mittel | p95 ms | p99 ms | Schlechtester Frame ms |
|---|---:|---:|---:|---:|
| Trockene Flaechen ebenfalls im non-bindless Wet-Pfad | 74.74 | 20.66 | 27.12 | 38.06 |
| Zusaetzlich 32-MiB-Mesh-Slab-Limit | 46.80 | 31.90 | 38.59 | 48.20 |
| Wet-Dry mit weggekuerzter Detailarbeit | 73.41 | 22.30 | 28.10 | 36.47 |

Die 32-MiB-Begrenzung ist verworfen; normale Allocator-Defaults bleiben
unveraendert. `TERRA_MESH_SLAB_MIB` ist ausschliesslich eine Smoke-Test-A/B-Option,
keine neue Spieleinstellung. Der Wet-Dry-Pfad ist durch die bindingfreie
Depth-Extension ersetzt. Zusaetzliche Shaderarbeit allein erklaerte dessen
Regression nicht. Der finale Produktionsvergleich oben ist ohne Profiler;
die Diagnosezeilen werden nicht als exakt isolierte Mikrobenchmarks ausgegeben.

## Native Hoehlenpruefung

`TERRA_CAVE_LOD_SHADOW_INSPECT=1 --smoke-test-caves` verwendet eine massive
Ueberdeckung ohne sichtbare Oberseite und denselben Bodenreceiver. Der Test
traced die **tatsaechliche**, fuer kurze Schatten geklemmte Lichtrichtung gegen
die geladenen Blockdaten und prueft Caster-Section (0,0,1), LOD, Materialtyp,
Mesh-Asset, Sichtbarkeit und aktive schattenwerfende Sonne. Keine Sun-Off-Maske.

Referenzmodus `TERRA_CAVE_LOD_SHADOW_REFERENCE=1` schaltet nur fuer diese Fixture
auf den alten trockenen Standardpfad. Seine Abschlussmeldung nennt ihn bewusst
keinen bestandenen Isolationstest.

| Caster-LOD | Alter Materialpfad, lineare Luma | Korrigierter Pfad |
|---|---:|---:|
| Near, hier ohne Proxy | 0.189369 | 0.002054 |
| Middle | 0.189355 | 0.002052 |
| Far | 0.189348 | 0.002054 |

Screenshots der Middle-Referenz und Korrektur wurden visuell betrachtet und
liegen unter `captures/`. Das sind begrenzte Lichtfixtures, keine visuelle
Abnahme aller prozeduralen Hoehlen. Fehlende/entladene bzw. vollstaendig
portalentfernte Dachgeometrie wird nicht ersetzt; deaktivierte Schatten bleiben
deaktiviert.

Die bestehende Eingangsfixture besteht ebenfalls: gemeinsamer Aussenboden von
innen 0.191722, von aussen 0.245376; Himmel 0.366496/0.377610. Geschlossene
Fackelansicht 0.017443, Unterwasser 0.009432 mittlere Capture-Helligkeit.
Eingangs- und geschlossene Ansicht wurden betrachtet. Die grossflaechigen
Terrain-Overrides lassen ausserhalb der Testwand bestehende generierte Props
stehen; diese schwebenden Fixture-Props sind keine korrekt generierte Szene.

## Reproduktion

Hauptlauf mit `config/` als `TERRA_CONFIG_DIR`, getrennten temporaeren
`TERRA_MENU_CONFIG_DIR` und `TERRA_WORLD_DIR`:

```text
TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 TERRA_BENCH_GRAPHICS=1
TERRA_BENCH_SECONDS=60 TERRA_BENCH_WEATHER=rain TERRA_BENCH_LOAD_SECONDS=180
TERRA_SOAK_ROUTE=explore WGPU_BACKEND=vulkan
target/debug/terra-client --smoke-test-benchmark
```

Wasserflug: `config-water/`, `TERRA_BENCH_GRAPHICS=1`,
`TERRA_BENCH_SECONDS=24`, `TERRA_BENCH_WEATHER=rain`,
`TERRA_BENCH_LOAD_SECONDS=240`, `--smoke-test-lod` statt `--smoke-test-benchmark`.
Keine integrierte Route oder HUD-Flags bei diesem separaten Renderer-Test.

Die Ausgangsbinary wurde vor den Aenderungen unter `/tmp/teramic136-baseline-client`
gesichert. Die Rohlogs sind hier mit ihren urspruenglichen Dateinamen erhalten.
`teramic136-final.log` ist trotz Dateiname ein **Wet-Dry-Zwischenstand**;
der finale Produktionsvergleich ist `teramic136-clean-after.log`.

## Validierung

- 2553 Workspace-Tests bestanden, sieben ignoriert, mit `-j2 -- --test-threads=2`.
- Formatierung, Workspace/All-Targets-Check, striktes Clippy und Workspace-Build
  erfolgreich; Client und Dedicated Server bauen.
- Native Near/Middle/Far-, Eingangs-, Fackel- und Unterwasserpruefungen bestanden;
  Referenzaufnahmen zeigen den tatsaechlichen Lichtfehler.
- Keine neuen 10-/15-/30-Minuten-Laeufe. Keine Behauptung, der seltene 9-FPS-
  Ausreisser oder der allgemeine zeitabhaengige FPS-Verfall sei abschliessend behoben.
