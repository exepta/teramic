# Blood Forest

Neue Welten verwenden `blood_forest_revision = 1`. Fehlende Felder in bestehenden
Welten werden als Revision 0 gelesen. Blood Forest ist das zehnte Oberflächenbiom;
Klima-, Farb-, Vegetations- und Wassergewichte gehen kontinuierlich ineinander über.
Die bestehende deterministische Höhen-/Flussgeneration bleibt die Grundlage.

Die Blutwald-Palette verwendet jetzt helleres Rosen-/Karminrot, warme Rinde und
mauvefarbenen Stein. Die eigenen Blattfarben bleiben auch im Jahreslauf erhalten;
Belichtung und Sonnenstärke werden dafür nicht verändert. Das Wasser ist heller
weinrot, ohne sein langsames Bewegungsbild zu verlieren.

Blutwald-Props 412–421 und Geistwald-Props 316–323 bestehen aus klar gestuften
Mikrovoxeln. Runde Renderflächen wurden durch achsenparallele Würfelflächen ersetzt;
Innenflächen werden entfernt und benachbarte Außenflächen verbunden. Details und
Modellbudgets: [Voxel-Prop-Generator](../tools/assets/voxel_props/README.md).

Neue Welten mit `ecology_revision = 5` verteilen Pflanzen in lockeren Gruppen und
erzeugen gelegentliche mehrzellige Felsformationen. Große Props halten mindestens
zwölf Meter Ankerabstand. Gespeicherte Revisionen 0–4 behalten ihre bisherige
Verteilung. [Verteilungsregeln und Felsgruppen](natural-features.md).

## Inhalt

IDs 400–421 liefern einzelne JSON-Dateien, GLBs und Texturen unter
`assets/terra/blocks`: Blutgras/-erde/-stein, Adernstein, Blutholz, Blutlaub,
Karminmyzel, Moos, Kristall-/Pilzblöcke sowie Pilze, Dornenbusch, Blume, hängende
Ranken, Wurzeln, Knochen, Farn, Riesenpilz, Ruinenbogen und Kristallsäule.
Gras/Myzel/Moos haben eigene Ober-, Unter- und Seitentexturen; Stämme eigenes
Stirnholz. Volle Terrainwürfel haben keine offenen Fasen. Props verwenden ihre
exportierten Kollisionsformen und speichern Drehungen wie die übrigen Dekorationen.

Blutholzbäume nutzen die verzweigte Baumgeneration und echtes rotes Laub. Ranken
hängen an tatsächlichen Ästen. Kräuter, Pilze, Wurzeln und kleine Ruinen wachsen
reproduzierbar auf geeignetem, trockenem Boden. Geöffnete Höhlen-/Flusspassagen
bleiben frei. Die neuen Pflanzen und Kristalle liefern passende Gegenstände;
Blutholz lässt sich zu Brettern verarbeiten.

Das Wasser ist dunkelrot, stärker absorbierend und bewegt sich optisch langsamer.
Das ist eine viskose Darstellung innerhalb des vorhandenen Wassersystems, keine
neue physikalische Flüssigkeitssimulation. Ein fester Pool aus höchstens 192
geteilten Tropfen animiert Wachstum, Fall und Auftreffen unter geladenen Blättern;
Suche, Reichweite und Darstellung sind begrenzt. Details:
[blood-forest-rendering.md](blood-forest-rendering.md).

## Kreaturen

Skelette (Art 24) haben drei Varianten: normal, Kämpfer mit Schwert und Wächter
mit Plattenrüstung. Sie erscheinen im Blood Forest und als Wächter in geladenen
Geistwald-Krypten. Raben und Blood Slimes sind friedlich; Blood Stalker, Blood Wisp
und Thorn Brute greifen Spieler über die serverseitige Navigation und vorhandene
Kampflogik an. Sichtlinien und Angriffspausen verhindern Schaden durch Wände.
Spawns und Tod verwenden stabile Identitäten und den vorhandenen Speicherpfad.

Glooms und Schmetterlinge besitzen neu aufgebaute, animierte 3D-Modelle nach den
Referenzen: zwei Gloom-Farben mit Armen, Flammenschweif und umlaufenden Flämmchen
sowie sieben Schmetterlingsfarben mit beweglichen, räumlichen Flügeln. Die
native Galerie zeigt die klare, facettierte Spielgeometrie; sie ist keine
pixelgenaue Umsetzung der weich beleuchteten Konzeptillustrationen.

Die neuen Arten ergänzen keine Zähmung, Zucht oder Fernkampfangriffe. Skelettwaffen
und Plattenrüstung gehören zur Modellvariante; sie sind keine ausziehbaren
Inventargegenstände. Die Varianten unterscheiden sich im Angriffsschaden und
in der Knochenbeute, besitzen aber dieselbe Grundgesundheit. Fluggegner verwenden
begrenzte lokale Hindernisproben; Bodenwesen nutzen die vorhandene Wegsuche.

Kryptawächter werden anhand geladener Kryptasteinböden im Geistwald ausgewählt,
bis zu zwei je Spawnzelle und innerhalb des Populationsbudgets. Auch von Spielern
platzierte passende Böden können diese Auswahl erfüllen. Ein eigener
Dungeon-Begegnungsmanager mit Bossen oder wiederkehrenden Wellen ist nicht
enthalten. Gespeicherte besiegte Wächter erscheinen durch erneutes Laden nicht
wieder.

## Im Spiel

Bei **Neue Welt → Biome → Blutwald** lässt sich das bevorzugte Startbiom wählen.
Für bestehende Welten mit `blood_forest_revision = 0` bleibt der gespeicherte
Generator erhalten; eine neue Welt erschließt die neue Verteilung. Der Chatbefehl
`/locate biome blood_forest` sucht einen passenden Bereich. Der angebotene
Teleport benötigt Creative oder die Umgebungsberechtigung des Servers.

Für gezielte Kreaturentests stehen mit derselben Berechtigung beispielsweise
`/spawn teramic::skeleton`, `/spawn teramic::skeleton_fighter`,
`/spawn teramic::skeleton_guardian` und `/spawn teramic::blood_stalker` bereit.
Weitere Arten heißen `raven`, `blood_slime`, `blood_wisp` und `thorn_brute`;
**Tab** vervollständigt die verfügbaren Namen nach `/spawn `. Skelette und die
feindlichen Blutwaldarten greifen angreifbare Überlebensspieler an; Creative
ist für die gefahrlose Modellansicht geeignet.

Blutlaub liefert ein Blutblatt und mit 20 % Chance zusätzlich dunkles Harz.
Kleine Blutpilze liefern einen Karmesinpilz und eine Spore, Riesenpilze drei
Pilze und zwei Sporen. Dornenbüsche geben zwei Dornenzweige und eine Blutbeere,
Blutblumen eine Blutbeere. Kristallblock und Kristallsäule liefern jeweils zwei
Blutkristallsplitter. Diese Beute gilt auch für automatisches Gleisräumen.
Beeren und Pilze können über die normale Nahrungsbenutzung gegessen werden.
Die neuen Blöcke und Gegenstände sind außerdem im Kreativinventar enthalten.

Die drei Skelettvarianten liefern zwei, drei beziehungsweise vier Knochen;
Raben einen Knochen, Blutschleime zwei oder drei Gel, Lauerer einen
Kristallsplitter, Irrlichter zwei Sporen und Dornenwächter drei Dornenzweige
sowie ein dunkles Harz. Einzelheiten zur Konfiguration und zu Modellvarianten:
[Kreaturenkatalog](../assets/terra/mobs/README.md).

## Daten und Kompatibilität

Neun Gegenstände ergänzen Beeren, Pilze, Dornen, Kristalle, Harz, Blätter, Sporen,
rotes Gel und Knochen. Beeren/Pilze verwenden die vorhandenen Nahrungsregeln.
Block-/Item-/Arten-IDs sind angehängt; Netzwerkprotokoll 144 verlangt passende
Client-/Serverstände. Kartenformat TRMAP005 speichert zehn Biomgewichte und liest
TRMAP001–004 ohne Verschiebung von Ufer-, Schnee- oder Erkundungsdaten weiter.

```sh
python3 tools/assets/build_blood_forest.py
python3 -m unittest discover -s tools/assets/blood_forest -v
bash tools/benchmarks/run-blood-forest.sh
```

Native Aufnahmen und Messergebnisse werden im
[Abschlussbericht](../tools/benchmarks/results/blood-forest-terrain-2026-11-14/README.md) dokumentiert.
