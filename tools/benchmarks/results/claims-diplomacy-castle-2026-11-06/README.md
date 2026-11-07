# Claims, Diplomatie und Burgkorrekturen (195)

Der nach dem IDE-Absturz vorhandene Entwurf fuer F55/F56 wurde fertiggestellt.

Spieler und autorisierte Bots beanspruchen horizontale 16-x-16-Chunks fuer
ihr Kingdom. Besitzer-Kingdom und ausfuehrender Spieler/Bot werden getrennt
gespeichert. Land muss ueber Seiten verbunden bleiben; Freigaben duerfen
keine Inseln abtrennen. Der Bergfried-Chunk bleibt geschuetzt. Konkurrierende
Ansprueche und unberechtigte Akteure werden vom Server abgelehnt.

Geschuetztes Bauland und offenes Territorium besitzen getrennte Limits.
Level 1 erlaubt 32 geschuetzte und 64 offene Claims; weitere Level erweitern
die Kontingente. Spieleranfragen reichen vier Chunks je Achse weit. Ein Bot
muss leben, dem anfragenden Mitglied gehoeren und innerhalb 64 Metern im
Zielchunk stehen. Bot-Anfragen verwenden ClaimBotChunk/ReleaseBotChunk.
Automatische NPC-Arbeitsplanung bleibt ein getrenntes Thema.

Claims ueberleben Unload, Reconnect und Save/Reload. IDX9 importiert IDX8
ohne bestehende, eventuell geteilte Bergfried-Chunks stillschweigend
zuzuweisen. Protokoll 103 repliziert die neuen Daten. Client und Server
wurden gemeinsam gebaut.

## Bedienung

Im Bergfried-Menue laesst sich der aktuelle Chunk schuetzen, als offenes
Territorium markieren oder freigeben. Auf der grossen Karte oeffnet das
Schwerter-Symbol **Diplomatie**. Ein Kartenklick waehlt den Chunk; die
Seitenleiste zeigt Kingdom, Gruender, Koordinaten, Anspruchstyp, Beziehung
sowie Spieler-/Bot-Akteur. Die dortigen Aktionen laufen ueber den Server.

Farbige Flaechen, Grenzen und Legenden folgen Live-Aenderungen. Die Anzeige
benutzt bestehende Karten-/Hoehendaten und fordert keine Welt-Chunks an.
Eigene und neutrale Reiche sind unterscheidbar; weitere Beziehungen und
Rollen verbleiben unter F57. Klicks auf die Seitenleiste waehlen nicht mehr
versehentlich die Karte darunter. Die Werkzeugleiste passt auch in kleine
Fenster; Legendennamen und Zusatztexte stehen untereinander.

## Bildkorrekturen

Die [Mauerkrone](references/crown.png) besitzt jetzt auch an den Koepfen
ausgearbeitetes Mauerwerk; ihre Kanten passen zum Unterbau. Die
[markierten Wandflaechen](references/walls.png) verwenden die gemeinsame
Steinpalette, feine verzweigte Risse und unregelmaessigen plastischen Bewuchs.
Dekorvarianten werden stabil aus Weltkoordinaten bestimmt. Die Bogenunterseite
ist durchgehend gekruemmt, hat kontinuierliche UVs und passende Tangenten;
zusammenpassende Haelften behalten keine sichtbare innere Schnittflaeche.
Formen, Kollision und Picking werden gemeinsam geprueft.

## Nachweise

Siehe `verification.json` fuer Testzahlen, Quell-/Bildpruefsummen und
Aufloesungen. Library-Tests, Workspace-Build, Clippy aller Targets mit
`-D warnings`, CSS-Parser, Format und Diff sind erfolgreich. Die Tests
enthalten echte UDP-Autoritaet mit Spieler-/Bot-Claims, spaeten Beitritt,
Besitz-/Inventarpruefungen, bezahlte Upgrades sowie IDX8/IDX9-Speicherpruefungen.
Ein UI-Test prueft echte Kartenaktionen, unveraenderte Replikate bis zur
Serverantwort und Klickabschirmung der Seitenleiste.

Ergebnis: **3647 Library-Tests bestanden, keine fehlgeschlagen, acht ignoriert**.

Zehn native Aufnahmen: sieben Modellansichten mit realen GPU-Previews,
Picking/Kollision und Live-Wappenwechsel; drei Diplomatieansichten mit
Live-Farbwechsel und kleiner Fenstergroesse. Die UI-Galerie nutzt ausdruecklich
eine temporaere Client-Projektion; sie ist kein Nachweis einer Serveraktion.
Diese wird getrennt ueber UDP getestet. Keine OS-Maus-/Tastatureingaben,
keine Aenderung vorhandener Spielstaende oder normaler Benutzereinstellungen.

- [Diplomatie](native/claims/diplomacy-claims.png),
  [kleines Fenster](native/claims/diplomacy-compact.png).
- [Mauern und Bewuchs](native/castle/castle-natural-walls.png),
  [Boegen](native/castle/castle-openings.png).
- [Tests](logs/library-tests.log), [Build](logs/build.log), [Clippy](logs/clippy.log).

Die SVG-Symbole fuer Aehre und Hammer sind eigene Vektorassets im bestehenden
UI-System. Die Burgdetails sind reproduzierbare Mesh-Geometrie aus
`tools/assets/build_castle.py` und `castle_details.py`.
