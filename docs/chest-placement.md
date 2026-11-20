# Truhen nebeneinander platzieren

Jede physische Truhe besitzt weiterhin ihr eigenes Inventar mit 18 Fächern. Eine Doppeltruhe verbindet genau zwei Hälften. Das Verbinden verschiebt oder kopiert keine Gegenstände und verändert weder die Holzart noch das Inventar der bestehenden Hälfte.

- Ohne Shift wird mit genau einer freien seitlichen Nachbartruhe verbunden. Ein vorhandenes Doppel ist dabei ein belegter Nachbar und blockiert die neue Truhe nicht. So können mehrere Doppeltruhen direkt nebeneinander entstehen.
- Sind mehrere freie seitliche Nachbarn möglich, bleibt eine automatisch platzierte Truhe einzeln. Eine Dreiergruppe oder überlappende Paare entstehen nicht.
- Mit linker oder rechter Shift-Taste auf Boden, einen anderen Block oder die Oberseite einer Truhe platzieren: Die neue Truhe bleibt einzeln.
- Mit Shift direkt eine seitliche Truhenfläche anklicken: Nur die tatsächlich angeklickte Truhe ist Verbindungskandidat. Sind beide frei und liegt die neue Truhe links oder rechts der bestehenden Ausrichtung, entsteht ein Doppel. Die neue Hälfte übernimmt die Ausrichtung der bestehenden Truhe. Vorder-/Rückseite und bereits verbundene Truhen bleiben ohne neue Verbindung; es wird kein anderer Nachbar gewählt.

Client-Vorschau und Server verwenden denselben Platzierungsplan. Der Server berechnet den tatsächlichen Treffer erneut, prüft Inventar- und Abschnittsrevisionen sowie Baurechte für jede betroffene Hälfte. Eine fremde Truhe wird beim Verbinden nicht ohne Erlaubnis verändert. Eine bewusst einzeln daneben gesetzte Truhe benötigt keine Änderung an diesem Nachbarn.

`ChestMerge` ist reine Platzierungsabsicht im Netzwerkprotokoll 141. Gespeicherte Inventare und die bestehende Metadatenstruktur für Doppeltruhen bleiben unverändert. Automatisierte Tests decken echte Shift-Eingaben, freie Nachbarpaare neben vorhandenen Doppeln, verschiedene Ausrichtungen, Rechte und abgelehnte atomare Transaktionen sowie echte Speicher- und UDP-Replikationspfade ab.
