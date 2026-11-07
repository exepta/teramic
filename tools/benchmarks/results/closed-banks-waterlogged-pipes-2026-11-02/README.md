# Geschlossene Wasserraender und wassergefuellte Rohre

Referenzen: `references/slabs.png` und `references/fence.png` sind die beiden
vom Nutzer gemeldeten Becken mit ungewolltem seitlichem Wasseraustritt.

Konstruktionsraender nehmen Wasser in ihren freien Zwischenraeumen auf,
geben es aber seitlich nicht in den Aussenraum ab. Geschlossene reine
Konstruktionsraender begrenzen Quellwasser auf die niedrigste feste Oberkante.
Der Mengenabgleich beruecksichtigt auch das Absenken bereits voller Quellen.

Beim echten Rohr-Commit wird implizites Naturwasser explizit erhalten und
an Replikas uebertragen. Nasse Rohre erhalten keine feste Terrainumfassung,
die den freien Wasserraum verdecken wuerde. Trockene Durchfuehrungen behalten
ihre runde Umfassung.

Die native Partial-Water-Pruefung verwendet frei stehende Slab- und Zaunringe,
prueft die trockenen Aussenseiten und zeigt alle vier Rohrmaterialien in
wassergefuellten Steinshaechten. Die Schachtrohre werden ueber den normalen
Editpfad gesetzt, ohne anschliessend Wasser in diese Zellen zu schreiben.

Automatische Tests: **3716 bestanden, 0 fehlgeschlagen, 8 ignoriert** in
12 Testprogrammen. Darunter sind die Mengenbuchhaltung beim Absenken voller
Slab-Quellen und der Wassererhalt beim Commit samt Replica-Import.

Build, Clippy (`-D warnings`), Format- und Diffpruefung bestehen. Beide nativen
Spielpruefungen bestehen: Partial-Water mit sechs Aufnahmen in 12,83 s und
Hydraulik mit 35 Pflichtaufnahmen in 74,75 s. Zusaetzliche Diagnoseaufnahmen
bleiben als solche erhalten. Unterwasserbau, Quellerhalt beim Rohrsetzen,
kontinuierlicher Glasrohrfluss und echte Infuserbefuellung sind bestaetigt.

Die erste Spielpruefung deckte einen Unsigned-Unterlauf beim Absenken mehrerer
voller Slabquellen auf. Die Mengenbuchhaltung ist korrigiert und eine eigene
Regression prueft die exakte Bilanz. Das urspruengliche Fehlerlog bleibt unter
`logs/initial-partial-accounting-failure.log` erhalten; die abschliessenden
Laeufe enthalten keinen solchen Fehler. Beim Hydraulik-Szenenwechsel erschien
einmal die bereits bekannte Bevy-Warnung zu einem RenderLayers-Eintrag an
einer entfernten Entity; keine Assertion schlug fehl.

Ergebnisse stehen in `verification.json` und `logs/`, Quellhashes in
`source-sha256.json` und die verwendeten Spielparameter in `config/`.
