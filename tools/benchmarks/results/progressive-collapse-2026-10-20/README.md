# Phase 91: Langzeitperformance Und Replikations-Lifecycle

Stand: Codefixes, Workspace-Validierung und native 30-/15-Minuten-Gegenproben
abgeschlossen. Der konkret gemeldete anhaltende Unter-18-FPS-Einbruch war
bereits im Vorherlauf nicht reproduziert; keine pauschale Behebung behauptet.

## Nachgewiesene Codefehler

- Replizierte Blockaenderungen und erneutes Veroeffentlichen einer Section
  legten Fluidauftraege im Clientjournal an. Der Client simuliert diese nicht;
  nur die Autoritaet arbeitet Fluid- und Graswachstum ab. Im nativen Vorherlauf
  stieg die ungenutzte Clientqueue von 0 auf 1014 Eintraege. Bestaetigte Importe
  und Replica-Reloads erzeugen jetzt keine autoritativen Simulationsauftraege.
  Gespeicherte Daten, Quellen, Revisionen und Renderinvalidierungen bleiben
  unveraendert. Server-/Speicherimporte behalten ihre Wiederaufnahme.
- Blockierte Kuehe, Schafe und Schweine starteten A* auch ohne Suchfreigabe,
  statt den vorhandenen Routen-Cache und das Acht-Suchen-Budget zu benutzen.
  Anstehende Futter-/Wasserrouten duerfen jetzt ebenfalls Budget anfordern.
  Die Suchplaetze rotieren fair auch bei mehr als 64 dauerhaft blockierten
  Tieren; Bewegungsfrequenz und Acht-Suchen-Limit bleiben erhalten.
- Die Wassersuche durstiger Tiere berechnete fuer jede Probe erneut die
  Terrain-/Fluss-/Hoehlenfelder. Sie verwendet jetzt die bereits vorbereiteten
  Spalten, einschliesslich vorhandener Live-Overrides und bisherigem Rueckfall
  fuer fehlende Spalten. Die Beduerfnis-Schwellen werden nach etwa 3-5 Minuten
  erreicht. Das ist ein konkreter Kostenpfad, noch kein Nachweis, dass er den
  vollstaendigen gemeldeten FPS-Kollaps allein verursacht.
- Rohrfuellungen entstehen erst ab drei opaken Nachbarn; ein und zwei Nachbarn
  bleiben offen. Drei-/vierseitige gemischte Materialien behalten dieselbe
  Auswahl-, Kollisions- und Batchgeometrie.

## Geaenderte Codepfade

| Datei | Funktionen / Verantwortung |
| --- | --- |
| `crates/terra-world/src/edits/journal.rs` | `import_replica_delta`, gemeinsamer `import_delta_with_simulation`, `record_with_order`: gleiche Zustandsuebernahme ohne Replica-Wakeups |
| `crates/terra-world/src/edits/fluids.rs` | `write_fluid_with_wake`: Daten/Invalidierung getrennt von autoritativem Aufwecken |
| `crates/terra-world/src/streaming/resources.rs` | `ChunkStore::apply_confirmed_delta`: bestaetigte Importe mit vorhandener Halo-Invalidierung |
| `crates/terra-world/src/streaming/jobs.rs` | `receive_jobs`: Fluid-Wiederaufnahme nur unter vorhandener Authority-Pruefung |
| `crates/terra-world/src/creatures.rs` | Landbewegung ueber `ground_waypoint`, gecachter Umweg und Suchfreigabe statt direktem unbudgetiertem A* |
| `crates/terra-network/src/server_creatures.rs` | `search_due`, `grant_searches`, `tick`: anstehende Routen und fairer Acht-Suchen-Cursor |
| `crates/terra-world/src/creature_needs.rs` | `water_surface`: bestehendes `ChunkStore::fluid_cell_at` statt ungecachtem Generatorsampling |
| `crates/terra-world/src/objects.rs` | `chimney_infill`: mindestens drei opake Nachbarn |
| `apps/client/src/performance_soak.rs`, `performance_route.rs`, `performance_edits.rs`, `bootstrap.rs` | Opt-in Telemetrie, reale Eingaben/Edits und explizite isolierte gespeicherte Benchmarks |
| `crates/terra-storage/examples/furnace_soak_fixture.rs` | Separater Offline-Fixturegenerator, kein Eingriff in regulaeres Gameplay |

Zusaetzlich gezielte Regressionen in `edits/tests.rs`, Streaming-, Navigation-,
Beduerfnis-/Populationstests sowie Terrain-/Prop-/Wasser-/Transition-Lifecycle-
Tests. `tools/benchmarks/measure.py`, `run-soak.sh`, `summarize_soak.py` und die
zugehoerigen Python-Tests liefern reproduzierbare Ressourcen-/Zeitreihen.

## Messbedingungen

Ryzen 9 5900X, RX 6950 XT / RADV / Vulkan, 32 GiB RAM. Optimiertes Dev-Profil
wie beim normalen `cargo run`: Workspace opt-level 1, Dependencies opt-level 3.
Kein paralleler Compiler und kein zweiter Spielclient in den Messfenstern.
Die vorhandene Desktop-Sitzung blieb aktiv.

Seed 2262437629133911810, echte 3840x2160, Sichtweite 12, vertikal 3,
LOD 153,6/256 m, High-Schatten/-Wasser, Ultra-Wetter, Rise-Animation,
ungekuerzte Vegetation, echter integrierter Server und TiltUI inklusive Minimap.
Creative vermeidet den Tod waehrend automatischer Fluege. Der Server nutzt
Simulationsweite 6 und ein Limit von 256 natuerlichen Tieren. Messbeginn nach
vollstaendig geladener Szene. Natuerliche Wetterwechsel und Weltzeit laufen
weiter; Regen wird zu Beginn autoritativ angefordert, nicht fuer 15 Minuten
statisch erzwungen. Vorherbuild: `target/phase91/terra-client-before`.

Der erste Idlelauf verwendet eine frische Welt ohne Savepfad, keine gespeicherte
Benutzerwelt. Das ist kein Nachweis fuer grosse gespeicherte Welten oder
Autosave. Ein optionaler isolierter Savepfad ist jetzt ausdruecklich verfuegbar.
Die Originaldaten unter `worlds/Test` wurden nur gelesen und in die Messordner
kopiert; `source-world.sha256` erlaubt die anschliessende Integritaetspruefung.

## Vorherlauf

`before-idle.log`: 900 Sekunden Messfenster, 917,512 Sekunden Prozesslaufzeit,
94,90 mittlere FPS, p95 14,966 ms, p99 18,385 ms. Kein Absturz oder Render-ERROR.
Kein FPS-Kollaps in diesem stationaeren Szenario reproduziert. Mesh-Assets
12087 -> 12084, Standardmaterialien unveraendert 199; 4393 geladene Sections.

| Messzeit | FPS vorher | RSS vorher (MiB) | FPS nachher | RSS nachher (MiB) |
| --- | ---: | ---: | --- | --- |
| 1 Minute | 96,08 | 1284,66 | 97,16 | 1233,18 |
| 5 Minuten | 95,78 | 1286,58 | 99,32 | 1146,84 |
| 10 Minuten | 95,72 | 1300,77 | 99,52 | 1166,54 |
| 15 Minuten | 95,60 | 551,16 | 99,30 | 1218,79 |
| 30 Minuten | nicht durchgefuehrt | nicht durchgefuehrt | 89,69 (Flug) | 1503,88 |

Nachherwerte stammen aus `after-30m.log`, dessen erste 900 Sekunden stationaer
sind. Alle Werte sind echte letzte 30-s-Fenster, keine kumulativen FPS und keine
Interpolation. Ab Minute 15 aendert sich die Szene durch echten Flug; spaetere
Werte sind deshalb kein identisches stationaeres A/B-Paar. Isolierte Karten-
Caches werden zwischen diesen Laeufen wiederverwendet. Eine Einzelpaarung
belegt weder allgemeine FPS-Steigerung noch Speicherparitaet.

**RSS ist nicht reservierter Gesamtspeicher.** Die Desktop-Sitzung benutzt Swap.
Bei Minute 13 wurden zusaetzlich rund 695 MiB Prozess-Swap festgestellt. Der
spaetere RSS-Rueckgang wird deshalb nicht als Speicherfreigabe oder Optimierung
ausgelegt. Zukuenftige Timeline-Messungen erfassen Swap und Major-/Minor-Faults.
Die alte Telemetrie nannte `Entities::len()` irrtuemlich `entities`; dieser Wert
ist in Bevy 0.19 die Indexkapazitaet, nicht die Zahl lebender Entities. Der
Auswerter kennzeichnet diese Altdaten; neue Laeufe zaehlen `count_spawned()`.

Einzelne GPU-Passzeiten und PBR-API-Aufrufe sind keine gemessene gesamte
GPU-Framezeit bzw. vollstaendige variable GPU-Subdrawzahl.

## Nativer 30-Minuten-Nachherlauf

`after-30m.log`: 1800 Sekunden Messfenster, 1816,702 Sekunden Prozesslaufzeit,
Exit 0. Erste 900 Sekunden stationaer: 97,87 FPS ueber alle 30-s-Fenster.
Danach echte Fly-Eingaben fuer 900 Sekunden: 91,34 FPS im Mittel,
85,69..98,72 FPS je 30-s-Fenster. Sieben vollstaendige 600-m-Hin-/Rueckwege
sind serverbestaetigt; der achte Hinweg laeuft beim regulaeren Testende noch.
Gleiche Hinwegfenster liefern 91,26 / 89,27 / 86,56 / 90,34 / 90,97 / 90,69 /
89,99 / 89,69 FPS: kein monotoner Einbruch pro Wiederholung.

**Aufloesungsgrenze:** Anfangsscreenshot ist 3840x2160, die Enddiagnose meldet
3840x2124 trotz unveraenderter angeforderter 3840x2160. Der Zeitpunkt dieser
Fenstergroessenaenderung wurde nicht protokolliert. Die letzte Zeile hat etwa
1,67% weniger Pixel als der Vorherlauf. Deshalb kein streng identischer
30-Minuten-FPS-A/B-Nachweis und keine kausale FPS-Steigerung behaupten.
Sichtweite, LOD-Distanzen, Vegetation, Wasser und Schatten wurden nicht reduziert.

Der separate Native-Benchmarkzaehler (Screenshot-Nachbarframes ausgeschlossen)
liefert insgesamt 94,61 FPS, p95 15,182 ms und p99 19,274 ms. Die Soak-Timeline
schliesst diese Frames dagegen ein und dokumentiert auch 186,68 ms im ersten
Fenster sowie spaetere einzelne Spikes. Beide Messmethoden werden nicht vermischt.

Bei gleichen Streckenphasen sind Assets wiederkehrend: beispielsweise 7973
Mesh-Assets im ersten und 7979 im achten fruehen Hinweg; am weit entfernten
Hinwegfenster 8620/8621. Standardmaterialien gehen zwischen Rueckwegen auf 199
zurueck, statt temporaere Varianten aufzusummieren. Generation-/Meshingqueues
werden abgearbeitet, die Client-Fluidqueue bleibt in allen 60 Proben leer.
Kumulative Job-/Uploadzaehler sind keine wachsenden Warteschlangen.

Peak-RSS 1506,89 MiB, Peak-DRM-VRAM 4637,75 MiB; GTT 68,14 MiB. Der VRAM-Bestand
liegt in der letzten langen Streamingphase konstant bei 4486916 KiB. RSS ist
gegenueber dem Beginn gestiegen; trotz begrenzter nachgepruefter Caches ist
die genaue Zuordnung dieses CPU-Speicheranstiegs nicht bewiesen. Kein Nachweis
von Speicherparitaet oder mehrstuendiger Stabilitaet. Kein ERROR oder Panic;
beim Verbindungsaufbau existiert eine einzelne Lightyear-Warnung ueber noch
nicht behandelte `CreatureStates`, die verworfen werden, nicht akkumulieren.

## Gespeicherte Welt

`saved-after.log`: zweiter Seed 10572586493552409009, bestehende kopierte
Welt `Test`, vollstaendiger gespeicherter Generatorvertrag und urspruengliche
Spieler-UUID. Autosave laeuft ueber den normalen integrierten Server. Keine
Veraenderung der Originalwelt: alle vier urspruenglichen Manifest-/Level-/Region-/
Spieler-Pruefsummen stimmen nach dem Lauf ueberein (`source-world.sha256`).
Andere Szene und Tageslaenge: nicht als direktes Vorher-/Nachher-Paar auslegen.

| Messzeit | FPS | p95 (ms) | p99 (ms) | RSS (MiB) |
| --- | ---: | ---: | ---: | ---: |
| 1 Minute | 108,64 | 12,262 | 15,375 | 1261,26 |
| 5 Minuten | 101,43 | 13,535 | 17,553 | 1307,18 |
| 10 Minuten | 107,03 | 12,100 | 16,111 | 1333,34 |
| 15 Minuten | 109,65 | 12,530 | 14,329 | 1355,45 |

Die Welt enthaelt etwa 30 replizierte Tiere, darunter von Beginn an durstige
Tiere. 4506 geladene Sections, Mesh-Assets 13385 -> 13382, Standardmaterialien
199 und Images 195 bleiben stabil. Die Client-Fluidqueue bleibt durchgehend
leer, bei 644 bestehenden Fluiddatensaetzen ohne aktive Quellen. Persistierte
Oberflaechenzustaende und Journalaenderungen nehmen waehrend natuerlichem
Wetter-/Zeitverlauf zu; der RSS-Anstieg von rund 94 MiB wird nicht als
Speicherparitaet ausgegeben. Es gibt keine Oefen in diesem Spielstand, daher
prueft dieser Lauf weder geschlossene Rauchraeume noch viele brennende Oefen.
Die erste 30-s-Probe enthaelt einen 179,99-ms-Spike; auch dieser verschwindet
nicht aus der Auswertung. Prozesslaufzeit 921,852 s, Peak-RSS 1396,79 MiB,
Peak-DRM-VRAM 4191,64 MiB, Exit 0, keine ERROR-/Panic-Meldung.

## Interaktionspilot

`after-edits-pilot.log`: zwei vollstaendig bestaetigte Zyklen mit echter
Fluessigkeitsausbreitung, Quelle -> Stein -> Luft sowie Bodenabbau/-wiederaufbau.
Die lokale Auswahl und Serverantwort werden geprueft. Beide Zyklen stellen
ihre Zellen wieder her; die restliche 60-s-Messzeit bleibt die Szene stationaer.
Dieser Pilot ist kein 15-Minuten-Terraforming-Nachweis.

`after-edits.log` ist dagegen der vollstaendige 180-s-Lauf mit zwoelf Zyklen
und insgesamt 60 bestaetigten Blocktransaktionen. Jeder Zyklus prueft echte
Quell-/Nachbarzellen, vollstaendigen lokalen Abfluss und wiederhergestellten
Boden. Nach 120 Sekunden folgt eine Minute ohne weitere Fixture-Edits.
Mesh-Assets bleiben am Ende bei 12088, Standardmaterialien bei 199,
Fluiddatensaetze/Quellen/Clientqueue bei null. Exit 0, keine ERROR-/Panic-Meldung.
Echte 3840x2160 laut Enddiagnose; die Kamera wird zum Zielen gedreht, daher
kein FPS-Vergleich mit der unveraenderten Idle-Kamera. Auch dieser Lauf ist
kein 15-Minuten-Terraforming-Nachweis.

## Geschlossener Rauchraum

`smoke-retry.log`: 900 Sekunden Messfenster, 916,609 Sekunden Prozesslaufzeit,
echter gespeicherter Ofen mit regularem Brennstoff und integrierter Autoritaet.
Das Rauchfeld startet leer, hat nach der ersten 30-s-Probe 44 Zellen und
erreicht zwischen 240 und 270 Messsekunden das Limit von 256. Es bleibt bis
zum Testende bei 256, der Ofen bleibt durchgehend an.

| Messzeit | FPS | p95 (ms) | p99 (ms) | RSS (MiB) |
| --- | ---: | ---: | ---: | ---: |
| 1 Minute | 111,00 | 12,515 | 14,181 | 1246,66 |
| 5 Minuten | 108,71 | 12,682 | 14,621 | 1319,25 |
| 10 Minuten | 107,16 | 12,033 | 14,340 | 1338,70 |
| 15 Minuten | 107,55 | 13,145 | 14,922 | 1349,15 |

Alle 30-s-Fenster liegen zwischen 96,73 und 113,12 FPS; Native-Benchmarkmittel
107,42 FPS, p95 12,572 ms, p99 15,639 ms. Kein anhaltender zeitabhaengiger
Kollaps und keine ERROR-/Panic-Meldung, Exit 0. Mesh-Assets 12095 -> 12092,
Standardmaterialien 199 und Images 192 bleiben begrenzt; Client-Fluidqueue
bleibt leer. Ein Ofen und ein Raum sind kein Lasttest vieler Oefen/Auslaesse.
Einzelne Frame-Spitzen bleiben vorhanden.

Die Aufnahme startet mit echten 3840x2160; wie beim Flug meldet die Enddiagnose
3840x2124 im Fenstermodus. Zeitpunkt der Groessenaenderung unbekannt, kein
pixelidentischer A/B-Nachweis. RSS steigt um rund 102 MiB; Peak-RSS 1376,37 MiB,
Peak-DRM-VRAM 4233,25 MiB. Kein Nachweis allgemeiner Speicherparitaet.

`smoke-after.log` ist der vorherige randlose Vollbildpilot: nach ungefaehr
zwei Minuten mit SIGTERM/Exit 143 beendet, ohne Panic-/Renderfehler und ohne
regulaere Benchmark-Zusammenfassung. Der Signalabsender ist nicht geklaert.
Dieser Pilot wird weder als Spielabsturz noch als erfolgreicher Langzeittest
gewertet. Der Wiederholungslauf verwendet einen neuen Klon des leeren
Ausgangssaves, keine uebernommenen Rauchzellen aus dem abgebrochenen Versuch.

## Reproduktion

```bash
bash tools/benchmarks/run-soak.sh target/debug/terra-client after-idle idle
bash tools/benchmarks/run-soak.sh target/debug/terra-client after-30m delayed
bash tools/benchmarks/run-soak.sh target/debug/terra-client after-edits edits
```

`delayed` bleibt 15 Minuten stationaer und fliegt danach ueber echte konfigurierte
Spielereingaben 600 m hin und zurueck, mit Serverbestaetigungen und 120-s-Zyklus.
Es handelt sich nicht um eine vom Spieler getrennte Kamerafahrt. `edits` fuehrt
endliche, bestaetigte Quell-/Blockwechsel aus und wartet auf echten Fluss und
Rueckfluss. Nur in isolierten Testwelten verwenden: Wasser kann echte Props entfernen.

`TERRA_SOAK_CONFIG_DIR`, `TERRA_SOAK_SECONDS`, `TERRA_SOAK_WEATHER` uebersteuern
die Testparameter ausdruecklich. Persistente Tests verlangen einen absoluten
`TERRA_BENCH_SAVE_DIR`. Fuer kopierte bestehende Welten kommen
`TERRA_BENCH_WORLD_ID` und ein isoliertes `TERRA_WORLD_DIR` hinzu; das bestehende
WorldLibrary-System validiert ID, Version, Seed und kompletten Generatorvertrag.
Keine automatische Ueberschreibung vorhandener Welten oder anderer Seeds.

`summarize_soak.py` liefert echte 30-s-Fenster und Minutenpruefpunkte innerhalb
von +/-5 s samt tatsaechlicher Messzeit. Fehlende Werte bleiben fehlend, ohne
Interpolation. Ressourcen werden nur mit belegbarer Zeitausrichtung verbunden.

## Validierung

- `cargo fmt --all --check`: bestanden (`fmt-final.log`).
- `cargo check --locked --workspace`: bestanden (`check-with-fixture.log`).
- `cargo test --locked --workspace --no-fail-fast`: 1480 bestanden,
  0 fehlgeschlagen, 2 ignoriert (`tests-with-fixture.log`, erneuter Gesamtlauf).
- Zwei explizite `terra-storage --example furnace_soak_fixture`-Tests bestanden
  (`furnace-fixture-tests.log`); der Generatorlauf prueft zusaetzlich den echten
  Spawn und den binaeren Speicher-Roundtrip (`furnace-fixture-prepare.log`).
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: bestanden
  (`clippy-with-fixture.log`).
- `cargo build --locked --workspace`: bestanden (`build-with-fixture.log`).
- Dedicated Server ohne `DISPLAY` und `WAYLAND_DISPLAY`: gestartet, 525 Sections,
  sauber beendet (`headless.log`).
- Der zusaetzlich explizit ausgefuehrte ignorierte Wasser-Mikrobenchmark ist
  bestanden; der vorhandene dreiminuetige UDP-Soak wurde nicht erneut ausgefuehrt.
- Native 30-Minuten-/Save-/Interaktions-/Rauchraumgegenproben bestanden wie oben.
- Zehn Python-Tests fuer Messparser, Timeline und Zeitzuordnung bestanden.
- Zwei verworfene Messpiloten sind separat dokumentiert: Eine zu lange Tageslaenge
  verhinderte Wetterbereitschaft; ein Zugriff auf ausgelagerte Mesh-Vertexdaten
  liess die neue Telemetrie abbrechen. Die Telemetrie verwendet jetzt `try_attribute`
  und hat dafuer einen Regressionstest. Keiner der Piloten gilt als Langzeitmessung.

## Isolierter KI-Abfragevergleich

`water-needs-micro-workspace.log`: direkt mit dem bereits erfolgreich gebauten
Workspace-Testbinary, identische Koordinaten und Fluessigkeitsergebnisse,
abwechselnde Messreihenfolge, 16 Wiederholungen pro Radius. Optimiertes Testprofil.

| Suchradius | Proben je Suche | Bisher, 16 Suchen | Vorbereitete Spalten, 16 Suchen |
| --- | ---: | ---: | ---: |
| 2 m | 64 | 0,657173 ms | 0,037502 ms |
| 6 m | 192 | 1,995655 ms | 0,113952 ms |

Das sind rund 17,5-mal schnellere Fluessigkeitsabfragen in dieser begrenzten
Mikroprobe, **kein** entsprechender FPS-Gewinn und kein alleiniger Nachweis der
gemeldeten 18-FPS-Ursache. Ein vorheriger Package-only-Aufruf ohne das optionale
Bevy-Feature fuehrte null passende Tests aus (`water-needs-micro.log`); er wird
nicht als Benchmark gewertet.

Die anfaenglichen Gesamttests fanden eine Namensmehrdeutigkeit in drei neuen
KI-Tests und zwei falsche neue Testannahmen: Bevy behaelt Registrierungs-Entities
nach einem Fade, und `set_fluid(amount=0)` entfernt den Override und kehrt zum
natuerlichen Wasser zurueck. Diese Tests pruefen nun den tatsaechlichen Vertrag;
der Produktionsvertrag wurde dafuer nicht geaendert. Der erste Clippy-Lauf fand
einen unnoetigen finalen Option-Reborrow, der entfernt wurde. Initiale Fehlerlogs
bleiben erhalten; nur die erfolgreichen finalen Laeufe gelten als Abnahme.

## Lifecycle-Abdeckung Und Grenzen

Die Replica-Regression vergleicht 100 Importe an negativen Section-Grenzen
(Fluidquelle -> Stein -> Luft) mit normaler autoritativer Wiederaufnahme:
gleiche Daten, Quellen, Revisionen und Terrain-/Wasser-Halo-Epochen, aber keine
ungenutzten Client-Fluid-/Grasauftraege. 100 echte `receive_jobs`-Reloads pro
Replica-/Serverrolle pruefen das erneute Veroeffentlichen: Quellen bleiben
erhalten, nur die Autoritaet wacht zur Simulation auf.

Die Renderer-Lifecycle-Tests pruefen 100 Terrainzyklen ueber drei LODs,
50 Propzyklen ueber drei LODs und vier Arten, 100 Wasserzyklen ueber zwei
Fuellstaende und jeweils 100 Fadezyklen fuer Standard-/Prop-/Wet-/Water-
Materialien. Alte eigene Mesh-IDs, temporaere Materialien und Retire-Entities
muessen verschwinden; gemeinsame Handles bleiben begrenzt und unveraendert.
Diese Headless-Tests beweisen nicht die GPU-Treiberresidenz.

Im laufenden Nachhertest stieg DRM-VRAM einmal um etwa 512 MiB und spaeter um
rund 36 MiB, blieb danach mehrere Minuten bei 4599292 KiB und sank waehrend
eines Rueckflugs auf 4486916 KiB. Bevy haelt Kapazitaet nichtleerer Mesh-Slabs
vor; ohne Slab-spezifische Zaehlung ist das keine belegte Zuordnung des Sprungs.
Insbesondere wird keine VRAM-Paritaet zum Vorherlauf behauptet.

Gesamt-GPU-Framezeit, vollstaendige GPU-Subdrawzahl, eigene Zeiten des gesamten
Main-/Render-Threads und GPU-Uploadbytes sind hier nicht zuverlaessig gemessen.
Vorhandene Passdiagnostik, Uploadvertices, Scopeintervalle und Prozess-CPU sind
getrennt ausgewiesen; verschachtelte Scopes werden nicht addiert.
Uploadvertices/-frames zaehlen die instrumentierten Terrain-/Prop-/Wasser-
Budgets, nicht saemtliche Feuer-/Raucheffekt-Uploads. Null in diesem Feld
bedeutet deshalb nicht, dass animiertes Feuer und Rauch kein GPU-Update haben.
CPU-Meshvertices zaehlen nur noch lesbare Assets, nicht bereits ausgelagerte
GPU-only-Terrainpuffer.

Fuer den neuen geschlossenen Rauchraum existiert ausserdem der explizite
Offline-Fixturegenerator `terra-storage/examples/furnace_soak_fixture.rs`.
Er verwendet normale Regionen, WorldLibrary, Ofenregeln und Spielerdaten,
keine zweite Simulation. Ein Raum mit 292 zusammenhaengenden Luftzellen wird
um den verifizierten Weltspawn gebaut, mit regularem Brennstoff und ohne
Rauchvorfuellung. Der native 15-Minuten-Einzelofentest ist oben dokumentiert;
viele Oefen und offene Raum-/Rohrnetzwerke bleiben separate Lastfaelle.
