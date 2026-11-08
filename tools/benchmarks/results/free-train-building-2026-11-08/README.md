# Phase 212: Freier Zugbau und Montagevorlagen

## Verhalten

Die Montagestelle stellt ihre Mausereignisse unter dem Spieloverlay wieder her.
Dekorative Ebenen nehmen keine Klicks entgegen; Text- und Iconziele erreichen
den Button. Lokomotive, Tender, Personenwagen und Gueterwagen bieten eine
optionale am Gleis ausgerichtete Vorlage. Gruen markiert fehlende Teile, Rot
Konflikte. Die Vorlage bleibt beim Schliessen zum Bauen sichtbar und wird bei
Abbruch, Montage oder Welt-/Verbindungswechsel bereinigt.

Die allgemeine Bodenpflicht fuer Moebel und Zugteile ist entfernt. Weiterbau
ueber alle sechs Seiten eines schwebenden Blocks ist moeglich. Besondere
Befestigungen und Spielerueberschneidungen werden weiterhin geprueft.

F64 montiert auch selbst gesetzte registrierte Bloecke mit Formen, Ausrichtungen,
Farben, Holzarten und Wappen. Die Quelle bleibt ein zusammenhaengender Aufbau
oberhalb des Gleisdecks. Mehrblockobjekte muessen vollstaendig enthalten sein.
Kollision, Zielerfassung und Darstellung verwenden die erhaltene Form.
Fenster, Doppeltueren, Rohrknoten, Becken und andere Verbindungen verwenden ihre
fahrzeugeigenen Nachbarn. Urspruengliche Dekorvarianten behalten ihre Position
als Zufallsschluessel.

Truhen sind montiert als privates Ladungsinventar verwendbar. Maschinenzustand,
Inventare, Fluessigkeiten, Filter, Brennstoff, Verarbeitung und Oberflaechenzustand
werden privat eingefroren und atomar zurueckgegeben. Montierte Maschinen
pausieren ohne Weltanschluesse. Landschaft, Gleise, Montagestellen, freies Wasser
und registrierte lebende Baeume werden nicht eingesammelt. Angehaengte separat
simulierte Pflanzen/WorldProps verhindern die Montage mit konkretem Hinweis;
leeres Ackerland und voxelbasierte Dekoration bleiben verwendbar.

Fahrbetrieb und Dampfantrieb bleiben F53. Die aktuelle Benutzeranleitung steht
in [docs/train-building.md](../../../../docs/train-building.md).

## Daten und Grenzen

Netzwerkprotokoll 117 uebertraegt oeffentliche Form-/Farbdaten ohne private
Maschinennutzlast. Der neue Zugindex ID14 liest ID13 ueber dessen eingefrorenes
historisches Layout. Alte Indizes bleiben lesbar.

Die bestehenden Grenzen von 64 Fahrzeugen, 256 Teilen pro Fahrzeug und 4096
Teilen insgesamt bleiben bestehen. Kollisionsbeschreibungen sind auf 4096 Boxen
pro Fahrzeug und 16384 insgesamt begrenzt. Private Nutzlast ist auf 64 KiB pro
Teil, 1 MiB pro Fahrzeug und 8 MiB insgesamt begrenzt. Ein Postcard-Grenztest
prueft die maximal belegte oeffentliche Beschreibung unter 2 MiB.

## Hauptthread und Hintergrundarbeit

Terrain-Modellgeometrie und Collider werden gemeinsam im Hintergrund berechnet.
Der Hauptthread kopiert begrenzte Voxeldaten und blockbezogene Metadaten; private
Inventare und Spielweltdaten gelangen nicht in diese Momentaufnahme. Lange
Foerderbaender und verbundene Fenster-/Tuerenreihen behalten ihre benoetigten
Metadaten. Voxel-, Profil- und Revisionspruefungen verhindern die Veroeffentlichung
ueberholter Ergebnisse.

Auch Fahrzeugmeshes und Fahrzeug-Collider werden asynchron gebaut. Jeweils
hoechstens zwei laufende Jobs sind zugelassen, einschliesslich ueberholter
nativer Berechnungen bis zu deren tatsaechlichem Abschluss. Generationstoken
und Geometrieschluessel verhindern spaete Veroeffentlichungen. Collider werden
erst nach ihrer Aktivierung im Physikbackend als bereit gemeldet. Vorhandene
Uebergaben bei Demontage und schnellem Wiederaufbau bleiben erhalten.

Die CPU-Mikromessung vergleicht den frueher synchronen Geometrieschritt mit der
neuen Vorbereitung der Momentaufnahme. Sie verwendet 256 verbundene Fenster,
45 lokale Sections und 2000 entfernte bearbeitete Sections, nach einem Warmlauf
mit 16 Stichproben. Collider-Cooking und Grafik sind nicht Teil dieser Messung.
Die protokollierten Werte stehen in `logs/collision-dispatch-benchmark.log`.
Es wurden keine Spielclients, Spielfenster, Screenshots oder GPU-Abnahmen gestartet.

| CPU-Zeit | Frueher synchron | Neue Uebergabe |
| --- | ---: | ---: |
| Mittel | 5,123 ms | 0,050 ms |
| p95 | 5,183 ms | 0,077 ms |
| Maximum | 5,183 ms | 0,077 ms |

## Regressionen

Der vollstaendige Workspace-Library-Lauf besteht mit 4152 erfolgreichen Tests,
keinem Fehler und neun ignorierten Tests:

| Paket | Bestanden |
| --- | ---: |
| terra-core | 438 |
| terra-network | 534 |
| terra-physics | 73 |
| terra-player | 118 |
| terra-render | 1054 |
| terra-server | 8 |
| terra-storage | 164 |
| terra-ui | 541 |
| terra-voxel | 25 |
| terra-world | 1197 |

Die echten UDP-Tests pruefen zusaetzlich zur bestehenden Montage eine normale
Truhe und einen gefaerbten, ausgerichteten Formblock. Besitzer, Beobachter und
spaeterer Beitritt erhalten dieselbe sichtbare Form; private Ladung bleibt
geschuetzt. Demontage, Speicher-Roundtrip und historische ID13-Daten erhalten
den Zustand. Kapazitaetsfehler und zwischenzeitliche Aenderungen publizieren
keine halbe Transaktion.

Die Renderingtests vergleichen importierte Modelle und ihre Verbindungen als
CPU-Geometrie. Die Shader-Importpruefung registriert Bibliotheken ausdruecklich
mit `WgpuSettings.backends = None`; damit wird kein GPU-Backend initialisiert.
Mausereignisse werden ueber den wirklich registrierten HTML-Handler geprueft.
World-/Verbindungswechsel, kurze Fensterhoehen, HD bis 4K, veraltete Worker,
echte Physikkontakte, sparse Wasserzustandsrueckgabe und nachfliessendes Wasser
an entfernten Quellzellen sind enthalten.

Der aktuelle Workspace-Build von Client und Server besteht (8 min 37 s), ebenso
Clippy fuer alle Targets mit `-D warnings`, Format und Diff-Pruefung.
Die Protokolle liegen unter `logs/`; `verification.json` enthaelt Ergebnisse,
Pruefsummen der Protokolle und die geprueften Repository-Quelldateien.

Die Cargo-Laeufe verwenden `CARGO_BUILD_JOBS=1`, `nice -n 10`,
`ionice -c 2 -n 7` und entfernte `DISPLAY`-/`WAYLAND_DISPLAY`-Variablen:

```sh
cargo test --locked --workspace --lib --no-fail-fast -- --test-threads=4 --skip shader_imprint_tests
cargo build --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Die separate CPU-Mikromessung startet ausschliesslich den passenden bereits
gebauten Physics-Test mit `authored_geometry_dispatch_benchmark --ignored
--nocapture --test-threads=1`.
