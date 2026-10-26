# Dorffelder, Textilien, Blumentoepfe und verbundene Fenster

## Implementierung

- F9-Linien: 3,51 statt 2,7 Bildschirmpixel (+30 %). Die eigene
  orange/gelbe Gizmo-Gruppe veraendert keine anderen Auswahllinien.
- Teppiche sind 1/8 Block hoch. Teppich und Wolle verwenden die 13 bestehenden
  Farbstofffarben. Nur weisser Teppich und weisse Wolle erscheinen im
  Too-Many-Items-Katalog; die Farbvarianten entstehen durch echte Rezepte.
  Vorhandene Block-IDs bleiben erhalten. Neue IDs: Teppich 106-118,
  gefaerbte Wolle 119-130, Blumentopf 131; weisse Wolle behaelt ID 103.
- Blumentoepfe nehmen Blumen und Pilze ueber die normale Platzieren-Taste auf.
  Leere Hand nimmt den Inhalt zurueck. Inventarverbrauch, Reichweite,
  Sichtpruefung, Revisionen, Abbau-Drops und Speicherung laufen autoritativ
  ueber die bestehenden Systeme. Maximal zehn Zustandsvarianten verwenden
  vorhandene Pflanzengeometrie statt neuer Entities pro Pflanzenteil.
- Gleichartige Fenster mit gleicher Wandorientierung verbinden sich zu
  1x2-, 2x1- und 2x2-Gruppen. Hoehere Gruppen behalten eine durchgehende
  Einfassung; breite Gruppen haben zwei gegenlaeufige Fluegel mit aeusseren
  Scharnieren. Darstellung, Animation, Kollision und Picking teilen dieselbe
  begrenzte Gruppengeometrie. Oeffnen ist eine atomare, kollisionsgepruefte
  Aktion, auch an Section-Grenzen. Gemischte vorhandene Offen-/Geschlossen-
  Zustaende werden nicht ungefragt umgeschrieben.

## Gezielte Feldkorrekturen

1. Fortschritt innerhalb derselben Wachstumsstufe aendert den gespeicherten
   Pflanzenzustand, aber nicht mehr faelschlich die gesamte geladene Welt.
   Ein Stufenwechsel gleicht Props gesammelt einmal je betroffener Section ab,
   statt einmal je Pflanze. Bewaesserte unveraenderte Nachtkulturen erzeugen
   ueberhaupt keine falsche Change Detection.
2. Reine Wasser-/Terrain-Replikate ohne neue Prop-Epoche bauen die vorhandene
   Pflanzenliste nicht erneut auf, wenn ihr Inhalt bereits zum Journal passt.
   Nach Section-Austausch werden auch bei gleicher Epoche fehlende Pflanzen,
   Wachstumsvarianten und gespeicherte Entfernungen wieder korrekt abgeglichen.
   Wirkliche Aussaat, Ernte und Stufenwechsel werden weiterhin uebernommen.
3. Generierte Dorfkanaele erhalten echte, volle Wasserquellen statt lediglich
   eines Blocks mit Wasser-Material-ID. Eine eng begrenzte Reparatur erkennt
   nur den alten unvollstaendigen Generierungszustand. Explizite Spieler-
   Aenderungen, entfernte Quellen und gespeicherte Fluessigkeitsdaten bleiben
   erhalten. Wakeups und Nachbar-Mesh-Aktualisierungen werden zusammengefasst.

Ein gezielter CPU-Test vergleicht 1000 reine Wasser-Deltas bei 64 reifen
Kulturen mit einem Referenzpfad, der jedes Mal einen vollen Prop-Abgleich
erzwingt: Median dreier Durchlaeufe **17,628 ms mit erzwungenem Vollabgleich,
6,753 ms mit dem finalen Hydrierungs-Guard**. Das sind Zeiten fuer die gesamte
Abfragemenge, keine FPS-Hochrechnung und kein Vergleich zweier alter/neuer
Executables. Die endgueltigen Props beider Pfade werden auf Gleichheit geprueft.

Die genaue Pflanzenauswahl wurde ebenfalls gemessen: ein reifes Weizenmodell
hat 3888 Vertices/1944 Dreiecke. Die Zielsuche im realen Feld kostet hier
median 0,209 ms, der Aufbau seiner Auswahlkanten 0,350 ms. Das ist kein
Beleg, dass die Pflanzenauswahl allein einen Verlust von etwa 20 ms verursacht.

## Wasserfall-Terrain und bestehende Welten

Neue Konfigurationen verwenden `river_terrain_revision = 1`. Fehlt dieses Feld
in einer gespeicherten Welt, bleibt deren bisheriger Wert 0 erhalten.
Weltidentitaet, Block-IDs, Generator-/Speicherformatversion werden nicht
stillschweigend geaendert. Wegen des zusaetzlichen replizierten Config-Felds
steigt die Netzwerk-Protokollversion auf 73; Client und Server muessen passen.

Revision 1 erweitert das Terrain um wirkliche Hoehenstufen eines Flusses zu
einem asymmetrischen Felskessel mit abgestuften Uebergaengen. Die alte starke
Dach-Erhaltung wird dort lokal geloest, waehrend tief vergrabene Fluesse ihr
Dach behalten. Nur tatsaechlich angeschnittene Steilbaenke verlieren die dicke
Erdhaut. Wasserbreite, Gefaelle, Quellen und echte Fluessigkeitslogik bleiben
dieselben; keine dekorative Wasserfall-Plane ersetzt die Simulation.

Die native Vergleichsszene bei Seed 42042, `[-424,614]`, ist tatsaechlich
aufgenommen und angesehen: Revision 0 verdeckt den tieferliegenden Wasserfall
mit der unveraenderten Bergflanke; Revision 1 oeffnet den abgestuften Felskessel
und legt Wasserfall und unteres Becken frei. Kamera `[-466,85,656]`, Blickziel
`[-424,32,614]`. Isolierte Konfigurationen `waterfall-before/` und
`waterfall-after/` schalten Fluesse explizit ein, Baeume/Hoehlen/Strukturen aus.
Die vier Geometrietests bestehen fuer Seeds 42042, 1337 und 99173, einschliesslich
negativer Koordinaten und exakter unveraenderter Wasser-/Gefaelleparameter.

## Nachgewiesener Dorf-Engpass

Die Feldkorrekturen allein loesen die niedrige stationaere Dorf-Framerate nicht.
Der kontrollierte Vergleich isoliert stattdessen die Fackel-Cubemap-Schatten:

| Identische Feldansicht, je 20 Sekunden | FPS | p95 | p99 | Render-Schedule-Wallzeit |
| --- | ---: | ---: | ---: | ---: |
| Vorher, GPU-Diagnostik aus | 42,34 | 28,552 ms | 32,485 ms | nicht instrumentiert |
| Feldkorrekturen, Fackelschatten unveraendert | 43,33 | 27,953 ms | 31,073 ms | 22,351 ms |
| Finaler Fix: nur verifiziert sonnenbeschienene Fackel-Cubemaps entfallen | 55,98 | 22,103 ms | 26,466 ms | 17,167 ms |
| Diagnose: nur Fackelschatten voruebergehend aus | 115,76 | 11,780 ms | 14,634 ms | 7,801 ms |

Im Feld sind fuenf echte Punktlichter vorhanden. Der finale Fix laesst die
Schatten der drei ueberdachten Quellen an und spart zwei offen sonnenbeschienene
Cubemaps. Die Shadow-Draw-API-Aufrufe fallen von 304 auf 209. Die Framerate
steigt gegenueber den Feldkorrekturen um rund 29 %, nicht auf den Diagnoswert.
Die neue Sichtbarkeitspruefung kostet gemittelt 0,024 ms pro Frame; ihr Cache
ist durch das bestehende Lichtbudget begrenzt. Kein dauerhaftes Wachstum der
Welt- oder Asset-Caches wird dafuer eingefuehrt.

Die Produktionspruefung verwendet die wirkliche Sonnenrichtung, bekannte
Gelande-/Baum-/Strukturhoehen, geladene Bloecke und Spieler-Dach-Aenderungen.
Unbekannte benoetigte Daten, Hoehlen-/Unterwasserzustand, Nacht und schwache
Sonne behalten Schatten. Echte Terrain-Aenderungen und Streaming invalidieren
abhaengige Strahlen; reine Wasser- oder Prop-Pakete tun dies nicht unnoetig.

Grenze: Die Optimierung beurteilt Lichtquellen, nicht jeden moeglichen
Empfaenger in deren Reichweite. Angrenzende Gebaeudeinnenraeume sind damit
nicht vollstaendig raumweise klassifiziert. Drei weiterhin notwendige lokale
Schatten-Cubemaps verursachen in dieser Szene noch erhebliche Renderkosten;
Nacht-/Innenraum-Performance ist damit nicht pauschal geloest.

Die gemessenen Shadow-Draw-API-Aufrufe fallen im isolierten Diagnoselauf von
304 auf 64; Sonnen-/Mondschatten, Lichtanzahl, Lichtreichweite, Intensitaet,
Sichtweite und Vegetation bleiben gleich. Die Diagnose-JSON-Aenderung ist danach
wieder entfernt worden. Sie ist **kein** pauschaler Produktionsfix.

Alle Vergleichslaeufe verwenden dieselbe Startposition und dieselben Optionen.
Die physikalisch eingeschwungene Kamerahoehe weicht um etwa drei Zentimeter ab;
dies sind keine pixelidentischen Standbilder. Der finale Lauf hat 5781 geladene
Sections, 2785 gerenderte Sections, 12187 Mesh-Assets, 64557 Props, 1651
Prop-Batches und 44 Wasser-Tiles bei 1871444 KiB Prozess-RSS. Eine einzelne
kurze Aufnahme ist kein Langzeit-Leak-Nachweis.

Bevys vorhandene GPU-Passdiagnostik instrumentiert diese Point-Shadow-Passes
nicht. Die zuvor gemessenen niedrigen Main-Pass-Zeiten belegen deshalb keine
niedrige GPU-Gesamtlast. Zusaetzliche opt-in Wallzeitmessung trennt echtes Main,
Render, Extract und den Render-Handoff; diese parallelen Intervalle duerfen
nicht addiert oder mit OS-CPU-Busy-Zeit gleichgesetzt werden.

## Reproduktion

Die beigefuegte Konfiguration verwendet Seed 42042, Sichtweite 12, 1280x800,
VSync aus und unbegrenzte FPS. Das echte Dorf in Region `[2,-2]` hat sein Feld
bei `[970,71,-565]..[984,77,-555]`, Bodenhoehe 73. Es enthaelt 28 Weizen-,
14 Mais- und 28 Salatpflanzen sowie sieben Wasserquellen.

```bash
TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/village-fields-decor-2026-10-25/configs" \
TERRA_MENU_CONFIG_DIR="$PWD/target/phase124-village-menu" \
TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 \
TERRA_BENCH_CAMERA=974,74.8,-557 TERRA_BENCH_LOOK_AT=974,74.1,-562 \
TERRA_BENCH_SECONDS=20 TERRA_CPU_SCOPES=1 WGPU_BACKEND=vulkan \
target/debug/terra-client --smoke-test-benchmark
```

Nicht als Vergleich verwenden: `farm-before.log` und
`farm-integrated-diagnostic.log` trafen eine ungueltige Teleport-Hoehe und
erreichten die Benchmark-Bereitschaft nicht. Der Logger verweigert dann
korrekterweise die FPS-Auswertung. Der Offline-Fernblick und der HUD-lose
Kontrolllauf haben andere Aufloesungen und sind keine identischen Szenen.

## Pruefstand

2416 regulaere Workspace-Tests bestanden, null Fehler, sieben opt-in Tests
regulaer ignoriert. Darunter sind 651 World-Tests, die oben genannten
Hydrierungs-/Quellen-/Fenster-/Terrainfaelle, Topf-Transaktionen und Persistenz
sowie die Schatten-Invalidierungs- und Streaming-Regressionstests.
Der oben ausgewiesene Crop-Replikationsbenchmark wurde zusaetzlich explizit
erfolgreich ausgefuehrt.
Die native Fenstergalerie besteht fuer drei Holzarten und alle drei Gruppenformen,
inklusive gegenlaeufiger Oeffnung, Materialpixeln, Section-Uebergang und echtem
Aufteilen nach dem Entfernen eines Fensters. Aufnahmen unter `native/windows/`.
Client und Dedicated Server bauen mit `cargo build --locked --workspace`.
Der finale Dedicated-Server-Smoke-Test besteht headless mit 27 Sections.

Die native Materialgalerie besteht mit allen 131 Block-IDs und GPU-Previews.
Die Aufnahme `native/materials-textiles-flower-pots.png` wurde angesehen:
farbige Wolle und 1/8-Teppiche sowie drei echte Blumen- und eine Pilzfuellung
verwenden die gemeinsame Block-/Prop-Geometrie. Alle 14 Galerieansichten
wurden ohne Shaderfehler aufgenommen.

Formatpruefung, `cargo check --locked --workspace`,
`cargo test --locked --workspace --no-fail-fast`,
`cargo clippy --locked --workspace --all-targets -- -D warnings` und
`cargo build --locked --workspace` sind erfolgreich. Logs liegen neben diesem
Bericht. Die native Materialgalerie und der finale Dorfvergleich enthalten
keine Warnungen oder Fehler im Log.

Wie gewuenscht kein weiterer 10-/15-/30-Minuten-Dauertest. Kein allgemeiner
oder langfristiger FPS-Fix wird aus den kurzen isolierten Messungen abgeleitet.
Die eigene temporaere 1,3-GB-Vergleichskopie des alten Clients ist entfernt;
Spielstaende und fremde Build-Dateien wurden dabei nicht geloescht.
