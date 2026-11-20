# Blockgrafik und gestreamtes Licht

## Behobene Ursachen

Die Lichtsuche behandelte jede nichtleere Section als möglichen Lichtträger. Bei
3.179 bereits geladenen Sections mussten deshalb Erd- und Steinbereiche das
Budget von 128 neuen Sections je 100 ms durchlaufen, bevor entfernte Lampen
entdeckt wurden. Neue Section-Publikationen warteten außerdem auf den nächsten
Timer. Der Fernlicht-Batch schloss Lichtblöcke außerhalb einer festen ID-Liste
aus, darunter die Spirit-Laternen.

`BlockSection::may_contain` liefert jetzt einen konservativen, konstant schnellen
Abwesenheitsnachweis. Dafür werden 256 Material-Buckets mit jeweils einem
16-Bit-Zähler gepflegt: 512 Byte zusätzlich pro Section, keine Änderung des
Speicherformats. Kollisionen führen höchstens zu einer zusätzlichen Prüfung;
Einfügen, Entfernen und Wiederherstellen einer Section aktualisieren den Index
vollständig. Journal-Änderungen und leuchtende Welt-Props umgehen den negativen
Nachweis, damit nachträglich platzierte Lampen nicht verloren gehen.

Nachweislich lichtfreie Sections blockieren die Warteschlange nicht mehr. Für
echte Kandidaten gilt weiterhin das Budget von 128. Publikationen aktualisieren
Lichtauswahl und Fern-Batch sofort im nächsten verarbeitenden Frame. Entfernen,
Ersetzen, Nachbarfreigabe und Entladen werden weiterhin berücksichtigt. Der
Fern-Batch verwendet die tatsächlichen JSON-Lichtdefinitionen und die bestehende
weiche Entfernungsblende. Unverändert bleiben 4/8/12/16 lokale PBR-Lichter und
128/256/384/512 kleine, tiefengeprüfte Fernlicht-Scheiben je Qualitätsstufe;
es kommen keine volumetrischen Lampen hinzu.

Die zehn Krypta-Lichtdefinitionen besitzen nun einen modellbezogenen `offset`.
Die Lichtquelle und ihre entfernte Darstellung sitzen auf der Flamme bzw. dem
leuchtenden Kristall. Die frühere allgemeine Ausweichposition in einem
benachbarten Luftblock konnte eine sichtbare Lichtscheibe neben der Lampe erzeugen.

## Geometrieprüfung

`python3 tools/assets/model_polish/audit.py --output /tmp/block-audit.json`
prüft standardmäßig alle vorhandenen Block-IDs bis 399. Erfasst werden die 399
Definitionen und 245 referenzierten Modelle: Texturpfade, Attributlängen, endliche
Werte, Indizes, Normalen und Tangenten, degenerierte Dreiecke, Culling-Grenzen
und Orientierung der Dreiecke gegenüber ihren Vertex-Normalen. Die gespeicherte
Ausgabe `assets-after.json` enthält 319.494 Dreiecke, keine Definitions- oder
Geometriefehler und keine verbleibenden Geometriewarnungen.

Der Audit fand bei Bergfried und Königsmauer jeweils 32 falsch ausgerichtete
Dreiecke im konkaven Anker-Wappen. Eine Dreiecksfächer-Triangulierung hatte die
konkave Kontur überkreuzt; die Normalen stammten nur aus der ersten, konkaven Ecke.
Der Generator verwendet jetzt die gesamte Kontur und Ear Clipping für konkave
Flächen. Ausschließlich die Anker-Primitive der beiden vorhandenen Modelle wurden
repariert. Vertexpositionen, Außenmaße, Kollisionsgeometrie und Materialien blieben
gleich. Die ursprüngliche und korrigierte Audit-Ausgabe liegen unter
`tools/benchmarks/results/block-graphics-light-loading-2026-11-14/`.

Der Audit belegt strukturelle Gültigkeit, keine pauschale künstlerische Freigabe
jedes Modells. Neue Blood-Forest-Assets 400–421 wurden getrennt geprüft:
`tools/benchmarks/results/blood-forest-terrain-2026-11-14/assets-tests.txt`
belegt drei erfolgreiche Python-Tests für Katalog/Texturen, unterschiedliche
Ober-/Seitenflächen und echte GLB-Grenzen, Normalen sowie degenerierte Flächen.

## Validierung

Die Python-Suite `python3 -m unittest discover -s tools/assets/model_polish`
enthält zusätzlich die vollständige Assetprüfung, exakte Flächenabdeckung beider
gespiegelter Ankerarme und die Bindung der Lichtoffsets an die emissive Geometrie.
Rust-Regressionsfälle prüfen Hash-Kollisionen, Save/Load-Gleichheit,
Journal-Nachträge, sofortige Publikationen, 3.375 nichtleere Terrainsections vor
einer entfernten Kryptalampe sowie die weiterhin begrenzte Kandidatenwarteschlange.

Für native Aufnahmen protokolliert `run-grave-sites.sh` in der vorhandenen
`*-inspection.json` unter `lighting.*.streaming_timing` passiv die ersten Frames
mit tatsächlich geladenen Lampen, lokalen Lichtentitäten und sichtbarem Fern-Batch.
Außerdem werden tatsächliche Suchläufe, schnelle Ausschlüsse, Restwarteschlange,
Quellenzahlen und größte Fernlichtentfernung gemessen. Beim Wechsel in die Krypta
werden nur lokale Lichter auf Höhe der Kamera gezählt. Reine Cache-Zähler
ersetzen keinen Sichtnachweis.

## Native Messung vom 14. November 2026

Ausgewertet wurde der bereits abgeschlossene Lauf
`target/grave-sites/run.1eQyOU/captures/`: vier echte Standorte, Seed 42042,
Strukturrevision 10, keine Fixture-Edits. Alle vier Inspektionsberichte melden
`passed`. Die kompakte Auswertung mit Quelldateien und SHA-256 liegt unter
`tools/benchmarks/results/blood-forest-terrain-2026-11-14/light-streaming-summary.json`.

Die Framezahlen sind fortlaufende Aufrufe des `PostUpdate`-Beobachters ab Beginn
der Szene. Sie sind weder Millisekunden noch GPU-Präsentationszeitpunkte.

| Szene | Erste geladene Lampe | Erster Fern-Batch | Erste lokale PBR-Lampe | Nach Kamerawechsel: geladene Lampe → lokale Lampe auf Kamerahöhe |
| --- | ---: | ---: | ---: | ---: |
| Friedhof | 2 | 3 | 145 | 891 → 891 (0 Frames) |
| Eldric / Knight | 2 | 3 | 141 | 890 → 892 (2 Frames) |
| Maelis / Priestess | 2 | 3 | 125 | 888 → 893 (5 Frames) |
| Corvin / Scholar | 2 | 3 | 143 | 893 → 894 (1 Frame) |

Fernquellen standen in allen Szenen einen Beobachter-Frame nach den ersten
geladenen Lampen als nicht ausgeblendeter Mesh-Batch bereit. Das ist **kein
Nachweis, dass die volle lokale Beleuchtung bereits in Frame 3 sichtbar war**:
Lokale PBR-Lichter wurden beim ersten Laden erst in Frame 125–145 beobachtet,
also 123–143 Frames nach dem ersten geladenen Lampenblock. Die Messung trennt
die Ursachen dieser anfänglichen Verzögerung nicht weiter auf. Insbesondere
fehlen Zeitstempel für erste vollständige Sichtlinien, Materialbereitschaft und
tatsächlich präsentierte Lichtpixel.

Beim anschließenden Wechsel in die drei Krypten wurden passende lokale Lichter
nach 1–5 Beobachter-Frames erfasst. Als Referenz dient der erste Frame mit
geladenen Lampen in dieser neuen Kamerastufe; ein exakter Bewegungszeitpunkt
wurde nicht gesondert gemessen. Die Aufnahme-Pools enthielten jeweils 12 lokale
Lichter. Davon lagen 5 / 10 / 4 beim Knight / Priestess / Scholar innerhalb von
sechs Metern zur Kamerahöhe; der Pool war also nicht ausschließlich auf diese
Etage beschränkt. Die drei Nahaufnahmen hatten CaveBlend 1 und EV100 6,451563;
Friedhof und Übersichten CaveBlend 0 und EV100 13,7. Alle erfassten lokalen
Lichter hatten Schattenmaps und volumetrische Beleuchtung deaktiviert.

| Szene | Geladene Sections am Ende | Schnelle Ausschlüsse | Ausgeführte Quellenscans | Entdeckte Emittenten |
| --- | ---: | ---: | ---: | ---: |
| Friedhof | 3.179 | 2.751 | 428 | 2.765 |
| Knight | 3.182 | 2.699 | 501 | 2.908 |
| Priestess | 3.179 | 2.769 | 414 | 2.838 |
| Scholar | 3.179 | 2.641 | 547 | 3.612 |

Die Zähler sind kumulativ und können durch erneute Prüfungen mehr Einträge als
einmalig geladene Sections enthalten. In allen erfassten Stichproben war die
verbleibende Warteschlange leer. Ein bereits innerhalb desselben Frames
abgearbeiteter Zwischenstand wird dadurch nicht ausgeschlossen. Der Fern-Batch
blieb bei 384 Quellen; die größte erfasste Entfernung der Übersicht lag je nach
Szene zwischen 112,95 und 118,94 Metern. Das sind Quellen im Batch, nicht 384
garantiert unverdeckte Bildpunkte.

Die fertige Friedhofübersicht zeigt Lampen entlang der Terrassen; die geprüfte
Priestess-Nahaufnahme zeigt warm beleuchtete Wand- und Bodenflächen. Diese
Bilder belegen das Ergebnis nach dem Laden. Der Lauf enthält keine zeitlich
aufgezeichnete Annäherung und keinen vergleichbaren Vorher-Lauf. Deshalb folgt
daraus weder eine allgemeine FPS-Verbesserung noch die Aussage, dass alle
Startverzögerungen der lokalen Beleuchtung beseitigt sind.
