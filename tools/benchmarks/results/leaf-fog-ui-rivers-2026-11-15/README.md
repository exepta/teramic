# Laub, Menüs, Flüsse und Nebelblock – 15.11.2026

Blood Slime hat 70 % Deckkraft am Körper und ein opakes Gesicht. Die drei
Skelettvarianten haben offene Rippen, separate rote Pupillen und geglättete
Lauf-/Angriffsbewegungen. Elf Laubsprites mit kleineren Blattgruppen ergänzen
die vorhandenen Blocktypen. Vier geneigte Gruppen je freier Blockseite greifen
bis zu 25 cm über die Voxelgrenze; tatsächliche Geometriegrenzen und PNG-Masken
werden gemeinsam geprüft. Nahe Zellen benötigen maximal 192, mittlere 48
Dreiecke. Fernansichten und Inventar behalten ihre bisherigen Bilder.

Menükategorien verwenden die gemeinsame Breite des längsten tatsächlich
geformten Textes einschließlich Icons und Rändern. Deutsche Beschriftungen
behalten ganze Wörter; längere Hilfetexte können an Wortgrenzen umbrechen.
Die Prüfung berücksichtigt auch anonyme Textkinder innerhalb gestylter Labels.

Neue Welten speichern Flussrevision 5 und Ecology 8. 164 reale voxelisierte
Flussquerschnitte über drei Seeds messen 7,5–20,125 Blöcke innerhalb des
vorgegebenen Bereichs von 6–22. Seen, Mündungen und auslaufende Endkappen sind
keine konstant breiten Einzelrinnen. Alte Weltrevisionen behalten ihr Gelände.
Der feste Nebelblock 430 entsteht im Geistwald und funktioniert nach dem
Platzieren auch unter einer Grasdecke. Sein animierter Kreis hat maximal acht
Blöcke Radius. Bis zu 16 Quellen teilen ein gecachtes Höhenfeld und einen
Nebelpass; überlappende Quellen vervielfachen keine Dichte. Geiststein 308 hat
keine Emission und erzeugt kein Blocklicht. Client/Server verwenden Protokoll 146.

## Reproduzierbare Prüfung

Die finalen nativen Läufe verwenden einen identischen, per SHA-256 geprüften
Client. Jede Galerie hat eigene Konfigurations-, Profil- und Weltverzeichnisse.
Die Rohbilder und maschinenlesbaren Messungen stehen unter `captures/`;
`final-native-checks.json` enthält Befehle, Quellverzeichnisse und Bildprüfsummen.
`final-assets.json` bindet die Aufnahmen an die ausgelieferten Laubtexturen.

| Prüfung | Nachweis |
| --- | --- |
| Format, Workspace-Check, striktes Clippy nach letzter Geometriekorrektur | [Prüfläufe](geometry-final-checks.json) |
| Vollständige Rustsuite | [Rohlog](final-tests.log) |
| Separater Netzwerk-Fristtest und vollständiges Netzwerkpaket mit vier Threads | [Einzeltest](final-network-timeout-recheck.log), [637 erfolgreiche Tests](final-network-suite-recheck.log) |
| Zwölf gezielte Laubtests einschließlich echter perspektivischer Mesh-/PNG-Prüfung | [Rohlog](geometry-final-tests.log) |
| Client, Server und Building Editor | [Buildlog](geometry-final-build.log) |
| Finale Blattgruppen und bestehende Laubbilder | [3 Spritetests](dense-leaf-assets.log), [5 Bildtests](dense-leaf-face-tests.log) |
| Flussgeometrie und natürliche Nebelquellen | [Flusstests](river-tests.txt), [Quellentests](mist-source-tests.txt) |

Die vollständige Rustsuite hatte 5.184 erfolgreiche Tests und einen
Netzwerk-Fristfehler bei `invalid_movement_is_disconnected_without_affecting_the_other_player`.
Dieser Test bestand anschließend einzeln; das gesamte Netzwerkpaket bestand
danach mit 637 Tests und vier Testthreads. Die Produktionslogik und die
Fünf-Sekunden-Frist wurden dafür nicht geändert. Damit wurden die 5.185
regulären Tests dieses Stands erfolgreich ausgeführt. Nach der anschließenden
Korrektur der Blattkanten bestanden nochmals zwölf gezielte Laubtests,
einschließlich einer neuen perspektivischen Prüfung der tatsächlichen
ausgegebenen Mesh-Indizes, UVs und Texturmaske. Zehn vorhandene
ignorierte Tests bleiben ignoriert; ein Test benötigt eine fehlende externe
Original-GLB-Datei und wurde ausdrücklich ausgelassen. Die ursprünglichen
Fehlläufe bleiben im Verzeichnis erhalten.

## Native Aufnahmen

Die fünf Galerien prüfen insgesamt 31 Bilder: Laub und Augen (11), Menüs auf
Deutsch/Englisch in 1920×1080 und 960×640 (8), Nebel und Geiststein (4),
Waldkreaturen (4) sowie Birkenwald/Wald/Kirschwald (4).
Die kleine Menügröße ist angefordert; die tatsächlichen PNG-Maße sind im
Bericht gespeichert. Pro Achse ist höchstens ein Pixel Abweichung erlaubt,
und alle Aufnahmen eines Falls müssen exakt dieselben Maße besitzen.

- [Laub und Augen](captures/foliage/verification.json)
- [Gemessene Menükategorien und Beschriftungen](captures/menu/verification.json)
- [Vergrabene animierte Quelle, Abbau und überdachter Geiststein](captures/fog/verification.json)
- [Skelette und transparenter Blood Slime](captures/forest/verification.json)
- [Laub in tatsächlichen Waldregionen](captures/biomes/verification.json)

Die Nebelprobe setzt echte Blöcke bei deaktiviertem Biomnebel. Zwei Zeitpunkte
verwenden dasselbe Höhenfeld mit unterschiedlicher Drift; nach dem Abbau sind
Quellen-, Volumen- und Probenzähler null. Der Geiststeinvergleich steht unter
einem Dach neben gewöhnlichem Stein, ohne aktive lokale Punktlichter.

Die Bilder dienen der Funktions- und Gestaltungskontrolle. Sie sind weder ein
FPS-Vergleich noch eine Zusage pixelgleicher Referenzgrafik. Laubgruppen bestehen
aus beidseitigen Cutout-Flächen. Der Nebel verwendet eine Bodenlage pro X/Z-Probe
und bildet keine vollständige Strömung über mehrere Stockwerke ab.

Alle 34 Dateien unter `worlds/` stimmen mit ihren Prüfsummen vor den Arbeiten
überein: [Integritätsprüfung](worlds-after.json).

[Laubtechnik](../../../../docs/foliage-reference-surfaces.md),
[Nebelblock](../../../../docs/fog-block-and-spirit-stone.md),
[Weltrevisionen und Flussmessungen](../../../../docs/river-width-and-fog-sources.md).
