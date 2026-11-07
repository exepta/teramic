# Grassblock im Terrain

Der aufbereitete FIVERR-Grassblock wird fuer Block-ID 1 in der Spielwelt verwendet,
auch in bereits gespeicherten Welten. Nur die Darstellung aendert sich;
Kollision, Abbau und gespeicherte Block-IDs bleiben unveraendert.

Die unveraenderte Quelldatei wird mit dem
[Importrezept und Werkzeug](../assets/terra/blocks/models/FIVERR_GRASS.md)
in ein Terrain-Asset aufbereitet. Die Original-GLB und ihre drei eingebetteten
Original-PNGs bleiben unveraendert. Das aktive `appearance`-Rezept verwendet
die neue [HD-Albedo](../assets/terra/blocks/textures/grass-reference-hd-v1.png)
und [Normalmap](../assets/terra/blocks/textures/grass-reference-normal-v1.png)
fuer das weiterhin gemeinsam genutzte Nahmaterial. Sechs Flaechengruppen erlauben das Ausblenden verdeckter
Nachbarseiten. Der 0.02-m-Joinbereich schliesst angrenzende Modellkanten.
Das Importrezept ergaenzt in der Nahansicht echte blockige Graslippen und
Steinchen vor den leicht vertieften Seiten. Sie folgen den gruenen/grauen
Bereichen der ausgewaehlten Albedo und teilen deren Materialslot; die
Quelldatei bleibt unveraendert.
[Bilder, Geometriebudget und Grenzen](../tools/benchmarks/results/grass-hd-main-terrain-2026-10-27/README.md).
Ferne Flaechen nutzen kachelbare PBR-Texturen aus derselben ausgewaehlten Darstellung und
Greedy-Meshing, nicht das alte Gras. Tangenten und Mipmaps bleiben erhalten;
keine Entity und kein eigener Material-Handle pro Terrainblock.

## Tatsaechliche Texturaufloesung

Albedo und Normalmap sind jeweils 1254x1254 Pixel grosse Atlanten mit
627x627 Pixeln je Quadrant. Die Albedo-PNG wird bytegleich eingebettet;
die Normalvektoren werden auf der CPU linear mit Staerke 0.45 aufbereitet und
normalisiert. Roughness 0.88 und Metallic 0 sorgen fuer eine matte,
nichtmetallische Oberflaeche. Die sechs groben LOD-Seiten bleiben 512x512 Pixel.
Das abgeleitete Asset enthaelt insgesamt sieben Materialien und 24 Bilder,
nicht einen eigenen Draw Call pro Block.

Die neue Textur und das echte Nahrelief orientieren sich an der Referenz.
Das ist keine Behauptung einer pixelgenauen 1:1-Reproduktion: Perspektive,
Beleuchtung, gebackene Bilddetails und die begrenzte Nahgeometrie beeinflussen
das Ergebnis weiterhin.

## Vergleichsszene

`cargo run --locked -p terra-client -- --smoke-test-fiverr-grass` startet nur eine temporaere,
flache Offline-Vergleichswelt. `TERRA_CAPTURE_DIR=/tmp/terra-fiverr-grass` waehlt
den Ausgabeordner; ohne Variable wird derselbe Ordner unter dem System-Temp verwendet.

Links steht das originale GLB als 2x2-Testfeld, rechts der neue Terrain-Grassblock,
platziert ueber echte Welt-Edits. Beide haben einen Meter Kantenlaenge und nutzen
dasselbe Tageslicht. Vier Aufnahmen zeigen Vorderseite, Schraegansicht, Oberseite
und dieselbe Vorderseite mit dem flachen PBR-Fallback. Unter und neben dem
Terrain-Testfeld liegt Dirt mit demselben eingebetteten Bodenmaterial wie die
Grassblock-Unterseite. Nur die temporaere Vergleichsszene schaltet fuer die
vierte Aufnahme die Nahgeometrie aus; die normalen Detailvorgaben bleiben erhalten.

Der benannte, nur lesbare AssetSource `fiverr` laedt die Originaldatei direkt aus
diesem Ordner, unabhaengig vom Startverzeichnis. Vier Vorschau-Instanzen teilen
einen Mesh und ein PBR-Material mit UVs, Normalmap, ORM und generierten Tangents.
Die urspruenglich mittigen Koordinaten werden nur durch den Instanz-Pivot um 0.5
verschoben. Die Datei wird weder veraendert noch nach `assets` kopiert.

Die linke Referenz zeigt bewusst unveraenderte Originalinstanzen einschliesslich
ihrer dunklen Join-Fugen. Rechts laufen die echten Welt-Edits durch die neue
Terrain-Pipeline mit geschlossenen Nachbarkanten. Das Original hat insgesamt
768 Dreiecke; der Terrain-Mesher emittiert nur die sichtbaren Face-Gruppen und
nutzt ausserhalb des Nahbereichs flache, zusammengefasste Quads.
