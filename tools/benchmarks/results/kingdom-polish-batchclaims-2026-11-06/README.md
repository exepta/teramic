# Kingdom-UI, Flächenauswahl, Fenster und Wappen-Capes (196)

Das Kingdom-Menü verwendet eine transparente äußere Rahmenfläche, kompakte
Typografie, umbrochene Buttontexte und den kupferfarbenen Colorpicker samt
Popup. Ein eigener Button öffnet die vollständige, durchsuchbare Symbolauswahl.
Katalogsymbole besitzen stabile Namens-Hashes; die acht bisherigen Symbole
behalten ihre Speicherkennungen. Banner und Capes verwenden denselben Glyphensatz.

## Bedienung der Karte

In **Diplomatie** zuerst **Build Chunk** oder **Territorium Chunk** wählen,
anschließend eine oder mehrere rechteckige Flächen mit der linken Maustaste
ziehen. Grün markiert geschütztes Bauland, Blau offenes Territorium. Die Auswahl
ist zunächst eine Vorschau. **Apply** bestätigt alle ausgewählten Chunks gemeinsam;
**Cancel** verwirft sie. **Pan** wechselt zum Verschieben der Karte.
Einzelne bestehende Claims können weiterhin freigegeben werden.

Der Server prüft die gesamte Auswahl, einschließlich Reichweite, Mitgliedschaft,
fremdem Besitz, Verbindung und getrennten Kontingenten, vor der ersten Änderung.
Ein Fehler hinterlässt keine teilweise angewandte Fläche. Maximal 256 ausgewählte
Chunks pro Anfrage; vorhandene Reichweiten und Kontingente gelten weiterhin.
Unveränderte Claims behalten ihren ursprünglichen Spieler-/Bot-Akteur. Die
Relief-Auswahl berücksichtigt die bereits bekannten Kartenhöhen und erfasst auch
die Mausposition im Loslass-Frame. Es werden keine Welt-Chunks nachgeladen.
Client und Server verwenden gemeinsam Protokoll **104**.

## Fenster und Platzierung

Mauerfenster verwenden das transparente Glas der normalen Fenster: vier Scheiben
um das Holzkreuz, mit passenden dünnen Kollisions-/Pickingflächen. Burgteile
bleiben beim Ansetzen an Seitenflächen aufrecht; beim Verlängern einer Mauer
übernehmen Fenster, Bögen und Zinnen die vorhandene Ebene. Deckenmontage und die
24 Ausrichtungen per Werkzeug bleiben möglich.

## Capes und Bot-Kingdoms

Neu über TMI oder Crafting erworbene ungefärbte Capes übernehmen Farbe und Wappen
des eigenen Kingdoms. Die konkrete Identität bleibt beim Ablegen, Weitergeben,
Ausrüsten und Speichern erhalten. Manuelles Färben setzt eine eigene Farbe.
Inventarvorschau, Recipe UI, Drops und animierte Spieler-Capes berücksichtigen
diese Daten. Bestehende Item-Varianten bleiben unverändert; IDX9/PLY7 bleiben lesbar.
Seltene Containerantworten sind wie andere große Kontrollnachrichten intern
geboxt; das verändert ihre serialisierte Form nicht und hält Kontrollnachrichten im Arbeitsspeicher klein.

| Bot-Kingdom | Beziehung |
| --- | --- |
| Lutera | Neutral |
| Betross | Neutral |
| Helteria | Freundlich |
| Petora | Feindlich |

Dörfer erhalten aus ihrer bestehenden stabilen Identität eine feste zufällige
Zugehörigkeit; ein Fünftel bleibt unabhängig. Die Zuordnung bleibt nach Reload
und Streaming gleich, ohne bestehende Bauten neu zu generieren. Betreten eines
zugeordneten Dorfs zeigt den Namen und die Beziehung. Die Karte zeigt die vier
Bot-Reiche sowie die Zugehörigkeit bereits geladener Dörfer. Spieler-Claims haben
Vorrang. Frei verwaltbare Beziehungen und Mitgliedsrechte bleiben Teil von F57.

## Noch fehlende Vorlagen

Die in der Nachricht erwähnten Bilder für **Kingdom 1**, **Kingdom 2** und
**Outpost** waren nicht angehängt. Diese drei Strukturvorlagen wurden daher nicht
erfunden oder als fertig gemeldet; sie sind mit dem fehlenden Bezug unter **F72**
in der TODO festgehalten.

## Prüfung

Die abschließenden Ergebnisse, Quellprüfsummen und nativen Aufnahmen stehen in
`verification.json`, `logs/` und `native/`. Die Kingdom-UI-Galerie verwendet einen
isolierten Spielstand und echte Gründungs-/Änderungsantworten des Servers. Die
Karten-Galerie verwendet eine temporäre Clientprojektion; die Autorität wird
getrennt mit echtem UDP geprüft. Keine OS-Eingaben oder normalen Spielstände.

Ergebnis: **3.674 Library-Tests bestanden, keine fehlgeschlagen, acht ignoriert**.
Workspace-Build, Clippy aller Targets mit `-D warnings`, Format, Diff und die
Parserprüfung aller 60 UI-Stylesheets sind erfolgreich. Echte UDP-Tests decken
atomare Rechteck-Claims und alle TMI-/Crafting-Erwerbswege der Wappen-Capes ab;
Speichertests prüfen Übergabe, Equipment und Kingdom-Wechsel. Der native SVG-
Ladetest stellt sichtbare Aehren-/Hammersymbole sicher.

**24 native Aufnahmen**: elf Burgansichten, vier Kartenansichten und neun
Kingdom-Aufnahmen einschließlich des ungebundenen Menüs. Die acht gezielten
Menüansichten prüfen QHD, 4K und 1024 × 768, vollständige Symbolauswahl, Suche,
das tatsächliche dunkle Colorpicker-Popup sowie Textgrenzen in Buttons.
Bildaufnahme und Zustandswechsel liegen in getrennten Frames; der nächste
Schritt wartet auf die abgeschlossene Screenshot-Rückgabe.

- [Kingdom-Menü](native/kingdom/kingdom-ui-qhd.png),
  [Colorpicker](native/kingdom/kingdom-ui-colorpicker.png),
  [4K](native/kingdom/kingdom-ui-4k.png),
  [kompaktes Fenster](native/kingdom/kingdom-ui-compact.png).
- [Symbolauswahl](native/kingdom/kingdom-ui-icons-qhd.png),
  [Suche](native/kingdom/kingdom-ui-icons-search.png).
- [Rechteck-Vorschau](native/claims/diplomacy-rectangle-preview.png),
  [Fenstervergleich](native/castle/castle-window-comparison.png),
  [Platzierungsseiten](native/castle/castle-placement-six-faces-front.png).
- [Tests](logs/library-tests.log), [Build](logs/build.log), [Clippy](logs/clippy.log).
