# Phase 106: Capes, Crusher Und Integrierte Ruestungsleiste

## Ruestungs-HUD

Die Schutzanzeige verwendet den schmalen, abgeschraegten Stamina-Rahmen samt
eingelassenem Medaillon. Der ausgefuellte, facettierte Schild ersetzt dort
den Blitz. Silberne Fuellung, weiche Wertuebergaenge und die vorhandene
Gold-/Gruen-Zeichensprache bleiben erhalten. Kein zusaetzliches Bitmap,
Fullscreen-Pass oder Material pro Frame. Die Anzeige misst weiterhin Schutz,
nicht Haltbarkeit; Shield Power bleibt getrennt. Nur das erste Status-Card-
Symbol wirkt auf Herz/HP.

## Capes

Neuer zehnter Ausruestungsplatz, Index 9; bisherige Indizes bleiben erhalten.
Leather, Iron, Gold, Diamond und Ruby beginnen weiss. Je ein Cape und ein
Farbstoff ergeben ein umgefaerbtes Cape derselben Materialstufe. Auch ein
bereits gefaerbtes Cape kann erneut gefaerbt werden; Weiss stellt die
urspruengliche Stofffarbe wieder her. Riemen, Beschlaege und Brosche behalten
die Materialfarbe. Werteboni und Schutz haengen an der Stufe, nicht am Dye.

Geschichtete Voxel-Stoffbahnen, Schal, Riemen und Brosche verwenden drei
gebuendelte Teile je Cape. Zwei Stoffteile bewegen sich dezent mit dem Gang;
das ist eine stilisierte Gelenkanimation, keine Cloth-Simulation.
Meshvarianten werden nach Bedarf angelegt und sind auf 35 Farben/Stufen mal
zwei Koerperformen begrenzt. Keine Entity je Stoffvoxel. Die vorhandenen
Avatar-/Portraet-/Item-/Drop-/Handpfade nutzen dieselben Modelle.

Fuenf einzelne Item-JSONs, fuenf Basis- und 35 Faerberezepte liegen in den
bestehenden Katalogen. Im Creative-Katalog stehen die fuenf weissen Varianten;
die gefaerbten Varianten entstehen durch Faerben. Ausruestung stapelt zu eins.

## F03 Crusher

Block 99, horizontaler Zweiblock-Footprint, 2 x 1 x 1,23 m. Die Ausrichtung
folgt dem Spieler. Abbau einer Haelfte entfernt beide und gibt private Inhalte
nur einmal frei. Struktureller Block: Gras darunter bleibt erhalten.

Das gemeinsame Modell besitzt zwei hohle Trichter, Holzrahmen, Metallbaender,
Nieten, Zahnwalzen, Seitenzahnrad, Laterne und Auswurfablagen. Reproduzierbarer
Generator: `tools/assets/build_crusher.mjs`. 15.832 Vertices, 8.016 Dreiecke,
31 Kollisionsboxen und 14 Materialgruppen. Statischer Rahmen bleibt in den
Section-Batches; sechs geteilte Meshes bilden die beweglichen Baugruppen.
Trichter erhalten gestufte Gussplatten, Innenkurse und echte flache Reliefs.
Eine gemeinsame Auswahlkontur ersetzt stoerende Gitter aller einzelnen
Kollisionsboxen; die physikalischen Teilformen bleiben erhalten.
Partikel verwenden ein wiederverwendetes gemeinsames Mesh, hoechstens acht
Chips an 16 nahen arbeitenden Maschinen.

Ein- und Ausgabe sind private Weltinventarslots. Automatische mechanische
Verarbeitung: vier Sekunden pro Item, ohne Brennstoff oder erfundenes
Stromnetz. Volle oder inkompatible Ausgabe pausiert ohne Verbrauch. Ausgabe
ist nur entnehmbar. Cursor, Shift, Drag, Rad und reservierte Zutaten nutzen
die bestehenden atomaren, revisionierten Containertransaktionen.

16 einzelne Rezepte unter `assets/crushing/`:

- Raw Iron und Iron Ingot -> je 1 Iron Dust.
- Raw Gold und Gold Ingot -> je 1 Gold Dust.
- Coal, Diamond, Salt Crystal und Ruby -> je 1 entsprechender Dust.
- Sieben farbige Blumen -> je 2 gleichfarbige Dyes, einschliesslich Weiss.
- Generische historische Flower -> 2 Red Dye als kompatible Altvariante.

Die sieben Blumenitems speichern ihre Pigmentfarbe beim Ernten und
Wiederpflanzen. Die blassrosa natuerliche Bluetenvariante liefert das vorhandene
violette Pigment; eine neue Pink-Dye-Farbe wird nicht eingefuehrt.
Die alte generische Blumen-Zutat akzeptiert diese Varianten
weiterhin; vorhandene Hand-Crafting-Rezepte werden nicht entfernt.
Der Crusher verwendet dagegen eindeutige farbgebundene Rezepte.

Too Many Items zeigt Crusher-Rezepte mit Ein-/Ausgabe, Prozesssymbol und Zeit.
Der Plus-Button befuellt nur einen passenden geoeffneten Crusher und schliesst
danach das Popup; Ansehen allein verschiebt nichts. Alternativen und
Seitenanzeige teilen sich die vorhandene Rezeptansicht mit Crafting/Ofen.
Das Maschineninventar besitzt eigene Slots und Fortschritt ohne Zahlenlabels.

## Lebenszyklus Und Kosten

Der Server verarbeitet geladene, aktive Maschinen mit einem fairen Budget
von 32 Stationen je 125 ms. Nachholzeit ist auf eine Sekunde begrenzt;
ungeladene Maschinen laufen nicht offline weiter. Stabile/leere oder
ausgabeblockierte Maschinen werden nicht permanent weitergerechnet.
Privater Fortschritt und Inhalte erzeugen keine Terrain-Mesh-Rebuilds.
Bis zu 64 nahe oeffentliche Status-Snapshots steuern die Animation.
Dedicated-Autosave beobachtet auch die Gameplay-Revision: Verarbeitung wird
selbst bei angehaltener Uhr und unveraenderter Terrain-Change-Detection
gespeichert, statt erst durch eine fremde Blockaenderung oder beim Beenden.

Der Renderer sucht nur in geladenen Sections und deren vier horizontalen
Nachbarn nach oeffentlichen Objektmetadaten. Der durch Section-Epochen
invalidierte Kandidaten-Cache vermeidet Welt-Scans im unveraenderten Idle.
Private Inventar-/Maschinenzustaende werden fuer sichtbare Geometrie nicht
vorausgesetzt. Auch eine allein geladene Sekundaerhaelfte erhaelt die Mechanik.
Keine harte Anzahlgrenze laesst ploetzlich Walzen verschwinden. Mehr geladene
Maschinen bedeuten entsprechend mehr Baugruppen, nicht kostenlose Instanzen.

Die 0,23 m ueberstehenden Trichter werden auch beim spaeteren Platzieren
darueber und an vertikalen Section-Grenzen beruecksichtigt. Freier Raum ueber
einer oberen Half-Slab bleibt nutzbar; kein pauschales Vollzellenverbot.

## Kompatibilitaet

Protokoll 58 prueft Ausruestungs- und Crusher-Rezeptkataloge. Client und Server
gemeinsam aktualisieren. Spielerdateien schreiben PLY7; PLY6 mit genau neun
Ausruestungsslots wird explizit in zehn Plaetze uebernommen, Cape bleibt leer.
PLY5 und aeltere Leser bleiben erhalten. RGA1 ergaenzt Crusher-Fortschritt mit
explizitem RGA0-/Altformat-Leser. Weltformat 2, Generator 5, bestehende
Block-/Welt-IDs und Seeds bleiben unveraendert. Keine Dependency-Upgrades.

## Native Funktionspruefung

Zwei kurze echte Vulkan-/UDP-Laeufe auf der RX 6950 XT, keine FPS-Benchmarks:

- `--smoke-test-equipment`: alle fuenf Sets, weisse und gefaerbte Capes,
  Ablegen, zehn Ausruestungsplaetze und konstante Gesamtmenge von 36 Items.
  16 bestaetigte Screenshots, maximal 2937 Mesh-Assets im gesamten Testclient;
  der fixtureseitige Cache-Grenzwert wird geprueft, nicht allgemeine
  Speicherparitaet behauptet.
- `--smoke-test-crusher`: echte Survival-Platzierung, Oeffnen, TMI-Rezeptwahl
  und Plus-Transfer, Verarbeitung aller 16 Rezepte, Entnahme und aggregierte
  Ein-/Ausgabeerhaltung. Fuenf bestaetigte Screenshots; eine Maschine mit
  sechs gecachten Mechanikmeshes. Der Arbeitszustand prueft nichttriviale
  Walzenrotation und aktive Partikel, nicht nur einen gesetzten Status.
- Beide UI-Faelle schliessen kompakte Fenstergroessen ein. Leere Aufnahmen,
  fehlende Readbacks, Itemverlust und Zeitueberschreitung brechen den Test ab.

Die finalen Bilder wurden tatsaechlich angesehen:
[Cape und integrierter Schild](native/equipment/armor-ruby-world.png),
[gefaerbtes Cape im Inventar](native/equipment/armor-ruby-blue-inventory.png),
[Crusher-Modell](native/crusher/crusher-placed.png),
[Crusher-Rezept](native/crusher/crusher-recipe.png) und
[kompakte Maschinen-UI](native/crusher/crusher-compact.png).
Trichter, Zahnwalzen und segmentierte Metallflaechen sind sichtbar, das interne
Collider-Drahtgitter ist entfernt. Die Maschine bleibt heller und glatter als
die dunklere, kleinteiligere Referenz; keine identische Art-Abnahme.

Isolierte Konfiguration mit `TERRA_CONFIG_DIR` und `TERRA_MENU_CONFIG_DIR`,
Capture-Ziel ueber `TERRA_CAPTURE_DIR`. Beide Flags starten einen kurzlebigen
integrierten Server mit eigener temporaerer Welt; kein Eingriff in Spielwelten.
Die GPU-Laeufe pruefen keine Speicherung ueber Neustarts; diese decken die
separaten Speicher-/Netzwerk-Regressionspruefungen ab.
Im Equipment-Startlog steht einmal eine Lightyear-Warnung ueber vor der
Empfaengerregistrierung verworfene `CreatureStates`; kein Renderfehler oder
Panic. Der Crusher-Lauf hat keine Warnungen/Fehler gemeldet. Die originalen
Logs bleiben unverkuerzt erhalten.

## Regressionstests

`cargo test --locked --workspace --no-fail-fast`: 1850 bestanden,
zwei bestehende Tests ignoriert, keine fehlgeschlagenen Tests. Die neuen
Pruefungen umfassen insbesondere:

- Fuenf weisse Cape-Grundtypen, 35 Faerbevarianten, Erhalt von Materialstufe
  und Boni bei erneutem Faerben, Stack-/Slot- und Inventartransaktionen.
- Explizite PLY6->PLY7-Migration der alten neun Plaetze; Cape, Farbe und
  abgeleitete Attribute nach Speicherung. RGA0->RGA1-Migration und private
  Maschineninhalte/Fortschritt ueber Regionsgrenzen.
- Alle 16 Crusher-Rezepte, gesperrte/volle Ausgabe, Revisionen, reservierte
  Zutaten, ungueltige Zeitwerte, begrenzte Nachholzeit und Entladungspause.
- Echter Dedicated-Autosave bei angehaltener Uhr, unveraenderter Terrain-
  Change-Detection und anschliessendem Store-Neuladen mit exakten Werten.
- Korrekte Hopper-Kollisionen an positiven/negativen Section-Kanten und
  Verhinderung spaeterer ueberlappender Blockplatzierung. Dieser Test hat
  einen realen fehlenden Collider in logisch leeren Nachbarsections gefunden
  und die Korrektur abgesichert.
- Geladene Sekundaerhaelften, oeffentliche Metadaten ohne private Maschinen-
  Zustaende, kein wiederholter Welt-Scan im Idle, begrenzte Partikel,
  sechs wiederverwendete Mechanikmeshes nach Unload/Reload.
- TMI-Rezeptwahl ohne sofortiges Verschieben, stationgebundener Plus-Transfer,
  Fortschrittsanzeige ohne Rucksackrevision und alle neuen Ergebniseintraege.
- Getrennte Farben fuer Blumen/LOD, vollstaendige Materialzuordnung und
  gemeinsame Auswahlkontur unabhaengig von der angeklickten Maschinenhaelfte.

Ein zuerst fehlerhafter neuer Kompatibilitaetstest erwartete pauschal zwei
Dyes fuer historische Handrezepte und zu wenige Primaerfarben zum Mischen.
Korrigiert sind ausschliesslich Testzutaten/-erwartungen; die vorhandenen
Produktionsrezepte bleiben unveraendert. Der abschliessende volle Lauf ist gruen.

## Abschlusspruefungen Und Logs

Alle Befehle mit unveraenderten Dependency-Versionen und einem zentralen
Cargo-Prozess; Build-/Check-/Test-/Clippy-Laeufe mit `CARGO_BUILD_JOBS=1`:

| Pruefung | Ergebnis | Originales Log |
|---|---|---|
| `cargo fmt --all --check` | bestanden | [Fmt](logs/phase106-fmt.log) |
| `cargo check --locked --workspace --all-targets` | bestanden | [Check](logs/phase106-check.log) |
| `cargo test --locked --workspace --no-fail-fast` | 1850 bestanden, 2 ignoriert | [Tests](logs/phase106-tests.log) |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | bestanden | [Clippy](logs/phase106-clippy.log) |
| `cargo build --locked --workspace` | bestanden | [Build](logs/phase106-build.log) |
| `terra-server --smoke-test`, ohne Display-Variablen | UDP, 27 Sections, Exit 0 | [Headless](logs/phase106-headless.log) |
| `terra-client --smoke-test-equipment` | 16 Aufnahmen, Exit 0 | [Equipment](logs/phase106-native-equipment.log) |
| `terra-client --smoke-test-crusher` | 16 Rezepte, 5 Aufnahmen, Exit 0 | [Crusher](logs/phase106-native-crusher.log) |

Die nativen Client-Laeufe pruefen die finalen Gameplay-, UI-, Shader- und
Modellpfade. Die danach ergaenzte Dedicated-Autosave-Korrektur ist im finalen
Workspace-Test/Build und Headless-Smoke enthalten; sie aendert diese Client-
Renderpfade nicht. Kein weiterer GPU-Neulauf nach dieser reinen Serverkorrektur.

## Grenzen

Die Vorlagen werden im vorhandenen prozeduralen Voxelstil interpretiert,
keine identische Pixel- oder Modellkopie zugesagt. Kein neues Stromnetz,
Automationsrohre oder Schmelz-Rezepte fuer Staub. Keine echten Stoffkollisionen
und kein Ruestungsverschleiss. Keine neuen 10-/15-/30-Minuten-Laeufe und
keine allgemeine FPS-/VRAM-Paritaetsbehauptung.
