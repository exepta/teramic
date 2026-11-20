# Geistwald, Grabstätten und Modellpflege

Neue Welten verwenden `spirit_blossom_revision = 2`. Gespeicherte Revisionen 0
und 1 werden beim Öffnen beibehalten. Auch unerkundete Teile einer alten Welt
verwenden weiter deren gespeicherte Revision, damit an Chunkgrenzen keine
Versionssprünge entstehen. Für das neue Gelände eine neue Welt anlegen;
Modelle, Wasserfarben und Nebel werden auch in bestehenden Welten aktualisiert.
Client und Server verwenden Protokoll 143.

## Gelände und Atmosphäre

Das trockene Innere des Geistwalds erhält 20 % mehr Höhenvariation. Zu Ufern,
Meer und Nachbarbiomen wird die Veränderung weich ausgeblendet. Die vorhandenen
Flussverbindungen, Wasserstände und Seeausläufe bleiben erhalten. Breitere
Flüsse und größere Seeflächen ergeben in der reproduzierbaren Stichprobe über
18 starke Geistwaldregionen und drei Seeds 39,00 % mehr tatsächlich nasse
Terrainspalten (5.566 → 7.737 bei 377.729 geprüften Positionen). Einzelne Regionen
können stärker oder schwächer reagieren; eine prozentgenaue Garantie für jeden
Seed gibt es nicht. Die trockene innere Höhenvariation steigt in derselben
Messung genau um 20 %; die zusätzliche Ufermodellierung beeinflusst die
Gesamtsteigung ebenfalls.

```sh
cargo run -p terra-world --example measure_spirit_wetlands
```

Geistwaldwasser ist türkis mit hellen, weißlichen Licht- und Schaumanteilen.
Kirschwaldwasser und die weich überblendete Ferne/Umgebungsfarbe sind rosa.
Normale Nachbarbiome bekommen keinen Geisterwasser-Leuchteffekt.

Der Geistwaldnebel verwendet ein einziges terrainfolgendes Volumen mit sechs
Strahlproben und einem 11 × 11 Texelfeld (484 Byte). Höchstens 121 bereits
verfügbare Bodenspalten werden alle 0,5 Sekunden gelesen. Dafür werden keine
Chunks erzeugt. Wind und Dichte bewegen sich weiter im Shader; Laternen
bekommen keinen eigenen Nebel. Die vorhandenen Lichtbudgets gelten auch für
Grabstätten: keine zusätzlichen Schatten oder volumetrischen Lichtquellen.
`TERRA_SPIRIT_MIST=off` deaktiviert nur den Biomnebel für Vergleichsmessungen.

## Friedhöfe und Grabgewölbe

Die Strukturen erscheinen ausschließlich in ausreichend starken, trockenen
Geistwaldregionen ab Revision 2. Sie nutzen die bestehenden Struktur-, Cache-
und Chunkbudgets sowie die Option `structures.ruins`. Die Standortprüfung
prüft Gelände und Biomgewichte über die Grundfläche, schließt erkannte
Gewässer und steile Hänge aus und prüft den Anschluss des Zugangs an den
natürlichen Boden.

- Friedhöfe: Einfassungen, Wege, Grabsteine, Sarkophage, Laternen und Bewuchs.
- Rittergrab: drei unterirdische Ebenen.
- Priesterinnengrab und Gelehrtengrab: jeweils vier unterirdische Ebenen.

Treppen verbinden die Ebenen mit begehbaren Gängen. Namen und Varianten werden
aus der Standortidentität abgeleitet. Truhen enthalten echte, gespeicherte und
abbaubare Vorräte. Die Strukturen bestehen aus normalen Weltblöcken und sind
keine darübergelegten Renderattrappen.

```text
/locate struct teramic::cemetery
/locate struct teramic::grave_dungeon
```

Die Grab-Suche zeigt den Namen, beispielsweise `Tomb of Corvin`. Die drei
Grundrisse und die Suchnamen sind implementiert; spezielle Bosskämpfe,
Questketten und ein Schaltkreis-/Fallenrätselsystem sind nicht enthalten.

## Neue Blöcke

IDs **324–399** ergänzen 76 Einträge mit einzelnen JSON-Dateien, GLB-Modellen
und PNG-Texturen: farbiger Lehm, Kryptenmauerwerk, Platten, Säulen, Bögen,
Reliefs, Banner, Gräber, Sarkophage, Lichter, Knochen, Truhen, Bewuchs,
Kristalle, Runen und Mechanik-Dekorationen. Creative-Inventar und Building-App
lesen dieselben Definitionen. 39 Rezepte für Lehm, Mauerwerk, Beleuchtung und
Grabtruhen liegen in `assets/crafting/biomes/graves/`.

Die verzierte Grabtruhe (377) besitzt ein echtes 18-Slot-Inventar, unterstützt
Arbeiterlager und bleibt eine einzelne Truhe. Abbau gibt ihren Inhalt zurück.
Neue Props speichern ihre Ausrichtung; Auswahl und Kollision verwenden die
jeweils im JSON angegebenen Formen.

Druckplatte, Hebel, Schalter, Gittertor, verschlossenes Tor, Stachel-/Pfeilfalle
und Nebelblock sind derzeit platzierbare Dekorationen. Sie lösen noch keine
Schaltkreise oder Schaden aus, öffnen nicht automatisch und erzeugen keine
zusätzlichen Nebelvolumen. Die aktiven Runen und Lichtdekorationen haben ihre
angegebene Licht-/Emissionswirkung.

```sh
python3 tools/assets/build_cemetery.py
python3 tools/assets/cemetery/recipes.py
```

## Modellpflege und Nachweise

Die Modellpflege umfasst den vollständigen ausgelieferten GLTF/GLB-Katalog
und die gemeinsamen Laufzeitgeneratoren für Spieler, Bewohner und Kreaturen.
22 Möbel- und Propmodelle erhalten kleine Fasen; runde Kreaturenkörper, Blüten
und Wasserpflanzen erhalten feinere Konturen. Vorhandene Figuren- und
Rüstungsdetails sowie mechanische Kontaktflächen behalten ihre genaue Form. Vollflächige Terrainanschlüsse
bleiben bündig, damit keine neuen Sichtfugen entstehen. Bereits detaillierte
Kurven werden nicht lediglich durch zusätzliche Dreiecke aufgebläht.

Die sechs Blütenmodelle mit Ellipsoid-Blütenblättern verwenden gemeinsam genutzte
Vertices und analytische, glatte Normalen. Ihre 13.340 Dreiecke, Konturen,
Materialzuordnungen und Kollisionsformen bleiben erhalten; die Vertexzahl sinkt
von 29.868 auf 9.996. Die Blütenbeleuchtung wird dadurch weicher. Dieses Verfahren
gilt ausschließlich für untexturierte Blütenmaterialien; vorhandene Textur-UVs
und deren Tangenten werden nicht umgeschrieben. JSON-Dateien und PNG-Texturen
bleiben unverändert.

Der Dreiecksvergleich im Modell-Audit (309.913 → 320.632) beschreibt nur den
nachgelagerten Fasen-Durchlauf der 22 Möbel-/Propmodelle. Neue Grabmodelle und
überarbeitete Blüten sind darin schon vor diesem Durchlauf enthalten; dieser
Wert ist daher kein Vergleich des gesamten Assetbestands mit der früheren
nativen Performance-Messung. Die gesonderte Blütenprüfung dokumentiert identische
Float32-Dreieckspositionen sowie gültige Indizes, Normalen und Tangenten.

Prüfprotokolle, Vorher-/Nachhermessung und native Aufnahmen liegen unter
`tools/benchmarks/results/spirit-world-polish-2026-11-13/`.
