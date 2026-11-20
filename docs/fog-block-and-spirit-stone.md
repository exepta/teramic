# Nebelblock und Geiststein

Der neue feste, abbaubare Nebelblock hat die ID 430 und den Bezeichner
`teramic::fog_block`. In der deutschen Suche heißt er **Nebelblock**. Das ältere
Material 399 (`Mystic Mist Block`) bleibt unverändert. Nebelblöcke liegen in neuen
Geistwaldregionen meist unter der Grasdecke; sie können nach dem Abbau an anderer
Stelle platziert oder über das Kreativinventar ausgewählt werden.

## Nebel nach dem Platzieren

Eine Quelle erzeugt bewegten, bodennahen Nebel im Kreis mit maximal acht Blöcken
Radius. Der Rand wird innerhalb des Kreises weich ausgeblendet. Auch eine direkt
darüber liegende Gras- oder Hausbodenfläche lässt den Nebel über dem Boden
erscheinen. Die Suche folgt bis zu acht zusammenhängenden, geladenen Deckblöcken;
eine darüber liegende unabhängige Decke wird nicht als neuer Boden gewählt.
Benachbarte Spalten benutzen die zum Quellboden nächstgelegene reale freie Fläche.

Der Renderer liest tatsächliche geladene Blöcke und bestätigte Journaländerungen.
Platzieren, Abbau, Section-Wechsel und Replica-Reset aktualisieren den Quellenindex.
Nach dem Abbau verschwindet die zugehörige Fläche; andere Quellen bleiben aktiv.
Es werden keine Chunks durch die Nebelanzeige angefordert oder nachgeneriert.
Die Platzierung selbst verwendet weiterhin die normale Blockprüfung, Inventare
und Serverbestätigung.

## Kosten und Darstellung

In Kameranähe werden höchstens 16 Quellen ausgewählt. Sie teilen **ein** Nebelvolumen,
eine 41×41-Höhen-/Dichtetextur und die vorhandene 32 KiB-Rauschtextur. Das Höhenfeld
wird höchstens alle 0,5 Sekunden oder nach bestätigten Sectionänderungen ausgewertet.
Unveränderte Felder behalten Mesh, Material und Textur. Die GPU verwendet sechs
Strahlproben, begrenzt jeden Kreis exakt auf Radius 8 und vereinigt überlappende
Quellen über ihre höchste Dichte. Übereinander platzierte Steine vervielfachen
weder Volumenanzahl noch Nebeldichte. Es entstehen keine Punktlichter oder
Schattenwerfer für den Nebel. Tiefe und echte Bodenhöhen begrenzen die sichtbare
Dichte, auch innerhalb des Nebelvolumens.

Nebel auf mehreren übereinanderliegenden Stockwerken ist kein vollständiges
Raum-/Strömungsmodell: das gemeinsame Höhenfeld hält pro X/Z-Probe eine Bodenlage.
Quellen müssen im geladenen Luftraum und ungefähr innerhalb 32 Blöcken der Kamera
liegen. Die acht Blöcke Radius sind Weltabstand, keine Zusage sichtbarer Dichte
hinter Wänden oder außerhalb geladener Sections.

## Geiststein und Kompatibilität

Geiststein 308 hat keine flächige Emission mehr und erzeugt kein Blocklicht.
Seine türkisfarbenen Adern bleiben Materialfarbe. Laternen, Geisterblumen und
andere ausdrücklich leuchtende Dekorationen behalten ihre Beleuchtung. Auch der
Asset-Generator bewahrt diese Unterscheidung bei erneuter Erzeugung.

Protokoll 146 verhindert, dass ältere Clients den neuen festen Block 430 falsch
interpretieren. Natürliche Quellen gehören zur Ecology 8; gespeicherte ältere
Weltrevisionen werden beim Öffnen nicht umgeschrieben. Spieler können den neuen
Block auch in bestehenden Welten platzieren.

Prüfungen decken reale Quellen unter Gras, Abbau/Unloading, feste Körper,
Höhenkodierung, Dach-/Bodenunterscheidung, Radiusgrenze, Quellenbudget und lokale
sowie noch nicht bereite Netzwerk-Welten ab. Die native Prüfung mit
`tools/benchmarks/run-fog-block.sh` setzt echte Blöcke, vergleicht zwei Animations-
zeitpunkte, entfernt die Quelle und zeigt Geiststein/Stein in einer überdachten
Kammer ohne aktive Punktlichter. [Prüfbericht](../tools/benchmarks/results/leaf-fog-ui-rivers-2026-11-15/README.md).

[Natürliche Quellen und Flussrevision](river-width-and-fog-sources.md).
