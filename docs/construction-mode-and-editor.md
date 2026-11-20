# Baumodus, Stadtlager und Building Editor

In eigenen Bau-Chunks zeigt die Leiste links neben der Minimap die bestaetigten
Bestandssummen fuer Holz, Stein, Eisen und Kohle. Die Quelle ist das im Bergfried
zugewiesene Stadtlager mit wirklichen Truhen. Doppeltruhen liefern beide
Inventarhaelften, werden dabei nicht doppelt gezaehlt. Unvollstaendig geladene
Lager werden gekennzeichnet; es werden keine entfernten Bestaende geschaetzt.

Die vier Summen sind eine Uebersicht. In der Materialliste einer Vorlage stehen
die **exakten Gegenstaende** als **verfuegbar / benoetigt**, beispielsweise
`100 / 50` Eichenbretter. Holzarten und Bretter/Stamm-Inputs bleiben fuer den
Bau unterschiedliche Items. Die Vorlage zaehlt ihre dauerhaften Bauteile;
standortabhaengige Bodenvorbereitung und Hilfsgerueste kommen im Auftrag hinzu.

## Top-down-Baumodus

- **TAB** schaltet die Baukamera in eigenen Bau-Chunks mit Arbeitsverwaltungsrecht um.
  Die Spielfigur bleibt an ihrem Standort; es findet kein Kamerateleport des
  Spielers statt. Tod, Sitz-/Schlafzustand oder verlorene Rechte beenden den Modus.
- **WASD** verschiebt die Kamera innerhalb der eigenen Bau-Chunks, das Mausrad
  zoomt. Nebel und Clip-Distanz beruecksichtigen die erhoehte Kamera auch bei
  kleiner Sichtweite und werden nach dem Modus wieder zurueckgesetzt.
  Das Scrollen ueber einem UI-Bereich bedient dessen Liste.
- **Bauvorlagen** oeffnet den echten Serverkatalog mit Ordnerkategorien.
  Die gewaehlte Vorlage erscheint als transparente orange Silhouette.
- **R** beziehungsweise die Drehschaltflaeche waehlt eine der vier Ausrichtungen.
  Linksklick auf geeigneten Boden beauftragt die Handwerker. Rechtsklick bricht
  die laufende Platzierung ab.
- Alternativ bestaetigt **Eingabe/Enter** die angezeigte Position. Die Schaltflaeche
  **Vorlage platzieren · Eingabe** im unteren Bau-Hinweis bietet dieselbe Aktion.
  Sie verwendet die zuletzt ausgewaehlte Position, wenn die Maus zur Schaltflaeche
  bewegt wird. Ohne Bodenpunkt/Vorschau und waehrend einer Serveranfrage bleibt
  sie deaktiviert. Die orange Vorschau allein ist keine Zusage der Bauplatzpruefung.
- Ohne aktive Vorlagenplatzierung oeffnet ein Linksklick auf einen eigenen zivilen Bewohner dessen vorhandenen
  Arbeits-/Inventardialog. Private Daten und Auftraege bleiben serverseitig
  berechtigt. **ESC** schliesst zuerst diesen Dialog; danach beenden ESC oder TAB
  die Baukamera.
- Gespeicherte Arbeits-, Lager-, Wohngebiete und Baupositionen werden automatisch
  eingeblendet. Rot markiert den aeusseren Rand der eigenen Bau-Chunks; gemeinsame
  Kanten benachbarter Bau-Chunks werden nicht doppelt gezeichnet. **H** blendet
  alle Gebietsmarkierungen gemeinsam aus und wieder ein. Beim erneuten Betreten
  des Baumodus sind sie wieder sichtbar; die manuelle Gebietsauswahl bleibt erhalten.
  Die Taste wirkt nicht in Dialogen oder zusammen mit F1.
- Die Markierungen folgen dem geladenen Boden als niedrige Nebelvolumen mit
  weich wandernder räumlicher Dichte und Tiefenprüfung. Sie bilden keine hohen Drahtkaesten. Unbekanntes Terrain wird fuer
  die Anzeige weder generiert noch durch eine geschaetzte Flaeche ersetzt.

Der Bergfried besitzt zusaetzlich **Im Baumodus platzieren** bei der gewaehlten
Vorlage. Die bisherige Standortauswahl bleibt als alternative Bedienung erhalten.
TAB ersetzt die alte Standardbelegung fuer die Cursorfreigabe; diese liegt nun
auf der Taste links neben 1 (`Backquote`). Alte Standardprofile werden migriert,
eigene kollidierende Tastenbelegungen bleiben erhalten.

Ein Klick erzeugt einen echten Bauauftrag. Der Server prueft weiterhin Claims,
geladenen Standort, Hindernisse, Vorlagen-ID und Rechte. Die Silhouette setzt
keine fertigen Weltbloecke. Fehlendes Material pausiert die normale Ausfuehrung,
bis geeignete Vorratstruhen befuellt und erreichbar sind.

NPCs, Tiere und kleine Pflanzen verhindern die Vorlagenplatzierung nicht. Solange
die orange Vorlage aktiv ist, trifft der Platzierungsklick den Boden statt den
Bewohnerdialog. Explizit freigeraeumte Luft und bis zu vier Bloecke hohes
unveraendertes natuerliches Terrain sind erlaubt; Handwerker ebnen diese Stellen
durch echte gespeicherte Vorbereitungsschritte ein. Gespeicherte Erde und Gras
unter der Fuss-/Ankerebene gelten als vorhandener Baugrund. Nur tatsaechliche
Boden-/Fundamentzellen der Vorlage werden dort vorbereitet; Erde neben dem Haus
bleibt erhalten. Dies umfasst auch Erde, die zuvor durch Graswachstum oder
die Abdeckung durch andere Bloecke ins Aenderungsjournal gelangt ist.
Baeume, Gebaeude, andere gesetzte Bloecke, oberirdische Erd-Edits, Wasser und
groessere Hoehenunterschiede bleiben Hindernisse. Bei der spaeteren wirklichen Blockplatzierung
warten Handwerker weiterhin, bis Spieler und Tiere die betroffene Stelle verlassen.

Fuer eine schreibgeschuetzte Pruefung eines echten Spielstands:

```sh
cargo run -p terra-server --example inspect_building_world -- worlds/Test 205294890893140149 16 16 24
```

Die Argumente sind Spielstandverzeichnis, dezimale Welt-ID, Anker X/Y/Z,
optional Ausrichtung 0..3 und Vorlagen-ID. Das Werkzeug liest die wirklichen
Weltparameter und Blockaenderungen, generiert nur die benoetigten Sections und
ruft dieselbe Planung wie der Server auf. Es aendert keinen Weltinhalt und
zeigt vorhandene Bauauftraege sowie Material-/Journalwerte am Standort.
Ablehnungen enthalten den ersten konkreten Grund mit Weltkoordinate, geladenem
und gespeichertem Material oder der Nummer eines reservierenden Bauauftrags.
Der Server schreibt dieselbe Diagnose bei abgelehnten Platzierungsanfragen ins
Log, zusammen mit Anker, Ausrichtung und Vorlagen-ID; der Netzwerkfehler bleibt
kompatibel. Insbesondere kann eine Truhe ausserhalb des sichtbaren Hauses noch
im benoetigten Geruest-/Arbeitsrand liegen.

Im autorisierten **Creative-Modus des Bauauftrag-Besitzers** brauchen Handwerker
weder Material noch ein Stadtlager. Die UI zeigt `∞`. Es entstehen keine
kostenlosen Inventarstapel; unbezahlte Gerueste koennen nicht als Beute
zurueckerstattet werden. Bereits wirklich bezahlte Vorrats-/Geruestmengen bleiben
beim Moduswechsel erhalten. Verliert der Besitzer diese Freigabe, pausieren
freie Auftraege. Bauen bleibt eine sichtbare Handwerkeraufgabe mit den vorhandenen
Arbeitszeiten, Navigation und maximal vier Arbeitern pro Haus.

## Eigene Vorlagen und Ordner

Der Katalog liest rekursiv `assets/buildings/`. Bestehende Dateien unter
`assets/buildings/templates/` bleiben kompatibel; der zusaetzliche alte Ordnername
`templates` wird bei der Kategorienanzeige ausgeblendet.

Beispiel: `assets/buildings/Wohnhaeuser/Fachwerk/mein-haus.json` erscheint in der
Kategorie `Wohnhaeuser/Fachwerk`. Mit **Vorlagen aktualisieren** beziehungsweise
dem Aktualisieren-Button im Bergfried wird der serverseitige Katalog ohne
Spielneustart atomar neu geladen. Normale HUD-Bestandsabfragen lesen keine
Vorlagendateien. Ungueltige Dateien erzeugen einen sichtbaren Katalogfehler;
bereits gespeicherte Bauauftraege behalten ihre aufgeloesten Plaene.

Dateischema, Materialpruefung und Groessenlimits:
[Gebaeude-Assets](../assets/buildings/README.md).

## Eigenstaendige Editor-App (F82)

```sh
cargo run -p terra-building-editor
cargo run -p terra-building-editor -- --load assets/buildings/templates/village/residential.json
```

Die App startet direkt in einer lokalen leeren Bauwelt, ohne Spielhauptmenue,
Multiplayerverbindung oder Server. Die geschuetzte Struct-Bauhilfe startet
mit 9 x 9 Bloecken (StructureStone, ID 43) und wird nicht exportiert. Der goldene Umriss zeigt die aktuelle
Wohnhaus-Huelle relativ zum Anker: X -4..4, Y -2..9, Z -6..4. Y=-1 ist der
Zimmerboden, Y=0 die erste Wand-/Fusshoehe; die unterste Bauebene Y=-2 ist das
Fundament. Die separate Bauhilfe liegt darunter auf Y=-3 und kann nicht per
Blockabbau oder Platzierung veraendert werden. Die erste darauf gesetzte
Modellschicht liegt auf der Fundamentebene Y=-2.

**ESC > Optionen > Groesse der Bauhilfe** bietet 9x9, 17x17, 25x25 und 33x33.
Diese Workspace-Einstellung wirkt sofort, bleibt innerhalb der Editorsitzung
bei Laden/Neuanlegen erhalten und veraendert weder Modell noch Materialkosten.
Sie wird nicht im JSON gespeichert; die Schema-1-Vorlagengrenzen bleiben gleich.

Maus und WASD steuern den Flug, Leertaste/Shift steigen und sinken. **E** oeffnet
den wirklichen Spielinventarkatalog. Rechtsklick platziert, Linksklick baut ab;
Tueren, Betten und weitere Objektpaare werden zusammen behandelt. Werkzeuge und
Nahrung werden nicht in erfundene Baubloecke umgewandelt.

ESC bietet Fortsetzen, Optionen, JSON laden, JSON speichern und Beenden.
Der Dateibrowser zeigt wirkliche Verzeichnisse und JSON-Dateien. Anzeigename und
Dateiname werden beim Speichern angegeben; bestehende Dateien werden erst nach
ausdruecklicher Ueberschreibbestaetigung atomar ersetzt. Ein neuer, noch nicht existierender Dateiname erzeugt eine neue ID; bestaetigtes
Ueberschreiben behaelt die ID der gueltigen Zieldatei.
Doppelte IDs werden abgewiesen. Ein leerer Entwurf ist keine fertige Vorlage.

Der Export leitet geordnete Bauphasen und exakte Materialmengen aus den gesetzten
Voxeln ab und validiert das Ergebnis durch einen erneuten JSON-Import. Aktuelle
Objektformen, Treppenausrichtungen, Slabzustaende und Truhenpaare verwenden die
gemeinsamen Spielregeln. Individuelle RGB-Faerbungen oder Holz-/Schienenzustaende
ohne verlustfreie Schema-1-Darstellung werden konkret gemeldet.
Weitere Bedienung und Struktur: [Editor-README](../apps/building-editor/README.md).

## Truhen und Kompatibilitaet

Einzel- und Doppeltruhen duerfen beliebig nebeneinander stehen. Eine schon
verbundene Doppeltruhe wird nicht in eine Dreiergruppe erweitert und blockiert
keinen neuen Nachbarn. Shift auf Boden haelt die neue Truhe getrennt;
Shift direkt an einer passenden freien Truhenseite waehlt nur diesen Partner.
[Genaue Regeln und Rechte](chest-placement.md).

Client und Server benoetigen gemeinsam **Protokoll 142**. Speicherindex **ID26**
uebernimmt die eingefrorenen ID24-/ID25-Daten und ergaenzt die Creative-Finanzierung
ohne Verlust von Plaenen, Vorrat oder Fortschritt. Der Editor verwendet dieselbe
versionierte JSON-Vorlagendefinition wie der Server. Die Simulation laeuft
weiterhin in geladenen Weltbereichen; eine Kameraansicht ist keine Fernsimulation.
