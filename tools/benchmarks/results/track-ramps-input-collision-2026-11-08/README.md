# Rampen, R-Auswahl und Kollision nach Blockänderungen

Implementierungsbericht vom 08.11.2026 zum Folgefeedback nach
[Kontextgleisen](../contextual-track-building-2026-11-08/README.md).
Die zentrale CPU-Abnahme umfasst 4.251 bestandene Tests, keinen Fehler und
neun bestehende ignorierte Tests. Keine Grafiktests, Spielclients, GPU-Tests
oder Screenshots wurden gestartet.

## Rampen am Gelände

Die vorherige Vorschau erkannte beim Blick auf ein unteres Gleisende nur bereits
vorhandene obere Gleisports. Ohne oberes Gleis wurde die angrenzende Stufe nicht
als Rampenziel entdeckt. Ein kurzer Abstand zur Stufe wurde außerdem als eigene,
zu kurze Steigung geplant, obwohl der vorhandene gerade Abschnitt ausreichend
Platz für den Übergang bereitstellt.

Der Kontextplaner untersucht jetzt höchstens fünf tatsächliche Zellen vor einem
anvisierten freien Ende. Eine echte um einen Block erhöhte Plattform kann so auch
ohne oberes Gleis erkannt werden. Boden-/Wandtreffer beziehen sich auf die erste
passende Kante des Korridors. Das letzte gerade Viermeterstück kann gemeinsam
mit bis zu vier Metern Fortsetzung in eine sanfte Rampe übergehen. Der hintere
Anschluss bleibt exakt erhalten. Geprüfte Landehöhe, Freiraum, Unterbau, Holzart,
vollständiger Besitz und alle bisherigen Rechte bleiben Voraussetzung.

## Kurven mit R und lesbare Bauhinweise

Das Track-Item verwendet die Modi **Auto**, **Kurve** und **Prellbock**.
Die konfigurierte Benutzen-Taste, standardmäßig **R**, wechselt die Form vor
der aktuellen Kontextplanung. An passenden freien Enden kann eine reale Kurve
direkt gewählt und gebaut werden. Ohne direkte Verbindung bleibt im Kurvenmodus
die Start-/Zielplanung verfügbar. Das Mausrad übernimmt weiterhin nur echte
Andockalternativen; Hotbaränderungen bleiben außerhalb dieses Kontexts normal.

Neue Kreuzungen sind weder mit dem Track-Item noch mit altem Kreuzungsbestand
platzierbar. Material 204 und seine gespeicherte Geometrie bleiben für vorhandene
Welten lesbar und abbaubar. Bestehende IDs, Rezeptbestände und Saveformate ändern
sich dadurch nicht. Abzweigungen verwenden eigenständige Weichen.

Der rahmenlose Hinweis enthält eine kurze Formüberschrift, getrennten
Materialbedarf, eine kompakte Tastenzeile und eine kurze Statuszeile.
Lange Planerdiagnosen werden verständlich gekürzt. Maus-/Shift-Belegungen bleiben
korrekt und erscheinen als kurze Beschriftungen. Ein schwarzer Textschatten mit
zwei Pixeln Abstand verbessert den Kontrast auf hellem Terrain.

## Bewegung nach Blockänderungen

Der bisherige Kollisionspfad wartete auf den fertigen sichtbaren Terrain-Mesh,
bevor ein separater Kollisionsjob starten durfte. Während dieser Kette wurde
die Bewegung im betroffenen Raum sicherheitshalber angehalten.

Bestätigte Blockdaten versorgen den Kollisionsaufbau direkt, während
das sichtbare Mesh asynchron aktualisiert wird. Kollision und Bewegungsfreigabe
müssen weiter zur bestätigten Welt passen. Unbekannte Generatorwerte bei
entfernten Overrides sowie überholte Workerergebnisse dürfen keine veraltete
Kollision freigeben. Die CPU-Regressionen prüfen diese Fälle ohne Renderer.
Ein benötigter aktueller Kollisionsjob bleibt notwendig; eine gemessene
FPS-Verbesserung oder ein Grafiktest wird hier nicht behauptet.

Ein weiterer Fehler fasste sekundäre Gleiszellen unter dem gemeinsamen Anker
zusammen, obwohl jede Zelle bereits ihre geclippten Kollisionsflächen liefert.
Jetzt fließt jede belegte Zelle in die Kollision ein. Gerade und ansteigende
Gleise werden über negative Sectiongrenzen sowie bei entladenem Anker gegen
ihre vollständigen kanonischen Flächen geprüft.

## Zentrale Abnahme

Alle zwölf Workspace-Library-/Bin-Suiten bestehen. Die Rampenmatrix prüft
60 tatsächliche Blickstrahlen in vier Ausrichtungen: auf das untere Gleis,
die Stufenwand und die Plattformoberseite, jeweils mit 0–4 Metern Abstand und
ohne oberes Gleis. Die Player-Prüfung verwendet das echte Targeting und sendet
genau einen signierten Baubefehl; die Authority prüft den Bau mit tatsächlichem
Spielerkörper, Materialbedarf und unverändertem Gelände.

R-/Mausradpriorität, freie Kurvenplanung, zurückgesetzter Itemwechsel sowie
verbotene neue und abbaubare vorhandene Kreuzungen sind abgesichert. Die
Kollisionsregressionen halten den sichtbaren Mesh ausdrücklich auf seiner alten
Version, prüfen tatsächliches Platzieren/Abbauen und verwerfen veraltete
Worker-Daten. Vergessene Overrides, unbekannte Nachbar-/Profilwerte und ungültige
oder veraltete Replikate bleiben abgesichert.

Clippy aller Targets mit `-D warnings`, Formatierung, Diff und kopfloser
Assetexport sind sauber. Der vollständige Workspace-Build erstellt aktuelle
Client-/Serverdateien. Ergebnisse und Quell-/Logprüfsummen sind in
`verification.json` festgehalten.
