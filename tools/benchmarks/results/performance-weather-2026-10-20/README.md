# Phase 85: Hohe Sichtweiten, Wasser Und Wetter

## Bedingungen Und Grenzen

Hardware: Ryzen 9 5900X, Radeon RX 6950 XT / RADV, Vulkan, 32 GiB RAM.
Keine parallelen Compiler oder Spielclients waehrend der nativen Messungen.
Die Desktop-Sitzung bleibt unveraendert. Zeitmessung mit
`TERRA_CPU_SCOPES=1`, GPU-Passmessungen mit `TERRA_RENDER_DIAGNOSTICS=1`.
Einzelne GPU-Passzeiten sind keine gemessene gesamte GPU-Framezeit.

Der bisherige Client ist vor Aenderungen unter
`target/phase85/terra-client-before` gesichert; Shader/Assets unter
`target/phase85/before-assets`. Kein Dependency-Upgrade, keine
Generatoraenderung, keine gekuerzte Sichtweite oder reduzierte Prop-Dichte.

Die bisherige Benchmark-Implementierung ignorierte `graphics.toml`.
Zudem uebersteuert das autoritative `WorldWeather` das alte Wetterfeld in
`client.toml`. Die sechs Baseline-Szenarien sind deshalb ausdruecklich
**Klima-Laeufe**, nicht garantierter Regen oder Schnee, auch wenn die
Ordnernamen die geplanten Wetterfaelle benennen.

Die Vergleichsmatrix nutzt identische Konfigurationen und jeweils 60 s
denselben LOD-Flug, ohne Chunk-Animationen. Ein vorheriger Rise-Test blieb
in der initialen Draw-Bestaetigung haengen und wurde abgebrochen
(`user20-before-rise-readiness.log`), nicht als fertige Messung gewertet.
Die echte physische Fenstergroesse ist aus jedem Log zu uebernehmen:
Wayland begrenzt das Testfenster auf den verfuegbaren Desktopbereich,
nicht exakt 3840x2160. Der erste Lauf hat 3840x2055, die folgenden
3828x2043. Echte 4K-Abnahmen verwenden anschliessend Borderless.

```bash
bash tools/benchmarks/results/performance-weather-2026-10-20/run-matrix.sh \
  target/phase85/terra-client-before before
```

Fuer eine Wiederholung des alten Builds nach Shaderaenderungen ist zusaetzlich
`TERRA_ASSET_DIR=target/phase85/before-assets` zu setzen. Die hier erfassten
Baseline-Laeufe fanden vor den Shaderaenderungen statt.

## Implementierte Korrekturen

- Terrain-Metadaten und Prop-Auftragslisten werden nur bei passenden
  Aenderungen neu bestimmt. Budgetwarteframes durchsuchen nicht wieder
  saemtliche geladenen Sections. Veraltete Worker-Ergebnisse erhalten
  weiterhin einen Retry, auch bei anschliessend stillstehender Kamera.
- Ankunftsbedingte Auftragssuchen unterscheiden sich von vollstaendiger
  LOD-Neuselektion. Bereits wartende LOD-Arbeit bleibt dabei erhalten.
  Kuestenbatches pruefen weiterhin neu verfuegbare Nachbarspalten.
- Mikrorelief, Prop-LOD und Render-Auftragsprioritaet verwenden den Abstand
  zum dreidimensionalen Section-AABB statt nur X/Z. Vorher bekamen auch
  weit unter einer fliegenden Kamera liegende Details die Nahgeometrie.
  Terrain-Grund-LOD bleibt spaltenbasiert, alle Entfernungsgrenzen bleiben
  unveraendert. Rein vertikale Kamerabewegung invalidiert jetzt die Auswahl.
  Die visuelle Neuselektion nutzt 4-m-Zellen innerhalb der bestehenden
  Mikrodetail-Hysterese; Streaming und Kollision verwenden weiterhin den
  unquantisierten Fokus. Edits und neue Sections warten nicht darauf.
- Die Kartografie indiziert geladene Spalten einmal pro Scheduler-Takt.
  Bis zu vier Detail-Snapshots nutzen denselben unveraenderlichen Index,
  statt die gesamte geladene Welt fuer jeden Snapshot erneut zu durchsuchen.
  Tests vergleichen Oberflaechen-/Hoehlenkarten, Edits und negative Koordinaten
  mit dem bisherigen direkten Snapshot-Pfad.
- Unload-Suchen und Prop-Gesamtzaehlungen laufen nur bei Aenderungen,
  nicht fuer jede aktive Worker-Warteschlange.
- Wasser-Uploads behalten ihre budgetierte Kandidatenliste.
- Lichtquellen erhalten eine kompakte Emitterliste. Unveraenderte leere
  Sections werden nicht pro Frame erneut fuer Fackel-Audio durchsucht.
- Wasser-Audio haelt maximal vier Abfrageergebnisse fuer unveraenderte
  Voxelpositionen; Chunk-/Journal-/Terrainwechsel invalidieren sie.
  Lautstaerke, Panning und Unterwasserfilter bleiben frameaktuell.
- Windmaterialien werden nur bei geaenderten Werten mutiert.
  `Assets::iter_mut` erzeugte zuvor auch bei gleichen Werten Modified-
  Events und damit erneute Materialaufbereitung.
- Der Fluessigkeitsmesher gibt fuer vollstaendig verdeckte Wasserzellen
  kein leeres Mesh mehr zurueck. Terrain-, Prop- und Wasser-Uploads
  weisen leere Vertex-/Indexpuffer zusaetzlich ab und entfernen dabei
  korrekt die vorherige Darstellung.
- Der Wassershader ueberspringt wirkungslose Ufer-/Sturmrechnungen.
  Suchschritte, Reichweite und Qualitaetseinstellungen bleiben erhalten.
- Native Single-Sample-Wasseransichten koennen Tiefenwerte mit dem bereits
  gebundenen linearen Clamp-Sampler filtern, statt pro Schritt vier
  Tiefen-Texel manuell zu laden. Bevy-Raytraversierung, naechster Tiefenwert,
  Suchschritte und Trefferpruefung bleiben unveraendert. Die Shaderbibliothek
  wird lokal erweitert; Bevy selbst wird nicht global ueberschrieben.
  MSAA, Web und unbekannte Bibliotheksfassungen behalten den bisherigen Pfad.
  `TERRA_BENCH_WATER_SOFTWARE_DEPTH=1` erzwingt diesen nur in Smoke-Tests
  fuer ein isoliertes A/B-Paar mit derselben Binary.
- Chunk-Animationen ignorieren Schatten-Proxys bei der Color-Draw-
  Sichtbarkeitspruefung, genau wie beim Start der Animation. Ein sichtbarer
  Shadow-AABB konnte sonst eine endlose Draw-Warteschleife ausloesen,
  obwohl saemtliche eigentlichen Farb-Batches ausserhalb des Bildes lagen.

## Slab-Allocator Und Terminal

Der konkrete Wasserfehler: `fluid_mesh` legte bereits vor der
Sichtbarkeitspruefung einen leeren `WaterPatch` an. Waren alle Faces
verdeckt, wurde `Some(empty)` trotzdem hochgeladen. Das fuehrt im
vorhandenen Bevy-Allocator zu dem bekannten Fehlerpfad fuer leere Meshes
([Bevy #24874](https://github.com/bevyengine/bevy/issues/24874)).
Regressionstests decken vollstaendig verdecktes Wasser an positiven und
negativen Section-Grenzen sowie den Austausch vorhandener Meshes ab.

Die Calloop-Warnung ist getrennt davon: Bevy filtert diese bekannte
Wayland-Meldung bereits in seinem Standardfilter
([Bevy #21548](https://github.com/bevyengine/bevy/pull/21548)).
Unser `info`-Filter ersetzte die Standardfilter vollstaendig.
Der Client behaelt diese nun bei, mit expliziten Benutzerfiltern als
nachfolgendem Override. Renderer-Fehler werden **nicht** ausgeblendet.
Dies ist eine Log-Policy-Korrektur, kein behaupteter Performance-Fix.

## Reproduzierbare Echte Wettertests

- `TERRA_BENCH_GRAPHICS=1`: validiert und laedt die echte Grafikdatei,
  einschliesslich Schattenqualitaet, TAA, Helligkeit und Cave-Qualitaet.
- `TERRA_BENCH_WEATHER=rain`, `snow 3`, `storm` oder `clear`: nutzt das
  vorhandene WorldWeather. Im integrierten Server werden echte
  `/weather set`-Kommandos gesendet. Messbeginn wartet auf das
  replizierte Wetter und die tatsaechlich gerenderten Partikel.
- `TERRA_BENCH_CAMERA=x,y,z` und `TERRA_BENCH_LOOK_AT=x,y,z`: validierte
  reproduzierbare Ansichten. Netzwerkansichten verwenden autoritatives
  Teleportieren statt rein lokaler Kameraverschiebung.
- `TERRA_BENCH_MODE=integrated --smoke-test-benchmark`: echter
  integrierter Server. Der bisherige reine Kamera-LOD-Flug ist kein
  autoritativer Mehrspielerflug und wird nicht als solcher etikettiert.

Logs enthalten effektive Grafikparameter, physische Aufloesung,
autoritativen Servermodus, Wetterzustand und Partikelstatus.

`TERRA_BENCH_UI=1 TERRA_BENCH_MODE=integrated --smoke-test-benchmark`
aktiviert zusaetzlich das echte Spiel-HUD, die Minimap und den Menueablauf.
Dieser Lauf wartet auf eine echte Serververbindung und auf die sichtbare UI.
Die bisherige rein visuelle Kamerafahrt ist kein Ersatz fuer diesen Test.

## Nicht Uebernommener IBL-Versuch

Die atmosphaerische Licht-Cubemap wurde separat mit 512 und 128 Texeln je
Kante verglichen. Dies betrifft nicht die sichtbare Skybox. Hintergrund:
[Bevy #24517](https://github.com/bevyengine/bevy/issues/24517) dokumentiert
hohe Kosten der pro Frame gefilterten Umgebungskarten.

Ein erster ruhender 4K-Regenlauf zeigte 67.11 -> 71.77 FPS und
p99 24.769 -> 22.072 ms. Die sechs Szenarien bestaetigten aber keinen
allgemeinen Vorteil: Regen/20 sank von 53.59 auf 50.94 FPS, p99 stieg
28.295 -> 30.781 ms; Schnee/20 sank 62.08 -> 60.08 FPS.
**Die 128er-Aenderung wurde zurueckgenommen. Produktion bleibt bei 512.**
Die historischen Lognamen `*-final128.log` bezeichnen diesen verworfenen
Versuch, nicht den finalen Quellstand. `run-paired-weather.sh` dokumentiert
seine damaligen Vergleichsbedingungen.

## Isolierter Wasservergleich

Dieselbe Binary, derselbe 4K-/30-s-Flug, Seed 12011991, Sichtweite 12,
Clear, High-Wasser, Rise und 512er-Umgebungslicht:

| Tiefenfilter | FPS | p95 ms | p99 ms | GPU-Wasserpass ms |
|---|---:|---:|---:|---:|
| Software | 82.66 | 15.325 | 18.171 | 3.332 |
| Hardware | 82.46 | 15.531 | 18.176 | 3.029 |

Der Wasserpass wird in diesem Paar rund 9.1% guenstiger, die Gesamt-FPS
verbessern sich **nicht messbar**. Keine daraus erfundene Gesamt-GPU-Zeit.
Beide Shaderpfade kompilierten nativ und liefen ohne Rendererfehler.
Rohdaten: `water12-depth-software.log`, `water12-depth-hardware.log`.
Der Software-Rueckfall registriert ebenfalls den Shaderimport: Bevy loest
Imports vor der bedingten Shaderauswahl auf. Ohne Alias konnten Pipelines
statt eines Shaderfehlers endlos im Wartezustand bleiben.

## Zwischenstaende

`intermediate-analysis.md` behaelt die erste, weitgehend wirkungslose
Cache-Matrix bei. Ihre FPS und p99 werden nicht nachtraeglich als Erfolg
ausgegeben. Die nach dem IDE-Neustart aufgenommenen Fensterlaeufe haben
teilweise andere physische Aufloesungen und sind keine exakten 4K-Paare.

Auch `*-final3d.log` ist ein Zwischenstand: 3D-Abstaende mit 1-m-
Invalidierungszellen. Schnee/20 verlor dort FPS, waehrend der Terrain-
Dispatcher haeufigere Komplettsuchen ausfuehrte. Der finale Stand
verwendet deshalb die oben beschriebenen 4-m-Zellen. Diese unterscheiden
nur die Frequenz visueller LOD-Pruefungen, nicht die Weltgenerierung.

## Finale 4K-Wettermatrix

Jeweils 30 s derselbe 896-m-Hin-/Rueckflug, Borderless 3840x2160,
VSync aus, FPS unbegrenzt, Schatten High, Wasser High mit Reflexionen,
Wetter Ultra, Rise-Animation. Drei Seeds:
Wasser 12011991, Regen 2262437629133911810, Schnee 8675309.
Die Prop-Anzahlen bleiben identisch; keine Reduktion der Weltbevoelkerung.

**Vergleichsbasis ist der Zwischenstand `light512`, nicht der urspruengliche
Phase-84-Client.** Beide enthalten bereits die Cache- und Wasserfixes.
Hier verglichen werden die letzten 3D-Detail-/Invalidierungskorrekturen.
Der urspruengliche Benchmark ignorierte Grafikeinstellungen und ist deshalb
nicht als exaktes 4K-Vorherpaar geeignet.

| Szene / Sichtweite | FPS vorher -> final | p95 ms vorher -> final | p99 ms vorher -> final | RSS MiB vorher -> final |
|---|---:|---:|---:|---:|
| Wasser / 12 | 82.95 -> 90.57 | 15.69 -> 14.15 | 18.64 -> 16.47 | 1102 -> 910 |
| Wasser / 20 | 53.55 -> 52.54 | 24.97 -> 25.37 | 29.70 -> 28.40 | 1514 -> 1371 |
| Regen / 12 | 71.87 -> 77.47 | 19.24 -> 18.95 | 23.16 -> 22.29 | 1168 -> 1116 |
| Regen / 20 | 53.59 -> 54.54 | 24.05 -> 24.40 | 28.29 -> 29.45 | 1773 -> 1593 |
| Schnee / 12 | 81.43 -> 61.64 | 17.25 -> 20.29 | 20.13 -> 22.45 | 1399 -> 1272 |
| Schnee / 20 | 62.08 -> 61.35 | 22.55 -> 22.57 | 27.19 -> 25.65 | 1820 -> 1763 |

Rohdaten: `logs/*-light512.log`, `logs/*-final85.log`. Alle sechs finalen
Laeufe beendeten Flug und Meshwarteschlangen ohne ERROR. RSS ist der
Prozesswert am Messende, kein nachgewiesener gesamter RAM-/VRAM-Peak.
Erstladezeiten final: 10.72/25.02 s Wasser, 11.26/26.20 s Regen,
10.01/23.25 s Schnee (jeweils Sichtweite 12/20).

Der schlechte Schnee/12-Erstlauf bleibt bewusst enthalten. Ohne Code-
oder Konfigurationsaenderung ergab die Wiederholung 86.14 FPS, p95 16.69,
p99 19.71 ms; der unmittelbar anschliessende Vergleichsstand erreichte
84.75 FPS, p95 17.23, p99 19.51 ms. Logs:
`snow12-final85-repeat.log`, `snow12-light512-repeat.log`.
Die Ursache der Laufstreuung ist nicht abschliessend isoliert. Kein
Cherry-Picking des besseren Wiederholungslaufs als einziges Ergebnis.

Ein echter integrierter Server mit HUD/Minimap, Regen und Sichtweite 20
erreicht im ruhenden 4K-Lauf weiterhin nur etwa 67 FPS
(`user20-hud-final3d.log`: 67.67 FPS, p95 21.754, p99 24.049 ms).
Dieser Lauf nutzt den 1-m-Zwischenstand; ein unbewegter Fokus misst
nicht die spaetere 4-m-Flugoptimierung. Der uninstrumentierte Vergleich
erreicht ebenfalls nur 67.47 FPS; Profiling allein erklaert die Last nicht.

**Das Gesamt-FPS-Problem ist nicht vollstaendig geloest.** Besonders
Sichtweite 20 bei 4K bleibt teuer; Regen-p99 verbessert sich nicht
durchgehend. Die nativen Kurzfluege sind keine mehrstuendige Multiplayer-
Streamingabnahme. Fuer weitere Arbeit sind vollstaendige GPU-Frame-Traces
einschliesslich Compute-/Umgebungslichtarbeit und lange autoritative
Fluege noetig. Einzelne Passdiagnosen ergeben keine gesamte GPU-Zeit.

## Native Funktionspruefung

- Sechs finale Vulkan-/4K-Fluege auf der RX 6950 XT, drei Seeds und
  klares Wetter/Regen/Schnee: Exit 0, keine ERROR-Meldung.
- Der integrierte Fluid-Test besteht mit der archivierten flachen
  `fluid-recession-mushrooms-2026-10-16/source-configs`-Testwelt:
  Source-Anker und FluidCell replizieren, 108 Nachbarzellen fliessen,
  fuenf individuell gespeicherte Props verschwinden. Der Server lehnt
  Quellenabbau ab; Ersetzen entfernt Quelle und nachfolgendes Restwasser.
  Zwei echte Screenshot-Readbacks, Exit 0, keine Rendererfehler:
  `logs/fluid-final85-fixture.log`, Bilder in `fluid-final85/`.
- Der erste Aufruf mit dem anderen `user12`-Gelaende-Seed wartete in
  Stufe 4 bis zum Timeout (`logs/fluid-final85.log`). Die Fixture setzt
  die flache, bewachsene Testwelt voraus; dieser Fehlversuch wird nicht
  als erfolgreicher Quellen-/Langzeittest ausgegeben.
- Dedicated Server mit entferntem `DISPLAY` und `WAYLAND_DISPLAY`,
  `--no-save --smoke-test`: 45 Sections, Exit 0.
  `logs/server-final85.log`; kein Weltstand geschrieben.
- Regen-HUD, Wasser-/Schneelandschaften und die finale Quellenaufnahme
  wurden anhand nativer Screenshots betrachtet. Keine vollstaendige
  Art-Abnahme oder Behebung aller historischen Grafikartefakte behauptet.

## Build-Speicher

Vorher rund 404 GiB in `target/`; Cargo meldet fuer die gezielte
Bereinigung der elf Workspace-Pakete 344576 entfernte Dateien /
357.4 GiB. Abhaengigkeits- und Toolchain-Caches bleiben erhalten.
Zusaetzlich wurden historische `target/debug/terra*-before*`-
Vergleichsbinaries entfernt. Welten, Assets und Benutzerkonfigurationen
wurden nicht geloescht. Nach abschliessendem Workspace-Build und Tests:
`du -sh target` meldet rund **80 GiB**, `~/.cargo` 2.2 GiB.
Rund 324 GiB weniger physische
Belegung im Buildordner; Cargo-Loeschzaehler und physisches `du` sind
unterschiedliche Messgroessen. Die vom Nutzer genannten 600 GB wurden
hier nicht gemessen.

## Abschliessende Rust-Pruefung

Alle folgenden Befehle erfolgreich, ohne Dependency-Upgrades:

```bash
cargo fmt --all --check
cargo check --locked --workspace
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace
```

1327 Tests bestanden, ein vorhandener 180-s-UDP-Soak-Test bleibt
standardmaessig ignoriert. Logs: `logs/{fmt,check,tests,clippy,build}-final85.log`.
Der erste Clippy-Durchlauf beanstandete einen vereinfachbaren Bool-Ausdruck
im Shader-Fallback; dieser wurde logisch aequivalent korrigiert und alle
Pruefungen danach erfolgreich abgeschlossen. Die nativen Messungen nutzen
denselben Laufzeitpfad vor dieser rein logischen Vereinfachung.
