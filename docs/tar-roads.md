# Straßenbau mit Teer

Mit einer Holz- oder Steinschaufel wird die freie Oberseite von Gras, Erde,
Sand oder Kies per Rechtsklick zu einem Wegblock (Path, ID 102). Spirit- und
Blood-Gras sowie ihre Erdvarianten eignen sich ebenfalls. Steine, Holz,
Wasser, Ackerboden und bereits gebaute Straßen werden nicht umgewandelt.

Ein leerer Eimer und vier Kohlenteer-Bindemittel ergeben an der Werkbank einen
Teereimer. Das Bindemittel stammt aus dem bestehenden Ofenrezept für Kohlestaub.
Mit Rechtsklick auf die freie Oberseite eines Wegblocks wird der Eimer
geleert; im Survival bleibt der leere Eimer erhalten.

Der glänzende, zähflüssige Film fließt schrittweise auf angrenzende Wegblöcke.
Ein Eimer enthält 64 feine Volumeneinheiten. Die Simulation verteilt diese
vorhandene Menge auf zusammenhängende Wege bis vier Blöcke vom Ausguss entfernt
und gleicht deren Pegel bis auf eine feine Einheit an. Sie erzeugt keine
unbegrenzte Quelle. Kleine, bezahlte Nachbartransfers lassen die Front schrittweise
wachsen; ein neuer Wegblock übernimmt nicht sofort die volle Schicht. Unvorbereiteter Boden, Hindernisse
und Wasser stoppen die Ausbreitung. Die Schicht liegt direkt auf der
abgesenkten Wegoberfläche. Weniger Teer ergibt eine dünnere Schicht.

Nach ungefähr einer Minute in einem geladenen Weltbereich wird aus dem
flüssigen Film ein matter, fester Straßenbelag. Ausgeladene Bereiche pausieren
den Fortschritt und setzen ihn nach dem Laden fort.
Menge und Trocknungsfortschritt werden mit dem bearbeiteten Block gespeichert
und an andere Spieler übertragen. Flüssige und getrocknete Beschichtungen sind
Weltzustände; im Inventarkatalog werden dafür Wegblock und Teereimer angeboten.
Die vorhandenen Asphaltblöcke, Rampen und Randsteine bleiben eigenständige
Bauteile für Straßenaufbauten und bestehende Bauten.

Wegbeschichtungen verwenden die Material-IDs 437 und 438. Der Teereimer trägt die
Werkzeug-ID `tar_bucket`. Client und Server verwenden Protokoll 152. Beim Bauen
und Abbauen überträgt der Client den angeklickten Oberflächenpunkt.

## Galle und Bürgersteige

Sandwürmer spucken Galle nur auf einen erkannten, verwundbaren Gegner mit freier
Schussbahn. Das Ziel wird vor dem angekündigten Angriff festgelegt; Ausweichen ist
möglich. Das Projektil trifft
bestätigtes Terrain und hinterlässt eine echte Flüssigkeitsquelle (439). Sie
fließt wie Wasser nach unten und horizontal höchstens vier Blöcke vom Ursprung,
mit der halben Wassergeschwindigkeit. Unbekanntes Terrain, Wände und Wasser
stoppen die Front. Ausgespuckte Quellen verschwinden nach fünf Sekunden, auch wenn
der Bereich zwischenzeitlich ausgeladen wird; verbleibende Flüssigkeit fließt aus.
Ein leerer Eimer kann die Quelle vorher aufnehmen; ein vom Galleeimer gesetzter
Ausguss bleibt bestehen. Säurekontakt verursacht bei Spielern im Survival und bei
Mobs Schaden über den normalen Lebenspunkte- und Todesablauf. Die tatsächliche
Flüssigkeitshöhe entscheidet über Kontakt; Kreativspieler bleiben geschützt.
Eine feste globale Zellgrenze zählt auch ausgelagerte Pfützen mit.

Galle verwendet 64 feine Volumeneinheiten pro Zelle. Wasser, Galle und Teer geben
Menge über lokale Flüsse weiter, anstatt benachbarte Blöcke unmittelbar zu füllen.
Gefälle und freie Verbindungen bestimmen die Richtung. Die sichtbare Front wächst
innerhalb der Zelle; Wasser und Galle besitzen bewegte Oberflächennormalen.
Der gespeicherte Weltzustand hält Quellen, Mengen, Reichweite und Trocknung fest.

Meere, Seen, Flüsse und bestätigte größere Wasserbecken werden als zusammenhängende
Gewässer behandelt. Ihre vorbereitete Geometrie enthält Ufer, Wassertiefe,
Strömung und Wasserfallanschlüsse. Die Abschnitte behalten gemeinsame Pegel an
ihren Grenzen; kleine Oberflächenwellen verändern den mittleren Wasserstand
nicht. Das Wassermaterial bewahrt die Tiefenwirkung, Biomtönung, Brechung und
Reflexionen. Ein eigener Gewässer-Uploader übernimmt diese bereits vorbereiteten
Flächen. Der frühere zusätzliche Wasser-Uploader und die dauerhaften
Wasserfallsprüher bleiben abgeschaltet, sodass keine zweite Fläche darüberliegt.

Auch kleine Wasserausgüsse und Blood-Wasser verwenden die vorbereiteten,
an Bauteilen abgeschnittenen Wasserflächen mit derselben Tiefenwirkung und
Reflexion wie ruhende Gewässer. Der Server überträgt tatsächlich bezahlte Mengen;
das Rendering interpoliert Füllstand und Vorderkante zwischen diesen Zuständen.
Galle und Teer verwenden ihre eigenen bestätigten Oberflächen mit langsameren
Übergängen und wandernden, zähen Materialfalten. Teer bleibt eine dünne Schicht
auf vorbereiteten Wegen. Die Kollisionsformen von Platten und anderen Bauteilen
begrenzen den freien Raum. Getrennte Becken über eine massive Trennzelle hinweg
übernehmen keine Animationspunkte voneinander.

Die Kameraposition bestimmt weder Flüssigkeitsmenge noch Form. Eine zweite,
kameranahe Partikelsimulation wird für gespeicherte Weltflüssigkeiten nicht
gezeichnet oder berechnet. Der GPU-Solver bleibt in einer gesonderten Testszene
für freie Partikelausgüsse verfügbar. Unveränderte Flächen bleiben beim
Herumlaufen und Überfliegen erhalten.
Neue Abschnitts- und Flüssigkeitsveröffentlichungen lösen begrenzte Aktualisierungen
aus; die vorbereiteten Gewässer werden dabei nicht erneut als ganzes Volumen
abgetastet. Auch gespeicherte entfernte Flüssigkeit bleibt sichtbar.

Flug und Bewegung einer freien Kamera erzeugen keine Wasserkontakte. Tatsächliche
Spieler und Mobs hinterlassen bei Kontakt kleine, kurzlebige Wellen. Passende
Eintritte können wenige Spritzer mit ballistischen Flugbahnen erzeugen; das
Gewässer bleibt dabei zusammenhängend und behält seinen mittleren Pegel.
Quellen, Eimer, Reichweite, Säureschaden und Trocknung verwenden weiterhin den
bestätigten Weltzustand. Nach einer bestätigten Entleerung zieht sich die
sichtbare Fläche kurz zurück und gibt anschließend ihre Geometrie frei.
Kontakt mit der benetzten Weltfläche bremst Spieler und Mobs;
trockener Teer und der feste Galleblock bremsen nicht.

Platziertes Wasser benötigt eine Verbindung zu einer bestehenden Quelle. Nach
dem Entfernen der letzten verbundenen Quelle ziehen sich auch eingeschlossene
Pfützen zurück. Eine zweite Quelle erhält das angeschlossene Wasser. Gespeicherte
Wasserverbindungen über ausgeladene Bereiche bleiben berücksichtigt; natürliches
Gewässer wird nicht allein wegen eines ungeladenen Nachbarchunks gelöscht.

Galleblock (440), Bürgersteigblock (441), sechs Bürgersteigformen (442–447) und
Strohblock (448) sind im Baukatalog und über Werkbankrezepte verfügbar. Die
Bürgersteigformen bieten gerade, linke/rechte Kanten, linke/rechte Bögen und einen
abgesenkten Übergang. Ihre Bordseite richtet sich beim Platzieren zu benachbartem
Asphalt oder Weg aus. Rendering, Auswahl und Kollision teilen dieselben Profile.
Die Kanten und Bögen liegen bündig auf der Gehfläche; ihre etwas dunkleren
Steine markieren den Straßenrand ohne zusätzliche erhöhte Leiste. Die abgesenkte
Form verbindet die Asphaltdecke mit der einen Viertelblock hohen Gehfläche.
Der massive Bürgersteigblock ist einen halben Block hoch. Beide Bögen besitzen
eine echte Viertelkreisfläche. Durch die freie Ecke lässt sich der Boden mit der
Schaufel zum Weg formen. Teer erreicht diesen Weg über die beiden offenen Seiten
und trocknet dort auf gleicher Höhe wie die angrenzende Straße.
Farb- und Normaltexturen sind gemeinsam genutzt und wiederholbar erzeugt:
`python3 tools/assets/build_street_blocks.py`.

Feine Teermenge, Alter und Galleursprung werden im RG15-Regionsformat gespeichert;
die älteren eingebetteten Objektlayouts bleiben lesbar. Client und Server müssen
für den erweiterten Flüssigkeitszustand dieselbe Protokollversion verwenden.
