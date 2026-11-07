# O09, F43 und Burgbloecke

06.11.2026. Kingdom-Gruendung am Bergfried, zwanzig Burgbloecke nach der
[Bildreferenz](../../../../docs/references/castle-blocks.png) und optimierter
CSS-Abgleich beim kalten Aufbau neuer Inventarlayouts.

Im kontrollierten Truhenvergleich sinkt der kalte Spitzenframe von **94,53 auf
54,84 ms** (rund 42 Prozent), erneutes Oeffnen von **22,45 auf 19,69 ms**.
Die Grafikqualitaet bleibt gleich. Der kalte Frame mit 54,84 ms bleibt messbar;
Methodik, Scopes und Grenzen stehen im [O09-Bericht](O09.md).

## Kingdom benutzen

Einen **Bergfried** platzieren und mit der konfigurierten Benutzen-Taste oder
Rechtsklick oeffnen. Mit der Abwaerts-/Schleichen-Bindung bleibt das Platzieren
an einem bestehenden Bergfried moeglich; die Standardbindung akzeptiert beide
Shift-Tasten. Der Dialog verwendet den bestehenden verzierten Rahmen und einen
scrollbaren Inhalt.

Ein freier Bergfried erlaubt die Gruendung mit einem Namen. Der Gruender kann
umbenennen, Mitglieder entfernen und nach ausdruecklicher Bestaetigung aufloesen.
Besucher koennen beitreten, normale Mitglieder wieder austreten. Der Gruender
muss sein Kingdom aufloesen, bevor er austreten oder den Bergfried abbauen kann.

Der Server prueft Identitaet, Berechtigung, Zustand, Entfernung und freien Blick
zum echten Bergfried. Die Reichweite betraegt acht Meter ab Augenposition.
Gruenden braucht Baurechte; Spectator darf nur inspizieren. Ein Spieler und ein
Bergfried koennen jeweils nur einem aktiven Kingdom angehoeren. Namen werden
getrimmt, erlauben 1 bis 32 druckbare Unicode-Zeichen und sind fuer aktive
Kingdoms ohne Beachtung der Gross-/Kleinschreibung eindeutig. Grenzen: 64
Mitglieder und 256 gespeicherte Kingdoms einschliesslich aufgeloester Historie.

Gespeichert und repliziert werden die monoton vergebene ID, Name, Gruender-UUID
und Anzeigename, Mitglieder-UUIDs und Anzeigenamen, Standort und Active/Dissolved.
Aufloesen behaelt die Historie und gibt Mitgliedschaften, Namen und Standort
frei. Eine Neugruendung erhaelt eine neue ID. Der aktive Bergfried ist auch im
direkten Journalpfad gegen Entfernen geschuetzt. Der Schutz wird beim Laden aus
der Registry wiederhergestellt.

Der Speicherindex IDX8 umschliesst den eingefrorenen IDX7-Stand; bestehende
Indexmigrationen bleiben erhalten. Netzwerkprotokoll: 102. Spaeter beitretende
Clients erhalten denselben vollstaendigen Stand. RPCs verwenden eigene Nonces;
sie beeinflussen ausstehende Blockedits nicht. Der Dialog korreliert jede Antwort
mit der exakt reservierten Anfrage-ID, damit alte Antworten nach erneutem
Oeffnen keine neue Anfrage abschliessen.

Chunk-Claims, Diplomatie und weitergehende Gebietsrechte stehen weiterhin als
F55/F56/F57 in der offenen Planung.

## Burgfamilie

Alle Bloecke besitzen eigene glTF-Geometrie, Texturen, Inventarvorschauen,
Handwerksrezepte, passende Oberflaechengeraeusche und physische Formen. Sichtbare
Mauerfugen, Zinnen, Bewuchs, Verstaerkungen und Banner werden als Geometrie
dargestellt. Die vorhandenen Texturen werden wiederverwendet. Der reproduzierbare
Generator ist [build_castle.py](../../../assets/build_castle.py).

| ID | Block |
| ---: | --- |
| 173 | Bergfried |
| 174 | Steinmauer |
| 175 | Bergfried Mauer / Mauerkrone |
| 176 | Rissige Mauer |
| 177 | Bewachsene Mauer |
| 178 | Moos Mauer |
| 179 | Verstaerkte Mauer |
| 180 | Zinnenmauer |
| 181 | Zinnen Ecke |
| 182 | Zinnen Saeule |
| 183 / 184 | Mauerbogen Links / Rechts |
| 185 | Mauer mit Schiessscharte |
| 186 | Mauer mit Fenster |
| 187 | Sandstein Mauer |
| 188 | Wuestenmauer mit Muster |
| 189 | Schneemauer |
| 190 | Schwarzer Stein |
| 191 | Koenigsmauer mit Wappen |
| 192 | Holzverstaerkte Mauer |

Vier Stein ergeben eine Steinmauer, vier Steinmauern eine Mauerkrone. Die
weiteren Rezepte verwenden diese Grundbloecke und vorhandene passende Zutaten;
sie erscheinen im bestehenden Rezeptbrowser. Die zwei Bogenhaelften sind
verlustfrei ineinander umwandelbar, insgesamt gibt es 21 Rezepte.

Jede Bogenhaelfte belegt eine Zelle mit **1 x 1 x 0,5 Block**. Zusammen ergeben
sie einen zwei Bloecke langen Bogen mit halber Blocktiefe. Beide Teile lassen
sich einzeln platzieren und wie Treppen in allen 24 Orientierungen ausrichten
oder mit dem Schraubenschluessel drehen. Auswahlstrahlen und Kollisionskoerper
folgen der Oeffnung auch kopfueber und ueber Section-Grenzen.

Native Originalaufnahmen: [Vorderseite](castle/castle-front.png),
[Rueckseite](castle/castle-back.png), [Bogenoeffnungen](castle/castle-openings.png).

## Pruefungen

3.836 Workspace-Library-/Bin-Tests bestanden, neun bestehende Tests ignoriert.
19 Python-Runner-Tests bestanden. Clippy aller Targets besteht mit `-D warnings`.
Die zusaetzlichen TiltUI-Tests und die kontrollierten O09-Messwerte stehen im
[O09-Bericht](O09.md).

Die Kingdom-Pruefungen umfassen echte UDP-Clients, Mitgliederaktionen,
unberechtigte Anfragen, Reichweite/Verdeckung, geschuetzte Edits, Save/Reload
und spaetere Beitritte. Ein maximal zulaessiger Stand mit 256 Kingdoms und
jeweils 64 Unicode-Mitgliedern wurde als **2.763.543 Byte** grosse Nachricht
ueber den echten zuverlaessigen UDP-Pfad fragmentiert und wieder zusammengesetzt.

Die Modelltests laden alle zwanzig glTF-Dateien durch den wirklichen
Scene-Loader und pruefen Rezepte, physische Oeffnungen, 24 Posen, Journal,
Replikation und Drehungen in beiden Richtungen. Die native Modellgalerie prueft
zusaetzlich alle GPU-Vorschauen sowie echte Picking-/Kollisionsabfragen.

Die native Kingdom-Pruefung gruendet ueber den echten platzierten Block,
verwendet die konfigurierbare Benutzen-Aktion, tippt den Namen ueber
KeyboardInput und klickt Gruenden/Umbenennen mit dem echten OS-Mauszeiger.
Sie prueft den geschuetzten Abbau und wartet auf die Speicherbestaetigung.
Ein neuer Prozess laedt dieselbe isolierte Welt und dasselbe Profil, bestaetigt
ID, Name, Gruender, Mitglieder und Standort, klickt die Aufloesungsbestaetigung
und entfernt danach den freigegebenen Bergfried mit einem echten Serveredit.

Logs liegen unter [logs/](logs/); Quell-/Patchnachweise unter
[provenance/](provenance/). Profile, Wegwerfwelten und grosse Binaerdateien
bleiben ausserhalb der versionierten Nachweise.

Mit dem finalen optimierten Build bestehen beide Kingdom-Prozesse erneut
(sechs Ansichten, darunter [Gruendung](kingdom/kingdom-founded.png) und
[Aufloesungsbestaetigung](kingdom/kingdom-dissolve-confirm.png)). Die allgemeine
OS-Bedienpruefung besteht mit [25 Inventar-/Rezept-/Fokusansichten](inventory/).
Eine gesonderte [Projektionsgalerie](surfaces/) besteht mit 26 Aufnahmen:
Spielerinventar und zwoelf Block-/Maschinenlayouts jeweils breit und kompakt,
mit belegten Slots, geometrisch geprueften Grenzen und erhaltenem autoritativem
Spielerinventar. Diese Stationsgalerie verwendet validierte lokale Snapshots;
sie ist ein Darstellungstest. Die echte Truhen-A/B-Pruefung und die
Kingdom-Pruefung verwenden dagegen den wirklichen integrierten Serverpfad.

## Reproduktion

```sh
cargo test --workspace --lib --bins --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
python3 -m unittest discover -s tools/benchmarks -p 'test_*.py'
cargo build -p terra-client
```

Alle nativen Laeufe verwenden isolierte Konfigurations-, Profil- und Weltpfade.
Fuer die Modellgalerie `TERRA_MATERIALS_CASTLE=1`, `TERRA_PREVIEW_WARMUP=1`,
`TERRA_BENCH_GRAPHICS=1`, `TERRA_CAPTURE_DIR=<absoluter Ausgabeordner>` und
`--smoke-test-materials` setzen. `graphics.toml` muss vorhanden sein.

Kingdom pruefen: `TERRA_KINGDOM_SMOKE=1`, `TERRA_NATIVE_OS_POINTER=1`,
`TERRA_KINGDOM_SAVE_DIR=<absoluter isolierter Savepfad>`,
`TERRA_CAPTURE_DIR=<Ausgabeordner>` und `--smoke-test-inventory`.
Anschliessend denselben Profil-/Savepfad mit `TERRA_KINGDOM_RELOAD=1` erneut
starten. Die OS-Klicks verwenden den vorhandenen Wayland-/uinput-Helfer.
