# Phase 216: Zuglager und Kohle per Klick

Die Zugoberflaeche verlor Klicks, wenn Bilder oder Stapelzahlen eine eigene
Element-ID hatten. Ihr registrierter Handler sucht jetzt nach dem zugehoerigen
Aktionsbutton. Die Auswahl fuer den Laderaum greift ausserdem auf den aktuell
besetzten Wagen zurueck, wenn keine Fahrzeugzeile ausgewaehlt ist.

Ohne gehaltenen Gegenstand verschiebt ein Klick einen Stapel zwischen Spieler
und Behaelter. Ein gehaltener Stapel wird angezeigt und verwendet die normale
Ablegegeste; leere Zielplaetze sind dafuer aktiv. Der Motor akzeptiert weiterhin
nur echte Ofenbrennstoffe. Gehaltene Nichtbrennstoffe lassen sich ins
Spielerinventar zuruecklegen.

Oeffentliche Fahrzeugrevisionen duerfen private Inventare weder ersetzen noch
deren Schreibberechtigung aktualisieren. Die Ansicht behaelt ihren privaten
Stand samt urspruenglichem Token. Private Brennstoffmeldungen ersetzen ihn;
ein veralteter Klick wird vom Server abgelehnt und nach frischem OpenCargo
hoechstens einmal wiederholt. Verbindung, Besitzer, Fahrzeug, Teiltyp, Cursor,
Quellstapel und Zieltyp muessen weiterhin passen. Schliessen, Zurueckgehen,
geaenderte Gegenstaende oder eine zweite Ablehnung brechen die Wiederholung ab.
Inventare werden ausschliesslich nach Serverbestaetigung aktualisiert.

Die Regressionen verwenden den registrierten nativen HTML-Klickhandler fuer
Button, Bild und Stapelzahl. Bestaetigte Mengen werden mit der echten
Inventartransaktion berechnet. Autoritaetstests pruefen Kohle auf dem Cursor,
Ablegen im Motor, Ablehnung veralteter Wiederholungen und die atomare
Zurueckweisung eines Nichtbrennstoff-Tauschs.

Der UDP-Fall startet mit leerem Motor, legt echte Kohle mit einer einzelnen
QuickMove-Geste ein und prueft Fahrt, Brennstoffentnahme, eine weiterhin offene
private Motoransicht und den erhaltenen Restvorrat beim Rueckbau. Ein fremder
Client bekommt keine private Ladung. Ein weiterer Client-Test prueft die
Invalidierung bei Besitzerwechsel, entferntem Fahrzeug/Teil und unpassendem
Teiltyp.

Alle Pruefungen laufen ohne Grafikbackend oder Spielfenster. Cargo verwendet
einen Job mit niedriger CPU-/I/O-Prioritaet; DISPLAY und WAYLAND_DISPLAY sind
entfernt. Die vollstaendigen Kommandos und Ergebnisse stehen in verification.json
und den zugehoerigen Logs. Protokoll 119 und Zugindex ID15 bleiben unveraendert.

Die Workspace-Libraries bestehen mit **4.228 bestandenen Tests**, keinem Fehler
und neun bestehenden ignorierten Tests. Die neuen Faelle sind darin enthalten.
Der vollstaendige Workspace-Build mit Client-/Server-Linking besteht in
4 min 09 s. Clippy aller Workspace-Targets mit `-D warnings`, Format- und
Diff-Pruefung bestehen ebenfalls.
