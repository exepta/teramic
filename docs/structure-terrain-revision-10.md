# Geländeangepasste Strukturen – Revision 10

Neue Welten verwenden `structures_revision = 10`. Gespeicherte Revisionen 0–9
behalten ihre bisherigen Planer und Blockkoordinaten. Es findet keine Bearbeitung
bestehender Weltdaten statt.

## Fundamente und Gelände

Friedhöfe erhalten keine durchgehende 53×53-Plattform mehr. Die Einfriedung folgt
jeweils der natürlichen Bodenhöhe, einzelne Grabgruppen und die Kapelle haben
kleine örtliche Fundamente. Der Mausoleumsboden liegt auf dem Median der Höhen
seines eigenen Grundrisses. Maximal drei Meter Höhendifferenz werden dort durch
Aushub oder Aufschüttung ausgeglichen. Grabgruppen erlauben einen Meter, die
Kapelle zwei Meter. Ungeeignete Flächen werden verworfen. Zwischen den Bauteilen
bleiben Boden und Vegetation erhalten; eine Struktur leert nicht mehr pauschal
das ganze Friedhofsvolumen.

Königreiche und Außenposten füllen ebenfalls keine kompletten Terrainkreise mehr.
Häuser bestimmen ihre örtliche Höhe aus dem jeweiligen Grundriss, Außenmauern
folgen dem Gelände. Fundamente sind auf vier Meter begrenzt; bestehende tragende
Geschossdecken können aufgesetzte Türme tragen. Wege gleichen maximal zwei Meter
gegenüber dem natürlichen Boden aus und verwenden gerichtete Treppenblöcke.
Bei zu steilem Gelände wird ein Standort verworfen, statt einen hohen Sockel zu
konstruieren. Gewöhnliche Dorfhäuser verwenden eine mittlere lokale Bodenhöhe
und höchstens sechs Meter Gesamtrelief im Grundriss.

## Grabdungeons

Die drei Varianten behalten ihre seedabhängigen Namen und drei beziehungsweise
vier Untergeschosse. Jede Etage hat einen eigenen Grundriss, unterschiedliche
Raumgrößen, Deckenhöhen und einen architektonischen Zweck:

- Ritter: Wachhalle/Waffenkammer, Familiengrüfte/Beinhaus, große Grabhalle.
- Priesterin: Prozessionshalle, Kreuzgang, Reliquienräume, Sanktuarium.
- Gelehrter: Bibliothek/Studierzimmer, Archive, Nebenbibliothek, Codexgewölbe.

Einzelne Räume erhalten jeweils Boden, Wände und Decke. Dazwischen verbinden
schmale, ausgekleidete Gänge und echte Treppen die Räume. Das Gestein außerhalb
dieser Raum- und Ganghüllen bleibt unverändert; es wird kein gemeinsamer massiver
Dungeonquader erzeugt. Die erste Treppe reicht abhängig vom Gelände 12–24 Meter
hinab, weitere Etagen liegen acht Meter auseinander. Jede Raumdecke muss unter
dem natürlichen Terrain liegen. Raumvarianten und Spiegelung sind deterministisch.

Jede Etage enthält eine echte Truhe mit dem vorhandenen Inventar-/Persistenzpfad.
Dekoration hält die reservierten Wege frei. Die Raumzwecke sind Architektur und
Ausstattung; damit werden keine neuen Quest-, Rätsel- oder Fallenmechaniken
behauptet.

## Schnittstellen und Prüfung

`SiteSummary.grave` enthält den optionalen aktuellen `GraveDungeonInfo`-Plan.
`grave_dungeon()` liefert für alte Standorte weiterhin deren Legacy-Geometrie.
Die zusätzlichen `floors` beschreiben Räume, Landepunkte und freie Kamerapositionen.
Der native Grave-Harness verwendet diese Kamerapositionen sowie die individuellen
Truhenspalten. Das Finder-Beispiel muss für die aktuelle Revision neue natürliche
Standorte ermitteln; alte Regionen sind kein garantierter neuer Standort.

Regressionen prüfen deterministische Patches, unterschiedliche Etagen,
zusammenhängende freie Wege, tatsächliche Drittelstufen-Kollisionshöhen,
gefüllte Truhen, erhaltenes Gestein zwischen Räumen und begrenzte Fundamente.
Build- und Testresultate werden im Abschlussbericht separat ausgewiesen.
