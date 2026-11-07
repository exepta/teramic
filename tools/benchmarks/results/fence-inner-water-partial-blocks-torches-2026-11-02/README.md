# Innere Zaunseite, Wasser um Teilbloecke und Fackeln

`references/correct-logic.png` markiert die gewuenschte innere Grenze am Zaun.
`references/water-problem.png` zeigt die Luecke um ein Rohr an einer Beckenwand.

Verbundene Zaunstreben begrenzen nun die vom Wasser erreichbaren Bereiche
innerhalb ihrer Zelle. Eine begrenzte Suche ueber die vier Bereiche am Pfosten
fuellt auch die innere Zaunecke und laesst den aeusseren Bereich trocken.
Punktabfragen, Schwimmphysik und Wassermesh nutzen dieselbe Grenze. Die
trockene Seite ist keine undurchsichtige Wand: die Wasserflaeche bleibt in
Strebenoeffnungen sichtbar. Bereits untergetaucht gesetzte Zaeune behalten
das vorhandene Wasser rundum.

Wasserfuellbare Teilbloecke erhalten vorhandenes Naturwasser und echte
Quellen. Endlicher Zufluss wird nicht in eine Quelle umgewandelt. Ein Rohr
direkt auf der ganzzahligen Wasseroberflaeche darf auch von seiner trockenen
Zelle aus das Wasser darunter nicht mit Terrain verdecken.

Fackeln koennen nicht in Wasser gesetzt werden. Wasser, das eine gesetzte
Fackel erreicht, entfernt sie ueber den normalen autoritativen Editpfad und
erzeugt genau einen Fackeldrop. Der Wasserzustand bleibt erhalten.

Die native Partial-Water-Pruefung enthaelt acht Ansichten, darunter eine
Zaun-Draufsicht und Teilmodelle unmittelbar an der Beckenwand. Sie prueft
die innere/aeussere Zaunseite, den Quellerhalt, abgelehnte Wasserplatzierung
einer Fackel und das einmalige Entfernen einer spaeter ueberfluteten Fackel.
Teilmodelle werden ueber den echten Editpfad gesetzt; ihr Wasser wird danach
nicht kuenstlich wieder eingesetzt.

**3721 Tests bestanden, 0 fehlgeschlagen, 8 ignoriert**, in 12 Testprogrammen.
Build, Clippy fuer alle Targets (`-D warnings`), Format- und Diffpruefung
bestanden. Die abschliessenden nativen Laeufe bestehen mit acht Aufnahmen in
14,80 s und 35 Hydraulik-Pflichtaufnahmen in 74,87 s. Beide Logs enthalten
keine Warnungen oder Fehler.

Direkte Bildpruefung:

- [Innere Zaunseite von oben](partial-water/fence-inward-water-top.png)
- [Wasser um Teilbloecke an der Wand](partial-water/waterlogged-wall-parts.png)
- [Rohre auf der Wasseroberflaeche in engen Steinshaechten](partial-water/waterlogged-enclosed-pipes.png)

Der erste Workspace-Lauf deckte bei einer erhaltenen Quelle einen zu hohen
Rueckgabewasserstand unter einer horizontalen Welle auf. Die Punktabfrage
verwendet nun das tatsaechlich verbundene freie Intervall. Der vorhandene
strenge Test blieb unveraendert und besteht im abschliessenden Gesamtlauf.
Das urspruengliche Fehlerlog ist als `logs/initial-span-regression-failure.log`
erhalten. Zwei aeltere Wasserlogging-Erwartungen mussten an die gewuenschte
Quellerhaltung angepasst werden: Ein Drain-Test sperrt seinen Zufluss jetzt
mit einem echten Vollblock; Teilbloecke erhalten vorhandenes Wasser. Tests
fuer endlichen Zufluss pruefen weiterhin, dass dabei keine Quelle entsteht.

`partial-preview/` enthaelt die vorgezogene Bildkontrolle vor der letzten
Intervallkorrektur. Fuer die Abnahme zaehlen ausschliesslich `partial-water/`
und `hydraulics/` aus dem abschliessenden Build. Diagnosebilder bleiben
getrennt von den 43 Pflichtaufnahmen gezaehlt.

Ergebnisse stehen in `verification.json`, Logs in `logs/`, verwendete Parameter
in `config/` und Quellhashes in `source-sha256.json`. Nach den Laufpruefungen
wurde ausschliesslich die Formatierung der korrigierten Intervallabfrage
angepasst; ausfuehrbarer Code blieb unveraendert.
