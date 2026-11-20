# Holzfaeller, Stationen Und Setzlinge

## Einrichten

1. Eine zivile Arbeitskraft mit Rechtsklick oeffnen und **Holzfaeller** waehlen.
2. Arbeitsgebiet und Lager bestimmen und den Auftrag speichern.
3. Eine **Holzfaeller-Station** im Arbeitsgebiet oder direkt neben dem Lager
   platzieren. Die Suche beim Lager umfasst maximal vier Bloecke je X/Z-Richtung
   und zwei Bloecke Hoehenunterschied. Das Lager darf auch die Station selbst sein.
4. Eine echte Axt im gewaehlten Lager bereitstellen. Ein Setzling der zu
   bearbeitenden Baumart sichert die Nachpflanzung, falls keine Laubbeute anfaellt.

Seit Phase 238 darf dieser Vorrat auch im besuchten Holzfaeller-Tisch liegen:
Fehlen Axt oder passender Setzling im zugewiesenen Ausgabelager, holt die Arbeitskraft
sie physisch von der Station. Das Ausgabelager bleibt das Ziel fuer Ertraege.
Rechte, Erreichbarkeit und erlaubte Transportwege werden fuer beide Lager geprueft.

Ohne passenden Tisch zeigt der Auftrag **Holzfaeller-Station benoetigt**.
Ein normaler Lagerblock oder eine Schmiede zaehlt nicht als Holzfaeller-Station.
Der NPC laeuft zur Station und prueft dabei Weg, Reichweite und Rechte. Erst
nach dem tatsaechlichen Erreichen wechselt er seine Berufsbekleidung und nimmt
die Arbeit auf. Maennliche und weibliche Modelle verwenden das unveraenderte,
aus der permanenten NPC-ID bestimmte Geschlecht. Auch bereits sichtbare Avatare,
Werkzeuge und Portraits werden beim replizierten Berufswechsel erneuert.

## Arbeit Und Wiederaufnahme

Der Holzfaeller holt seine Axt aus dem Lager, erreicht eine reale Arbeitsseite
am Baum, faellt zugeordnete Staemme und raeumt die Krone schrittweise auf.
Die Baumwurzeln muessen im Arbeitsgebiet liegen; fuer ausladende Kronen gelten
weiterhin die echten Abbaurechte. Spielerbauten und fremde gepflanzte Baeume
werden nicht als eigener Wald abgeerntet.

Echte Holz- und Setzlingsdrops werden aufgesammelt und angeliefert. Danach holt
er einen passenden Setzling aus dem Lager und pflanzt bevorzugt an der alten
Wurzel oder an einem geeigneten nahen Platz innerhalb seines Gebiets nach.
Axtbenutzungen und Saatgut werden tatsaechlich verbraucht. Ohne Setzling wartet
der Nachpflanzauftrag auf Vorrat; es wird keiner erfunden.

Wird die Station entfernt, ungeladen oder unzugaenglich, stoppt die Arbeit.
Inventar, Teilbaum und ausstehende Drops bleiben erhalten. Nach Reload oder
Neuzuweisung wird die Station erneut gesucht und besucht. Die Suche bleibt
begrenzt: maximal 64 Kandidaten pro besuchter Arbeitskraft und Tick, aufgeteilt
zwischen Lagerumgebung und Arbeitsgebiet. Eine Station direkt beim Lager wird
dadurch schneller gefunden als am anderen Ende eines maximalen Arbeitsgebiets.

## Laubbeute

Jeder erfolgreich entfernte Laubblock hat eine **20-%-Chance** auf einen
Setzling. Derselbe stabile Koordinatenwurf gilt fuer normalen Abbau,
Gleisraeumung, verzoegertes Kronenaufraeumen und Holzfaellerarbeit. Wiederholte
oder fehlgeschlagene Entfernungsversuche erzeugen keine zusaetzlichen Wuerfe.

Zugeordnete natuerliche und gepflanzte Baeume liefern ihre tatsaechliche Art,
auch Fichten sowie Herbst- und Weisskirschkronen. Bei platzierten Blaettern ohne
Baumzuordnung bestimmt das Material die Art; das gemeinsame gruene Laubmaterial
verwendet Eiche als Standard. Die normalen Laubbloecke werden beim Abbau
verbraucht und nicht zusaetzlich als Block zurueckgegeben. Staemme geben keinen
garantierten Setzling mehr. Kleine Baeume koennen daher ohne Setzlingsbeute bleiben.

## Buttons Und Kompatibilitaet

Aktionsbuttons verwenden automatische Inhaltsbreite mit horizontalem Symbol
und Text. Sie fuellen keine komplette Zeile kuenstlich aus. Bei begrenztem
Fensterplatz umbrechen die Aktionszeilen; die Buttonbeschriftung bestimmt die
Breite statt einer pauschalen festen Pixelvorgabe.

Phase 237 verwendete gemeinsam **Protokoll 138** fuer den neuen
Stations-Wartezustand und fuehrte keine neuen Speicherfelder ein. Bestehende NPC-IDs,
Auftraege, Inventare und Baumfortschritte bleiben gueltig. Aktuell verwenden Client
und Server **Protokoll 140** und **ID25** fuer die zusaetzlichen
[Handwerker- und Hausbauauftraege](settlement-building.md).

Abnahme der Phase 237: 4763 erfolgreiche Workspace-Tests, Formatierung, Check, striktes Clippy,
Doctest-Pruefung und Client-/Server-Build. Die native Galerie bestaetigt neun
NPC-/Stationsaufnahmen und vier weitere DE/EN-Menueaufnahmen; der Server startet
ohne Display. [Pruefbericht und Aufnahmen](../tools/benchmarks/results/lumberjack-stations-2026-11-12/README.md).
