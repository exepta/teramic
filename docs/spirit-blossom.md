# Spirit Blossom und bewegter Gebietsnebel

F35 verwendet das Spirit-Blossom-Motiv der Referenzbilder: rosa, weiße und
lavendelfarbene Blütenkronen, gewundene Stämme, türkisfarbene Grasflächen über violettem
Gestein, leuchtende Blumen, Laternen und ruhige Gewässer. Die Assets sind eigene,
prozedural erstellte Spielmodelle; die Referenzbilder werden nicht als Texturen
oder fertige 3D-Modelle ausgegeben.

## Im Spiel

Bei der Welterstellung **Spirit Blossom / Geisterblütenhain** als Startbiom
wählen. Alternativ findet `/locate biome spirit_blossom` das Biom in einer
dafür erzeugten Welt. Minimap, Debuganzeige und Kartenfarben kennen den neuen
Eintrag. Der Biomname und alle neuen Gegenstände sind deutsch/englisch lokalisiert.

Die Weltoption `spirit_blossom_revision = 2` aktiviert das neue Biom samt
Grabstätten und erweitertem Gelände in frischen Welten. Revision 1 behält den
ursprünglichen Hain ohne diese Geländeerweiterungen. Alte TOML-Weltmanifeste ohne dieses Feld lesen weiterhin Revision 0;
ihre Gelände- und Baumgenerierung bleibt erhalten. Alte gespeicherte achtteilige
Biomdefinitionen bekommen beim Laden lediglich den neuen Definitionseintrag.
Der Revisionsschalter verhindert, dass er die gespeicherte Welt verändert.
Zum Erkunden des neuen Bioms eine neue Welt anlegen; die Revision eines
bereits bebauten Spielstands nicht nachträglich umstellen.

## Blöcke und Dekorationen

Die stabil angehängten Material-IDs **306–323** umfassen:

| IDs | Inhalt |
| --- | --- |
| 306–309 | Geisterblütengras, Erde, Geisterstein, Blütenblattblock |
| 310–312 | Rosa, weißes und lavendelfarbenes Blütenlaub |
| 313–315 | Stamm, Bretter, leuchtender Laternenstein |
| 316–318 | Geisterblumen, Blütenbusch, Laternenpfosten |
| 319–323 | Lotus, Seerosenblatt, Hängelaterne, Blütenblattdecke, Schrein |

Jeder Eintrag besitzt eine eigene JSON-Definition und ein GLB-Modell unter
`assets/terra/blocks/`; die Texturen bleiben einzelne PNGs. Sechs Rezepte stehen
unter `assets/crafting/`. Der Creative-Katalog und die Building-App verwenden
dieselben Definitionen. `python3 tools/assets/build_spirit_blossom.py` erzeugt
die Assets deterministisch, ohne zusätzliche Python-Pakete.

Globales Noise mischt das Biom weich in seine Nachbarn. Gelände, Materialwahl,
Bäume und Dekorationen hängen von Seed und globalen Koordinaten ab. Flüsse und
Seen verwenden weiterhin die bestehende Gewässergenerierung. Das Wasser erhält
eine weich gemischte türkise Färbung mit weißlichen Lichtanteilen. Laternen und Geisterblumen nutzen
das vorhandene begrenzte Lichtsystem; Blütenlaub bewegt sich mit dem Wind.
Spirit-Gras trägt einen Türkiston, während das Gras im Kirschwald leicht lila
gefärbt ist. Die Biomfärbung gilt auch für Grashalme und die Kartendarstellung.

## Tiere

F37 ergänzt Blossom-, Petal-, Wisteria-, Spirit-, Goldenbloom-, Moonpetal- und
Sakurawing-Schmetterlinge. Sie fliegen, landen kurz und kommen auch in Wald,
Kirschwald und Ebenen vor. Spinnen können sie als Beute fressen.

F38 ergänzt zwei freundliche Geisterlichter, Blossom und Moon. Sie erscheinen
natürlich nur in Spirit Blossom. Lebende Geisterlichter geben lebenden Survival-Spielern im
Radius von **6 Metern** Regeneration I über das vorhandene Statussystem.
Die serverseitige Prüfung läuft einmal pro Sekunde; mehrere Auren stapeln sich
nicht, und stärkere Trankeffekte bleiben erhalten.

Der friedliche Geisterfuchs hat ein eigenes Modell mit drei leuchtenden,
bewegten Schweifen. Alle neuen Kreaturen nutzen vorhandene Spawn-, Simulations-,
Speicher- und Replikationswege. Ihre Typ-IDs 21–23 sind angehängt; vorhandene
Kreaturen behalten ihre IDs. Skelettkrieger aus F39 bleiben eine eigene Aufgabe.

## Nebel

Gespeicherte Arbeitsgebiete und rote Build-Grenzen verwenden niedrige,
transparente Nebelvolumen. Der Shader bewegt die räumliche Dichte durch eine
gemeinsame Noise-Textur und integriert acht Proben pro sichtbarem Fragment.
Weiche Ränder, Höhenverlauf und Tiefenprüfung verhindern sichtbare Boxflächen
und Übermalen verdeckter Geometrie. Es pulsiert nicht bloß eine Bodenfarbe.

Die Gebiete bleiben in fünf Material-/Mesh-Batches zusammengefasst. `H` blendet
sie im Baumodus aus und wieder ein, ohne die Assets neu anzulegen. Der zusätzliche
Spirit-Nebel verteilt sich über das geladene Gelände des Bioms und blendet zu
seinen Nachbarn aus. Seine Verteilung hängt von Biomgewichten und Bodenhöhe ab;
Laternen und Schreine erzeugen keine eigenen Nebelbänke. Die Lichtquellen behalten
ihre normale Beleuchtung. Entfernung und Anzahl der Nebelvolumen sind begrenzt;
entfernte Chunks werden dafür nicht nachgeneriert.

## Kompatibilität und Prüfung

Client und Server verwenden gemeinsam Protokoll **143**, weil die Weltoptionen
und die übertragenen Biome erweitert wurden. Die Minimap schreibt `TRMAP004`
mit neun Biomgewichten und liest weiterhin `TRMAP001` bis `TRMAP003`; alte
Erkundung und Wegpunkte bleiben erhalten.

Die nativen Abnahmen verwenden ausschließlich temporäre Konfigurationen,
Profile und Welten:

```sh
cargo build --locked --offline -p terra-client
bash tools/benchmarks/run-spirit-blossom.sh
bash tools/benchmarks/run-construction-mode.sh
```

Die Kreaturengalerie prüft tatsächliche Modelle und Animationen im Renderer;
sie ist kein KI-Nachweis. Dafür gibt es separate Rust-Tests für Spawns,
Bewegung, Kollisionen, Aura, Serialisierung und Materialien.

## Revision 2: Grabstätten, Gelände und Modellpflege

Neue Welten verwenden Revision 2 mit mehr Hügeln und Wasserflächen, Friedhöfen
und benannten Grabgewölben. Kirschwaldatmosphäre und Wasser sind rosa; der
Geistwaldnebel verwendet jetzt ein einzelnes budgetiertes Volumen. Details,
Blockumfang und Grenzen: [Geistwald-Erweiterung](spirit-world-polish.md).
