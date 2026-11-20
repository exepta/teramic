# Regionale Flora und Kakteen — Ökologie 6

Die regionale Flora wurde mit `ecology_revision = 6` eingeführt. Neue Welten
verwenden inzwischen Revision 7, die ausschließlich die Birkenform ergänzt;
[Birkenkronen und Tiefenbewegung](birch-aquatic-depth.md) beschreibt diese Änderung.
Gespeicherte Versionen 0–6 werden
beim Laden übernommen; ein fehlendes Feld bedeutet weiterhin Revision 0. Die
Revision verändert Bepflanzung und Baumkronen, keine Gelände-, Fluss- oder
Wasserhöhen. Bestehende Spielstände werden nicht nachträglich umgestaltet.

Die Pflanzenverteilung folgt den kontinuierlichen Wiesen-, Gehölz-, Lichtungs-
und Freiraumfeldern der vorhandenen Grasverteilung. Kleine Gruppen verwenden
eine Pflanzenart, versetzte Anker und unregelmäßige Ränder. Die vorhandenen
Biomegewichte wählen regionale Pflanzengruppen ohne harte rechteckige Grenzen.
Freiräume sind Vegetationslücken, keine zusätzlich gebauten Straßen.

| Gebiet | Bepflanzung |
| --- | --- |
| Ebenen | Margeriten, Lupinen, einzelne blühende Sträucher |
| Wald | Farne an Baumgruppen, Sträucher in den Lichtungen |
| Kirschwald | Blütensträucher, Margeriten und Lupinen |
| Hochland | Niedrige Heide auf Gras und Schnee; bestehende Nadelbäume |
| Wüste | Spärliche Trockensträucher und zusammenhängende Kakteen auf Sand |
| Spiritwald | Gruppierte Leuchtblumen, Blütensträucher und Blütenboden; seltene Laternen/Schreine |
| Blutwald | Bestehende gruppierte Blutflora mit neuen asymmetrischen Baumkronen |
| Küste | Blumen oberhalb des Wassers, Schilf an geeigneten Ufern; vorhandene Unterwasserflora bleibt erhalten |

Die bestehenden Oasenpalmen, Korallen, Wasserpflanzen und schwimmenden
Spirit-Seerosen werden weiterverwendet. Die bisherigen einzeln verteilten
Spirit-Landobjekte werden ab Revision 6 durch Gruppen ersetzt. Geplante Bauwerke,
Höhlenöffnungen, steile Hänge, tatsächliche Baumbelegung und Baumfüße bleiben frei.

## Kakteen

Material 422 ist ein normaler solider, deckender und abbaubarer Weltblock.
Kakteen sind 2–8 Blöcke hoch, niemals ein acht Meter hohes Einzelmodell. Größere
Pflanzen können ein oder zwei abgewinkelte Arme besitzen. Jedes Segment ist über
eine Blockfläche verbunden; die Arme bleiben unter der Spitze des Hauptstamms.
Stammanker halten mindestens neun Meter Abstand. Alle Höhen treten seedabhängig
auf. Sand, warmes Wüstenklima, trockener Boden, geringe Neigung und Abstand zu
Palmen/Bauwerken begrenzen ihre Standorte.

Die neuen kleinen Pflanzen 423–429 sind auswählbare, nicht solide Weltobjekte:
`meadow_daisies`, `lupine`, `forest_fern`, `flowering_shrub`, `alpine_heather`,
`desert_shrub`, `shore_reeds`. Sie belegen jeweils höchstens eine Zelle.

## Bäume und technische Einbindung

Eichen bekommen breite tiefe Astgabeln, Birken schmale hohe Kronen, Kirsch- und
Spiritbäume gestufte Fächer und Bloodwood hohe asymmetrische Kronen. Wurzeln
folgen weiterhin den geprüften lokalen Geländehöhen. Fichte, Tanne und Palme
nutzen ihre vorhandenen eigenständigen Formen. Ältere Baumkonstruktoren bleiben
für gespeicherte Ökologierevisionen unverändert.

Die Naturpatches bleiben 32 Meter groß und auf 128 Cacheeinträge begrenzt.
Headless-Sections, Client-Sections und direkte Materialabfragen verwenden
dieselben geplanten Zellen. Gespeicherte Spieleränderungen haben Vorrang.
Kakteen erhöhen auch die berechnete obere Grenze ihrer Spalte, damit ihre
Spitzen an vertikalen Sectiongrenzen geladen werden.

Für tatsächliche Standorte ohne Weltänderungen liefert das Beispiel JSON:

```bash
cargo run -p terra-world --example inspect_biome_flora -- desert
TERRA_FLORA_USE_CONFIG=1 cargo run -p terra-world --example inspect_biome_flora -- forest
```

Weitere Auswahlwerte: `plains`, `cherry`, `spirit`, `highland`. Der Suchbereich
ist begrenzt; wenn keine geeignete Stelle gefunden wird, meldet der Finder dies.
Die APIs `surface_cacti(tile)` und `surface_flora(tile)` verwenden globale
32-Meter-Tiles, einschließlich negativer Koordinaten. Die Cactus-Metadaten
enthalten Ursprung, Höhe, inklusive Grenzen und tatsächliche Voxelanzahl.

Regressionen prüfen Verbundenheit aller Kaktushöhen, negative Sectiongrenzen,
Habitate und Abstände, deterministische Pflanzengruppen, erhaltene Freiräume,
gemeinsame Client-/Server-/Editpfade sowie unveränderte historische Revisionen.
Die Ausführung wird mit der gemeinsamen Projektvalidierung dokumentiert;
dieses Dokument behauptet keine noch nicht gemessenen Leistungswerte.

## Native Aufnahmeprüfung

Nach dem gemeinsamen Client-Build startet `tools/benchmarks/run-reference-biomes.sh`
isolierte Prozesse für die acht Referenzbiome und vier Pflanzenansichten
(`reference-cactus`, `reference-plains`, `reference-desert`, `reference-spirit`).
Einzelne Szenen können als Argumente übergeben werden. Das Skript kompiliert
nichts und verwendet ausschließlich neue Testverzeichnisse unter
`target/reference-biomes/`; Spielerwelten werden nicht geöffnet.

Alle zwölf Szenen verwenden dieselben frischen Generatorrevisionen mit dem Seed
der Konfiguration und den aktuellen Biom-JSON-Profilen. Die Zeit ist für die
Aufnahmen auf 10 Uhr angehalten. Acht Panoramen und je zwei Pflanzenbilder
ergeben im vollständigen Lauf 16 Screenshots. Die PNGs und Prüfberichte landen
im ausgegebenen Laufverzeichnis. Der Prüflauf setzt keine künstlichen Blöcke.

Die Pflanzenansichten prüfen die geladenen globalen Voxel und beide Sichtlinien.
Die Kaktusansicht bevorzugt ein natürliches, großes Exemplar mit Armen und prüft
alle seine Segmente, die Verbindung über Blockflächen, Sandunterstützung und
die Höhenbegrenzung. Die vollständige Verteilung über sämtliche Höhen 2–8 ist
Gegenstand der automatisierten Generatorregression, nicht einer Einzelaufnahme.

## Spirit-/Blood-Texturen in 256 Pixeln

Die 13 Spirit- und 19 Blood-Cubetexturen werden direkt in 256×256 Pixeln
prozedural berechnet. Es handelt sich um zusätzliche Formen und Details,
nicht um hochskalierte 64-/128-Pixel-Bilder: zugespitzte Blattlagen mit
Mittel- und Seitenadern, Blütenfalten, unterschiedlich gerichtete Moosbüschel,
Erdschichten, unregelmäßige Steinfrakturen und feinere Holzfasern.
Die zuvor aufgehellte Blood-Palette bleibt erhalten. Die Texturen bleiben
einzeln austauschbare PNG-Dateien, ohne Atlas und ohne neue Lichtquellen.

15 passende Erd-, Gras-, Stein- und Holz-Cubes verwenden vorhandene,
ausschließlich nach innen gerichtete Oberflächendetails (`surface`). Die
Würfelaußenkanten bleiben geschlossen, `bevel_width` bleibt null. Laub bekommt
keine zusätzlichen Terrain-Einschnitte. Die Prop-Geometrie, Kollisionsformen
und GLBs werden von diesem Textur-Export nicht verändert.

```bash
python3 tools/assets/build_spirit_blossom.py --textures-only
python3 tools/assets/build_blood_forest.py --textures-only
python3 -m unittest discover -s tools/assets/biome_surfaces -v
python3 -m unittest discover -s tools/assets/blood_forest -v
```

Der gemeinsame Autorencode liegt in `tools/assets/biome_surfaces/`. Die vier
Artprüfungen und drei Blood-Assetprüfungen wurden erfolgreich ausgeführt;
ihre Protokolle liegen in
`tools/benchmarks/results/reference-biomes-f14-2026-11-14/biome-surface-tests.txt`
und `blood-assets-tests.txt`. Sie prüfen alle 32 PNG-Auflösungen, tatsächliche
Details innerhalb von 2×2-Pixelgruppen, deckendes Alpha, passende
Surface-Metadaten, periodische Abfragen und die lesbare Blood-Palette.
Die bestehenden GLB-Prüfungen kontrollieren weiterhin Grenzen, Normalen und
Dreiecke. Ein SHA256-Vergleich vor und nach dem gezielten Export bestätigte,
dass alle 134 vorhandenen GLBs unverändert blieben. Die Rust-Prüfung für
Würfelanschlüsse und Einwärtsrelief gehört zur gemeinsamen Cargo-Validierung.

## Gemeinsame Grundmaterialien der übrigen Biome

Gras und die davon abgeleiteten Erdbauformen nutzen jetzt eine zusätzliche
Materialableitung des vorhandenen Grasmodells. Erde zeigt ungleich breite,
erodierte Schichten, feine zusammenhängende Tonfragmente und verzweigte
Wurzelfasern. Die Grasoberfläche verwendet versetzte, gedrehte Halme mit
feinen Mittelrippen. Neue Farb- und Normalmaps werden zusammen berechnet;
die frühere Körnung der Normalmap wird nicht auf die neue Erde übertragen.
Die Farbkarten der einzelnen Flächen haben echte 512×512 Pixel. Das unverändert
UV-belegte Nahmodell verwendet eine kompatible 1024×1024-Karte mit vier
Quadranten, während LOD-Flächen weiterhin einzeln und wiederholbar bleiben.

`grass.json` bindet `models/fiverr_grass_reference_base.glb`. Die originale
Gras-GLB samt Import-Provenienz bleibt bestehen. Geometrie, UVs, Szenen, sieben
Materialslots, 24 Bildslots, Clamp-/Repeat-Sampler und matte ORM-Faktoren sind
unverändert. `DownLod` bleibt die gemeinsame PBR-Quelle für Erde, Slabs,
Treppen und Zäune. Schnee und Schneeschicht teilen ihre neue sanfte
512-Pixel-Textur; dafür entsteht kein zusätzlicher Materialslot.
Die bereits erneuerte Eichenrinde sowie die 512-Pixel-Runtime-Patterns für
Stein, Sand, Birke und Fichte bleiben erhalten.

```bash
# Vorschau ohne Änderung der aktiven Spielassets:
python3 tools/assets/biome_surfaces/base_materials.py
# Reproduzierbare Aktivierung der geprüften Materialableitung:
python3 tools/assets/biome_surfaces/base_materials.py --activate
```

Details zur Herkunft stehen in
`assets/terra/blocks/models/FIVERR_GRASS.md`; der SHA256-Verweis auf die
Importbasis in `fiverr_grass_reference_base.art.json`. Die jetzt insgesamt
acht Python-Artprüfungen sind erfolgreich im oben verlinkten
`biome-surface-tests.txt` protokolliert. Zusätzlich zu den Spirit-/Blood-Prüfungen
kontrollieren sie tatsächliche JSON-Bindungen, die gemeinsame Dirt-/Snow-
Materialnutzung, Geometriegleichheit, unverändertes PBR und die gespiegelte
Nah-/LOD-UV-Zuordnung. Das ist kein Ersatz für die anschließende native
Sichtprüfung der Biome.
