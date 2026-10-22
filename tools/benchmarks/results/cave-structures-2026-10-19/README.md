# Phase 80: Hoehlenraeume, Seen Und Abbaubare Segmente

## Architektur

Die bestehende 192-m-Hoehlenregion und ihre drei CaveKind-Profile bleiben die
Grundlage. `generation/cave_structures.rs` bereitet eine deterministische
Raumoekologie pro horizontaler SurfaceColumn vor. Geschlossene, unregelmaessige
Becken und Terrassen erweitern das bestehende Dichtefeld, kein zweiter Mesher.

- Salzraeume bleiben auf die bisherige Cliffted-Desert-Auswahl mit separatem
  10%-Regionsticket begrenzt. Salzwaende sind feucht; sparsame Tropfen verwenden
  denselben budgetierten Partikelbatch wie der vorhandene Wasserspray.
- Tropfsteinraeume erhalten breite, gerippte Boden-Decken-Pfeiler und separate
  verjuengte, haengende oder stehende Spitzen. Freie Durchgaenge bleiben erhalten.
- Hoeher gelegene bepflanzte Raeume enthalten Grass-/Dirt-Flecken, kleine Baeume,
  vorhandene Busch-/Farn-/Gras-/Pilzprops, Deckenranken, Wurzeln und schwach
  leuchtende bernsteinfarbene Pilze. Manche besitzen einen natuerlichen Lichtschacht.
- Unter Highlands liegen Wasserfallraeume mit hoehenversetzten Becken und
  Eiskristallen. Allgemeine Steinraeume koennen ebenfalls einen See besitzen.

`WaterColumns` trennt Oberflaechen- und Hoehlenreservoirs an derselben X/Z-Stelle.
Die gleiche Belegung gilt fuer Solver, Quellen, Schwimmen, Stroemung, Meshes,
Unterwasserumgebung, Regenausschluss und Wasser-Foley. Seen und Fallvorhaenge
verwenden den bestehenden Channel-Mesher, Wellenshader, Schaum und Spruehnebel.
Gespeichert werden weiterhin nur die implizite Weltkonfiguration und echte
Zellaenderungen im vorhandenen Regionsjournal.

Seegras waechst in neuen Welten als einzeln abbaubare Metersegmente. Der obere
Teil kann geerntet werden; beim Schnitt darunter werden nur die haltlosen oberen
Zellen entfernt. Wasserquellen bleiben beim Pflanzen und Ernten erhalten.
Vines/Wurzeln haengen von oben, Kristalle koennen an beiden Enden Halt haben.
Abgetrennte Mineralzellen fallen als bestehende WorldDrops, koennen Spieler
verletzen und werden nach dem Aufprall sammelbar. Salzhost 30/38 liefert nichts;
Kristalle 51 liefern das vorhandene Salzitem. Kein zweiter Drop-/Persistenzpfad.

IDs 50-56 sind append-only Segmenttypen mit einzelnen JSONs und gemeinsamer
prozeduraler Geometrie. Sie nutzen Material-/Section-Batches, exaktes Dreiecks-
Picking und die bestehenden Abbauteile. Pflanzen, Blaetter und Kristallfacetten
erzeugen keine einzelnen Entities. Nur tatsaechlich fallende Weltitems sind
individuelle bestehende Drop-Objekte. Querschnitte, Windgewichte und Collider
werden bei entfernten Schnitten oder spaeter geladenem Halt gezielt invalidiert.
Support-Reconciliation ist auf 32 Queue-Eintraege pro Tick begrenzt; bei vollem
Dropbudget bleibt die restliche Pflanze stehen, bis wieder Platz vorhanden ist.

Teilzellen werden unabhaengig von verdeckten Cube-Faces aus ihrer gespeicherten
Segmentliste komponiert: dichtes Seegras und innenliegende Kristallansaetze
duerfen nicht vom Vollwuerfel-Mesher ausgeblendet werden. Identische Vertices
benachbarter Dreiecke werden ohne neue Geometrie oder weichgerechnete Kanten
wiederverwendet; Picking und Renderdreiecke bleiben deckungsgleich.
Die Mining-Isolation trennt genau die ausgewaehlte Zelle von ihren Nachbarn.
Hoehlengras verwendet seine Biomfarbe auch unterhalb des Meeresspiegels statt
der ausschliesslich fuer die Oberflaeche gedachten Sand-Uferfaerbung.

Wasserfallbecken besitzen einen gemeinsamen Achtmeter-Ueberlauf mit
maeandrierender Lippe und zwei unterschiedlich breiten Durchlaessen. Gerundete
Fels-Schultern halten die uebrige Oberkante geschlossen; an den Durchlaessen
stehen obere und untere Wasserspalten direkt nebeneinander. Damit entstehen
echte Fallvorhaenge im bestehenden Mesher statt eines trockenen Spalts.

Natuerliches Meereis entfaellt vollstaendig. Platzierter Eisblock 49 bleibt
kompatibel. Riffboden-Vorspruenge werden hoeher und staerker von verbundenen
Korallenmaterialien belegt; die vorhandenen Korallenprops sitzen darauf.

## Kompatibilitaet

Neue Welten speichern `cave_structure_revision = 1`, alte fehlende Werte lesen 0.
Kein automatisches Ueberschreiben alter Hoehlen, Prop-Tombstones oder Wasserstaende.
Alte ganzheitliche Seegrasprops bleiben in solchen Welten lesbar; geerntete alte
Seegrasitems koennen als neue Segmente gepflanzt werden. Fuer neu generierte
Hoehlenraeume und natuerlich segmentierte Seegraswaelder eine neue Welt anlegen.
Ein nachtraegliches Umschalten ist keine getestete Weltmigration.

Die ausdruecklich gewuenschte Meereis-Entfernung und breiteren Riffvorspruenge
gelten auch beim erneuten Generieren uneditierter `ocean_revision = 1`-Boeden.
Vorhandene gespeicherte Zelledits haben weiterhin Vorrang. Bitidentische alte
Riffhoehen oder unveraenderte hoehenabhaengige Prop-Kandidaten werden nicht
behauptet; fuer die komplette neue Landschaft ebenfalls eine neue Welt nutzen.

Seed, Welt-IDs, Generatorversion 5, alle alten Material-IDs und REG9/PLY3 bleiben
erhalten. Netzwerkprotokoll 45 traegt den erweiterten Weltvertrag; Client und
Server muessen gemeinsam neu gebaut und gestartet werden. Keine Dependencies
oder Render-/Audio-Pflichtmodule fuer den Dedicated Server hinzugefuegt.

## Messbedingungen

Nativ unter Linux/Wayland mit Vulkan, RX 6950 XT (RADV, Mesa 26.2.4),
Ryzen 5900X und 32 GiB RAM. 1920x1080 physisch, Sichtweite 8, vertikal 3,
hohe Wasserqualitaet mit SSR, Zeit 10:00 angehalten, ohne VSync/FPS-Limit.
RustRover, weitere Benutzerprogramme und der bereits laufende fremde Server
blieben offen. Keine Compiler oder weiteren Testclients waehrend der Messfenster.
Je 30 Sekunden stationaere CPU-Wall-Framezeiten nach Szenenbereitschaft;
kein Streaming-/Langzeitnachweis und keine isolierte Konsolen-Benchmarkmaschine.

Der alte Client liegt unter `target/phase80/terra-client-before`, seine Assets
unter `target/phase80/before-assets`. Beide sind lokale, nicht versionierte
Messartefakte. `TERRA_ASSET_DIR` waehlt den alten Asset-Snapshot beim Vorherlauf.
Die alten Konfigurationen liegen im benachbarten Phase-79-Bericht:
`benchmark-configs/` fuer den Legacy-Fluss und `configs/` fuer den Seegraswald.
Die neue Konfiguration liegt hier in `configs/`. Messungen erfolgen ueber
`python3 tools/benchmarks/measure.py <client> <smoke-argumente>` mit
`TERRA_BENCH_SECONDS=30`, `TERRA_RENDER_DIAGNOSTICS=1`,
`TERRA_DRAW_DIAGNOSTICS=1` und `TERRA_MEASURE_GPU_MEMORY=1`.

## Performance

| Szene | Vorher FPS | Nachher FPS | p95 ms vorher/nachher | p99 ms vorher/nachher | Initial s vorher/nachher |
| --- | ---: | ---: | ---: | ---: | ---: |
| Legacy-Fluss, gleicher Weltvertrag | 181.16 | 174.33 | 6.931 / 7.251 | 8.580 / 9.749 | 3.292 / 3.258 |
| Dichter Seegraswald, neuer Segment-/Hoehlenvertrag | 184.56 | 140.27 | 6.604 / 8.501 | 8.010 / 9.271 | 2.757 / 3.306 |
| Neue Salzkammer, nur Nachhermessung | - | 120.74 | - / 9.404 | - / 9.940 | - / 4.317 |

| Szene | Peak-RSS KiB vorher/nachher | Peak-VRAM KiB vorher/nachher | Terrain-Komposition Worker-s vorher/nachher | Laengster Kompositionsjob ms vorher/nachher |
| --- | ---: | ---: | ---: | ---: |
| Legacy-Fluss | 858608 / 841868 | 2813280 / 2810408 | 2.157 / 2.160 | 14.192 / 14.023 |
| Seegraswald | 845704 / 812896 | 2851716 / 2844048 | 1.010 / 2.225 | 11.098 / 20.055 |
| Salzkammer | - / 938064 | - / 3407368 | - / 3.363 | - / 20.403 |

Das Flusspaar hat identische 2101 geladene/1575 gerenderte Sections, 5663
Meshassets, 2433 sichtbare Meshentities, 25277 Props in 381 Batches und 31
Wassertiles. Es zeigt keine allgemeine FPS-Beschleunigung: der Nachherlauf
ist rund 3.8% langsamer und besitzt einzelne externe/unzugeordnete Spitzen.

Der Wald hat dieselbe Kamera und 2041 geladene Sections, aber absichtlich
andere Geometrie und neue darunterliegende Hoehlen. 13454 alte ganzheitliche
Props werden zu 4558 verbleibenden Props plus logischen Seegrassegmenten.
Gerenderte Sections steigen 1133 -> 1223, Meshassets 4632 -> 5110,
sichtbare Meshentities 2195 -> 2623, Propbatches sinken 312 -> 306.
Der neue Wald ist in diesem Paar rund 24% langsamer. Das bleibt eine
offene dichte-Szenen-Regression, nicht als Performanceparitaet abgeschlossen.

Das lokale Vertex-Sharing bewahrt alle sichtbaren Dreiecke und erhoeht im
neuen Wald den Messwert von 134.88 auf 140.27 FPS; Peak-RSS sinkt
865488 -> 812896 KiB. Diese beiden Laeufe sind kein statistischer Nachweis.
Der rohe Voroptimierungslauf bleibt in `logs/forest-unwelded.log`, eine
zusaetzliche kurze CPU-Scope-Probe in `logs/forest-scopes.log` erhalten.

Die Salzkammer enthaelt 2835 geladene/2752 gerenderte Sections, 8835 Meshassets,
4617 sichtbare Meshentities, 3249 Props in 422 Batches und 20 Wassertiles.
Ihre beiden expliziten Inspektionslichter sind Teil der Messung; das ist
keine Messung der normalen Fackelszene und besitzt kein Vorher-Paar.
Workerzeiten sind aufsummierte Komposition, keine GPU-Uploadzeit und keine
isolierte CPU-Weltgenerierungszeit. Ein neuer identischer 4375-Section-
CPU-Generierungsvergleich wurde nicht gemessen; die historische 2.6-s-
Baseline wird nicht mit diesen Szenenladezeiten vermischt.

Instrumentierter Opaque-GPU-Pass: Fluss 0.763 -> 0.755 ms, Wald 0.971 ->
0.974 ms, Salzkammer 1.956 ms. Dies sind einzelne Passmittel, keine komplette
GPU-Framezeit. Letzte gemessene erfolgreiche PBR-API-Aufrufe: Fluss 43 -> 41,
Wald 142 -> 136, Kammer 44, jeweils indirekt; UI-Aufrufe 1/5/1.
GPU-interne variable Multidraw-Subdrawzahlen sind weiterhin nicht bekannt.
Rohdaten: `logs/before.log`, `after.log`, `forest-before.log`,
`forest-after.log`, `cave-benchmark.log`.

## Validierung

Alle fuenf vorgeschriebenen Cargo-Pruefungen sind bestanden:

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo test --workspace`: 1271 bestanden, ein bestehender Langzeit-UDP-Soak regulaer ignoriert.
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --workspace`

Der vollstaendige Lauf mit `--no-fail-fast` ist ebenfalls bestanden.
Gezielt geprueft sind deterministische klimatisch begrenzte Hoehlen, getrennte
Wasserintervalle, geschlossene Becken und echte acht Meter tiefe Fallvorhaenge,
negative Section-Grenzen, gueltige nach aussen gerichtete Dreiecke, stetiger
Wind/Taper, exaktes Picking, Teilzellen ohne exponierte Cube-Interfaces,
Vertex-Sharing ohne geaenderte Dreiecke, Collider-Invalidierung, unabhaengige
Seegrastops und Mineral-Kettenbruch. Abbau/Reload, volle Dropbudgets,
spaeter geladene leere Supportsections, Wasserquellenerhalt beim Pflanzen,
Salt-Host ohne Drop, eindeutige Block-/Ressourcen-IDs, Regionen-/Inventar-
Roundtrip und UDP-Late-Join mit autoritativen Entnahmen sind abgedeckt.

`logs/test-first.log` enthaelt den ersten Lauf mit drei danach korrigierten
Testannahmen: Seegras wird nun als Block statt altem Prop platziert, alle
Block-IDs brauchen mehr als zwei Inventare, und ein Speicherfixture belegte
versehentlich dieselbe Koordinate zweimal. Aeltere erste Bild-/Unit-Fehlerlaeufe
bleiben ebenfalls ausdruecklich als solche erhalten; nicht als bestandene
Abnahme werten. Aktuelle Resultate stehen in den finalen Logs.

### Native Ansichten

`--smoke-test-cave-structures` sucht echte generierte Raeume ohne Platzierungs-
Fixtures, prueft vorhandene Segmente/Wasser/Partikel und erzeugt je zwei
GPU-Screenshots in `captures/`. Die Ansicht verwendet zwei explizite weisse
Inspektionslichter, um die Raumgeometrie unabhaengig von einer platzierbaren
Fackel beurteilen zu koennen; das sind keine natuerlichen Hoehlenlichtquellen.

| Raum | Zentrum Seed 42042 | Natuerliche Segmente im geladenen Bereich | Wassertiles | Spraypartikel |
| --- | --- | ---: | ---: | ---: |
| Salz | 16800 / -35 / -17952 | 611 Salz | 20 | 16 Tropfen |
| Tropfstein | -18336 / -24.55 / -18336 | 379 Spitzen | 94 | 0 |
| Bepflanzt | -18144 / -5.81 / -18336 | 243 Ranken, 67 Wurzeln, 111 Pilze | 9 | 0 |
| Wasserfall | -13344 / 63.45 / -18336 | 625 Eiskristalle | 46 | 60 |

Wasser-/Segmentzaehler umfassen den geladenen Bereich, nicht ausschliesslich
die sichtbare Kammer. Die finale Galerie beendet sich nach 33.134 s mit Exit 0,
Peak-RSS 1115636 KiB und VRAM 3433360 KiB. Alle acht Bilder sind gespeichert;
Salz, Tropfstein, bepflanzter Boden und Fallbecken wurden visuell angesehen.

`--smoke-test-ocean-habitats` prueft ohne Platzierungsfixtures grosse Riffe,
segmentiertes Seegras und kaltes offenes Meer in sieben GPU-Ansichten. Exit 0
nach 28.864 s, Peak-RSS 947804 KiB und VRAM 3012404 KiB. Die Eiszaehlung bleibt
in allen Bereichen 0. Riff, Seegras und kalter Meeresboden wurden visuell
angesehen. Die Tests validieren Bildinhalt und gueltige laufende Shader; in
beiden finalen Logs stehen keine WARN-/ERROR-/Slab-/WGSL-Fehler.

Beide Modi lassen sich mit `TERRA_CONFIG_DIR=<dieser Bericht>/configs`,
einem isolierten `TERRA_MENU_CONFIG_DIR` und `TERRA_CAPTURE_DIR` reproduzieren.
Der 30-s-Kammertest verwendet zusaetzlich `TERRA_CAVE_STRUCTURE_BENCH=0` und
`--smoke-test-benchmark`; der Waldtest `TERRA_HABITAT_BENCH=1`.

### Headless

Der frisch gebaute Dedicated Server startet mit geloeschten DISPLAY- und
WAYLAND_DISPLAY-Variablen, waehlt trockenen Spawn und generiert 27 Sections
im bestehenden `--smoke-test`; Exit 0. Der Test bindet einen ephemeren Port
und schreibt keinen echten Weltstand. Der bereits laufende Benutzerserver
auf Port 5000 wurde nicht beendet oder veraendert.
`cargo tree -p terra-server --edges normal --prefix none` enthaelt keine
normalen bevy_render-/bevy_pbr-/bevy_ui-/bevy_winit-/wgpu-Abhaengigkeiten.

## Grenzen

Vertikale lokale Supportketten sind auf 32 Zellen begrenzt. Das ist keine globale
Einsturzsimulation fuer beliebige Felsmassen oder eine Wasser-Drucksimulation.
Aufprallschaden betrifft Spieler; eine eigene Schadensregel fuer Tiere fehlt.
Die neuen Pflanzentypen sind native prozedurale Segmente, kein universelles
JSON-Shape-Scripting. Es gibt noch keine Vermehrung oder Erntewachstumszyklen.
Kurze Laufzeiten und eine einzelne GPU beweisen keine allgemeine Performance-
oder Speicherparitaet. Bildreferenzen bleiben Art-Richtung, keine 1:1-Kopie.
