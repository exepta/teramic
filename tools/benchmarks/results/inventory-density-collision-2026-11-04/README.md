# Inventaraufteilung, 4K-Dichte und Kollisions-Invalidierung (191)

HUD-Hotbar um 30 Prozent verkleinert. Inventarzellen wachsen um 20 Prozent
auf 33.6 logische Pixel, Ergebnisse entsprechend auf 38.4. Player-/Crafting-
Bereiche sind statt 188 jetzt 150.4 Pixel hoch. Rahmen und Katalogbreiten
passen zu den breiteren Gittern; Ausruestung bleibt vollstaendig erreichbar.

Text und Bedienicons bekommen oberhalb Full HD eine gemeinsame Dichtekorrektur:
HD/FHD 100 Prozent, 1440p etwa 87 Prozent, 4K etwa 71 Prozent der bisherigen
Groesse. Diese Korrektur ist von OS-DPI unabhaengig. Eingabefelder werden mit
Text, Platzhalter, Cursor und Auswahl als Einheit transformiert; ihre linke
Kante bleibt verankert. Das verhindert unterschiedliche Koordinatensysteme.

Wuerfelkollisionen ohne Modellabhaengigkeiten werden bei reinen
Nachbar-/Renderinvalidierungen weiterverwendet. Eigene Blockrevisionen,
geaenderte Voxeldaten, unbekannte zukuenftige Meshstaende und angrenzende
Modellformen verhindern die Wiederverwendung. Ein begrenzter Halo-Test
beruecksichtigt bereits im Journal bestaetigte, noch nicht remeshte Modelle.
Der Wiederverwendungsstand wird anschliessend aktualisiert, sodass diese
Halo-Pruefung nicht dauerhaft in jedem Bewegungsframe laeuft.

Regressionen pruefen, dass normale Nachbarbloecke weder die Bereitschaft
aufheben noch zusaetzliche Colliderjobs erzeugen; Modelle und lokale Edits
invalidieren weiterhin. Der bestehende UDP-Test fuer wartende Spruenge bleibt
erhalten. Die konkrete vom Nutzer beobachtete Teleportbewegung wurde noch
nicht reproduziert; Blockart, Modus und Reproduktionsschritte sind angefragt.

3554 Library-Tests bestanden, keine fehlgeschlagen. Workspace-Build, Clippy
aller Targets mit `-D warnings`, Format, Diff und CSS-Parser geprueft.
Native Ergebnisse und Abmessungen stehen in `verification.json` und `logs/`.
Alle 26 nativen Ansichten bei 3840 x 2160 und 1025 x 641 bestanden.
Bei den Slotmassen werden ganze Layoutpixel vor dem lokalen Zoom verwendet;
die logische Messpruefung beruecksichtigt die Rundung eines Layoutpixels.
Die opt-in Galerie verwendet temporaere Client-Projektionen und eine eigene
Konfiguration; keine OS-Eingaben oder Aenderungen vorhandener Spielstaende.

- [Spielerinventar 4K](native/fixed-player-wide.png)
- [Spielerinventar kleines Fenster](native/fixed-player-compact.png)
- [Werkbank und Truhe 4K](native/surface-workbench-linked-wide.png)
- [Protokoll](logs/native.log)
