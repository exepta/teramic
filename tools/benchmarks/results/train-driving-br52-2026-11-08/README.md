# Phase 213: Brennstoffbetrieb und BR52-Baukasten

## Verhalten

Fahrgestell und Rahmen sind volle Blockkoerper. Groessere Raeder liegen auf den
beiden Schienen auf; Rahmen, Kupplungen, Sitze, Kessel und Daecher haben passende
Montagekontakte. Ein durchgehender, frei gebauter Aufbau nimmt normale Bloecke,
Formen, Holzarten, Tueren, Fenster und Inventare auf. Zwei Drehgestelle werden
bei neuer Montage mit 3 bis 15 Metern Mittenabstand verlangt; die Vorlage ist
entsprechend einstellbar. Alte gespeicherte kuerzere Fahrzeuge bleiben lesbar.

Brennstoffmotor 218 verbraucht echte Ofenbrennstoffe aus einem privaten Inventar.
Angefangene Brenndauer ueberlebt Fahrt, Speichern und Rueckbau ohne Verdoppelung.
Der Besitzer steuert aus einem montierten Fuehrerstand einen siebenstufigen
Hebel, Bremse und Fahrtrichtung. Konfigurierte W/S-, Sprung- und Absteigebelegungen
sowie der wirkliche HTML-Slider sind angebunden. Das Fahrpult zeigt echte
Geschwindigkeit in Bloecken/s bzw. Metern/s. Der Motor und beliebig angeordnete
Laderaeume sind vom eigenen Fuehrerstand aus erreichbar.

Die Bewegung folgt aktiven Gleiswegen, Kurven und Rampen; beide Drehgestelle
bestimmen eine starre Fahrzeuglage. Gekuppelte Wagen folgen derselben Strecke.
Das vorhandene Live-Gleisbettlimit wird einmal angewandt. Fehlende/ungeladene
Gleisabschnitte, Hindernisse, andere Fahrzeuge, Personen im Fahrweg und fehlende
Baurechte verhindern weitere Fahrt. Fahrtzustand und private Nutzlasten werden
mit dem Spielstand gespeichert.

Montage ist nach Stillstand an einem ebenen, geraden Gleis neben einer
Montagestelle moeglich. Der Fahrer verlaesst zuerst seinen Sitz; Kupplungen
muessen geloest und andere Mitfahrer ausgestiegen sein. Rueckbau setzt die
Teile am aktuellen Ort wieder in die Welt, samt mitgedrehter Ausrichtung,
Mehrblockpartnern und Maschinenmetadaten. Besitz und private Ladung bleiben
geschuetzt. Eine bisher falsche globale/per-Fahrzeug-Revisionspruefung und ein
unpassender UI-ACK-Zweig sind korrigiert. Zugriff zielt auf die tatsaechlichen
transformierten Teilformen statt auf gerundete Zellmitten; feste Waende bleiben
als Sichtschutz wirksam.

## F54

Alle 76 neuen BR52-Komponenten 218–293 haben native Blockdefinitionen,
GLTF-/Binaermodelle, Inventarnamen und Rezepte. Die Referenz wird als modularer
Baukasten umgesetzt: Kessel/Rauchkammer, Rahmen/Puffer, Raeder/Achsen/Gestaenge,
Fuehrerhaus/Daecher, Tender, Leitern/Gelaender, Nieten/Schrauben/Luken,
Scheinwerfer/Schilder/Werkzeug und Holz-/Kohledekoration. Schwarzes Eisen,
dunkler Stahl, Blech und echtes offenes Gitter bieten je sechs Bauformen.
Slabs und Treppen behalten ihre vollstaendigen Ausrichtungscodes; gemischte
Slabs und Gitteroeffnungen verwenden passende Geometrie und Kollision.

Raeder und Gestaenge folgen den wirklich zurueckgelegten Metern; jedes Rad
behaelt seinen eigenen Drehpunkt. Der Fuehrerstandhebel folgt der ausgewaehlten
Stufe. Fenster und Tueren haben echte Fluegel und Durchsicht. Das private
Brennstoffinventar ist funktional. Dampfventile, Druckinstrumente und der
Tender-Wassertank sind als Aufbau gekennzeichnet: echte Wasserversorgung und
Kesseldruck gehoeren weiterhin zum offenen Dampfantrieb F53.

## Daten und Laufzeit

Protokoll 118 uebertraegt kompakte Bewegung getrennt von der Blockgeometrie;
ruhende unveraenderte Zuege erzeugen keine wiederholten Positionspakete.
Die maximale Posefrequenz ist 60 Hz. Existierende native Kinematikkoerper und
Renderwurzeln werden verschoben; ihre Geometrie wird dabei nicht neu gebaut.
Aufwendige Fahrzeugjobs behalten ihre bisherige Grenze von jeweils zwei.
Berechnete Formgrenzen verwenden eine vorbereitete starre Matrix statt
Trigonometrie pro Ecke. Gekoppelte Gruppen werden vor Veroeffentlichung gemeinsam
geprueft. Die letzte Pose vor einer Demontage bestimmt die kausale Collider-
Uebergabe an die tatsaechlich zurueckgesetzten Sections.

Zugindex ID15 speichert Pose, Streckencursor und Motorbrenndauer. Eingefrorene
historische Decoder lesen ID13 und ID14. Bei ID14 werden die alten eingebauten
Platten-/Radprofile auf die neuen Blockkoerper aktualisiert; frei montierte
Formen und private Nutzlasten bleiben erhalten. Vorhandene Teile-, Volumen-,
Nutzlast- und Nachrichtenbudgets bleiben begrenzt. Der gesamte Rezeptkatalog
passt mit 415 Rezepten weiter unter die Grenze von 512.

## Abnahme

Die Abnahme verwendet ausschliesslich CPU-Tests und Build-/Lintwerkzeuge.
Es werden keine Spielclients, Spielfenster, Screenshots oder GPU-Tests gestartet.
Die Shader-Importpruefung verwendet ausdruecklich kein GPU-Backend.

Die Cargo-Aufrufe laufen einzeln mit `CARGO_BUILD_JOBS=1`, `nice -n 10`,
`ionice -c 2 -n 7` und entfernten `DISPLAY`-/`WAYLAND_DISPLAY`-Variablen:

```sh
cargo test --locked --workspace --lib --no-fail-fast -- --test-threads=4
cargo build --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Alle **4.206 Workspace-Library-Tests** bestehen; kein Fehler, neun bestehende
Tests ignoriert. Der zusaetzliche Zugfilter besteht mit 125 Tests. Darunter sind
wirkliche registrierte UI-Klick-/Sliderereignisse, reale native Kinematikkoerper,
alle Holz-/Metallformen, Blockkontakte und eingefrorene Nutzlasten.

| Paket | Bestanden |
| --- | ---: |
| terra-core | 449 |
| terra-network | 536 |
| terra-physics | 75 |
| terra-player | 118 |
| terra-render | 1062 |
| terra-server | 8 |
| terra-storage | 166 |
| terra-ui | 554 |
| terra-voxel | 25 |
| terra-world | 1213 |

Die reale UDP-Abnahme prueft Aufbau, normale Truhe, Fahrer/Sitz, fremde
Zugriffsversuche, Fahrtposen fuer Besitzer und Beobachter, Verbrauch eines echten
Brennstoffitems, Bremsen, Teilbrenndauer, aktuelle Rueckbauzellen und spaeteren
Beitritt. Weitere Tests pruefen 3–15 m Radbasis, echte Kurven/Rampen, aktive
Weichenwege, duenne Hindernisse, gekuppelte Koerper, alte Speicherindizes und
verlustfreien Rueckbau nach Bewegung/Drehung. Die Zugriffskorrektur ist durch
einen sichtbaren Fuehrerstand und eine anschliessend davor gebaute Wand geprueft.

Die CPU-Geometriepruefung umfasst 84 Zugmodelle mit 907 begrenzten Kollisionsboxen,
5.064.912 Binaerbytes und 44.788 Dreiecken. Alle Puffer, Accessoren, Indizes,
Texturpfade, Namen und 415 eindeutigen Rezept-IDs/-Keys sind gueltig:

```sh
python3 tools/assets/lint_train_parts.py --log target/verification213/asset-lint.log
```

Der vollstaendige Workspace-Build von Client und Server besteht (8 min 55 s),
einschliesslich Linking. Die Programme wurden nicht gestartet.
Clippy aller Targets mit `-D warnings` besteht. Format- und Diff-Pruefung sind
sauber. Die Protokolle liegen unter `logs/`; `verification.json` enthaelt die
Pruefergebnisse sowie Quell- und Protokollpruefsummen.
