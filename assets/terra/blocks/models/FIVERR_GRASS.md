# Fiverr-Grassblock

## Aktive Materialableitung für die Biomreferenzen

`grass.json` verwendet nun `fiverr_grass_reference_base.glb`. Diese zusätzliche
Datei übernimmt die unten dokumentierte Importbasis unverändert in Geometrie,
UVs, Tangenten, Indexdaten, Szenen und Materialslots. Nur die Bilder der sieben
sichtbaren PBR-Materialien werden ersetzt: prozedural erzeugte zusammenhängende
Erde, Grashalme und dazu passende flache Normalmaps. Das matte ORM und sämtliche
Materialfaktoren bleiben erhalten. Es gibt weiterhin sieben Materialien und
24 Bildslots; Clamp/Repeat und die gespiegelten Seiten bleiben unverändert.

Die sechs eigenständigen Albedodateien liegen unter
`../textures/reference_base/`. Top, Side, gespiegelte Side, Earth und Snow haben
512×512 Pixel. Nur das vorhandene Nahmodell benötigt wegen seiner unveränderten
UVs die vier Quadranten in `grass_near.png` mit 1024×1024 Pixeln. Das ist kein
neuer gemeinsamer Blockatlas. Die fünf Gras-/Erdbilder besitzen jeweils eine
korrespondierende Normalmap. Das Dirt-Material und seine Bauformen verwenden
weiterhin denselben `DownLod`-PBR-Slot; Schnee und Schneeschichten teilen eine
separate Textur.

Der Export lautet:

```sh
python3 tools/assets/biome_surfaces/base_materials.py --activate
python3 -m unittest discover -s tools/assets/biome_surfaces -v
```

Ohne `--activate` schreibt der Generator lediglich eine Vorschau nach
`/tmp/terra-reference-base-preview`. Das Herkunftsprotokoll
`fiverr_grass_reference_base.art.json` enthält den SHA256-Fingerprint der
Importbasis. Tests vergleichen die vollständigen Mesh-/Accessor-/Node-Daten,
den ursprünglichen Binärpräfix, alle PBR-Faktoren, Bild-/Materialanzahlen und
die affine Nah-/LOD-Zuordnung aus der ursprünglichen Provenienz.
`FIVERR/VoxelGrassBlock glb file.glb`, `fiverr_grass_faces.glb` und dessen
Import-Provenienz wurden dafür nicht geändert. Der folgende Abschnitt
beschreibt diese weiterhin reproduzierbare Importbasis.

`fiverr_grass_faces.glb` wird aus der unveraenderten Originaldatei
`FIVERR/VoxelGrassBlock glb file.glb` erzeugt. Das wiederverwendbare Rust-Werkzeug
liest ein JSON-Rezept und verwendet die bereits vorhandenen glTF- und PNG-APIs.

```sh
cargo run -p terra-render --example import_block_faces -- tools/assets/import_fiverr_grass.json
cargo run -p terra-render --example import_block_faces -- tools/assets/import_fiverr_grass.json --check
```

Der zweite Aufruf prueft die abgeleitete GLB und ihre Provenienz bytegenau,
ohne Dateien zu schreiben. Die JSON-Provenienz enthaelt stabile FNV-1a-64-
Fingerprints, Quellgroessen, Dreieckszahlen, UV-Abbildungen und deren Residuen.
Fingerprints dienen der Reproduzierbarkeit, nicht als kryptografischer Nachweis.
Die erzeugten Dateien gehoeren zusammen mit Rezept und Original ins Repository.

## Nahansicht

Das Original hat 1494 Vertices und 768 Dreiecke. Die Dreiecke werden anhand ihrer
eindeutigen geometrischen Hauptnormalen in `West`, `East`, `Down`, `Up`, `North`
und `South` partitioniert. In diesem Schritt werden keine Dreiecke neu erzeugt
oder entfernt.
Positionen werden um `[0.5, 0.5, 0.5]` in die Blockzelle `[0, 1]` verschoben;
Normalen, UVs und Winding bleiben bei der Partitionierung erhalten. Die drei
eingebetteten Original-PNGs werden bytegleich im abgeleiteten Asset erhalten;
die Original-GLB wird niemals veraendert. Materialslot 0 bleibt das gemeinsam
genutzte Nahmaterial. Seine Bildbindungen werden bei aktivem `appearance`
durch die unten beschriebene Darstellung ersetzt. Fehlende Tangenten werden
beim normalen Bevy-glTF-Import passend zur gebundenen Normalmap erzeugt.

Die sechs Gruppen lassen Nachbarflaechen im Terrain-Mesher einzeln ausblenden.
`grass.json` verwendet `face_groups` und einen Join-Bereich von 0.02 Bloecken,
damit angrenzende, leicht gewoelbte Originalkanten geschlossen werden koennen.
Das Werkzeug setzt diese Kanten in der Nahgeometrie ausdruecklich nicht gerade.

Das optionale `relief` im bestehenden Importrezept ergaenzt danach blockige
Graslippen und kleine Steinformen an passenden gruenen/grauen Albedoflaechen.
Dazu werden die inneren Seiten leicht vertieft, die Formen sitzen darin und
bleiben vollstaendig in `[0,1]`. Die aeusseren Join-Raender bleiben unveraendert.
Die Grasoberseite erhaelt ein sehr flaches Relief ohne zusaetzliche Dreiecke.
Das veraendert nur das abgeleitete Nahmesh, nicht die Quelldatei oder deren
PBR-Bilder. Pro Erdseite gelten feste Obergrenzen fuer Gras-/Steinformen;
keine eigenen Entities, Materialien oder laufenden Texturabfragen im Spiel.
Die Provenienz enthaelt die tatsaechlichen Formen- und Geometriezaehler.

## Ausgewaehlte Darstellung

Das aktive Importrezept verwendet `appearance.base_color` mit
[`grass-reference-hd-v1.png`](../textures/grass-reference-hd-v1.png) und
`appearance.normal` mit
[`grass-reference-normal-v1.png`](../textures/grass-reference-normal-v1.png).
Beide Atlanten haben tatsaechlich 1254x1254 Pixel, also 627x627 Pixel je
Quadrant. Die ausgewaehlte Albedo wird bytegleich eingebettet, nicht neu
skaliert oder umgefaerbt.

Die Normalmap wird als lineare technische PBR-Daten behandelt: Ihre
XY-Komponenten werden auf der CPU mit Staerke 0.45 gewichtet, die Vektoren
in die positive Z-Hemisphaere begrenzt und renormalisiert. Der Material-
Normalfaktor ist danach 1.0, damit die Staerke nicht doppelt angewandt wird.
Eine gemeinsame 1x1-ORM-PNG enthaelt AO 255, Roughness 224 (0.88 quantisiert)
und Metallic 0. Roughness-Faktor 1.0 und Metallic-Faktor 0.0 erhalten die
matte, nichtmetallische Oberflaeche.

Im Weltshader wird ausschliesslich die gruene Grasoberseite zu 78% auf ein
helleres einheitliches Gruen gezogen. Die verbleibenden 22% erhalten feine
Texturdetails, ohne viele konkurrierende Farbtoene. Der Test beruecksichtigt
die geometrische Flaechennormale und Blockoberkante; seitliche Graslippen,
Erde und Steine werden nicht umgefaerbt. Biometint wird vor der Farbauswahl
entfernt und danach wieder angewandt. Jahreszeiten, Naesse, Schnee, Normalmap
und echte Reliefgeometrie bleiben erhalten. Die eingebetteten PNGs bleiben
dadurch unveraendert.

Die Originalbilder bleiben erhalten, drei neue Appearance-Bilder werden
ergaenzt. Zusammen mit den 18 LOD-Bildern enthaelt das Asset 24 Bilder und
sieben Materialien. Alle sichtbaren Bloecke werden weiterhin in bestehenden
Section-/Materialbatches gesammelt; es entsteht kein eigener Draw Call oder
Material-Handle pro Block.

Die Aufloesung und die echte Nahgeometrie naehern sich der Bildreferenz an,
stellen aber keine pixelgenaue 1:1-Reproduktion dar. Insbesondere Bildlicht,
Perspektive und die Anzahl geometrischer Details bleiben vom Spielrenderer
und dessen Detailstufe abhaengig.

## Grobe LOD

`WestLod` bis `SouthLod` sind sechs flache Unit-Quads mit eigenen Materialslots
1 bis 6. Ihre UVs entsprechen dem Greedy-Mesher: West/Ost verwenden U=Z, V=Y,
Unten/Oben U=X, V=Z und Nord/Sued U=X, V=Y, jeweils positive Weltachsen.
Die LOD verwendet keine alten Turf-/Earth-Ersatzmaterialien.
`dirt.json` und die Dirt-Bauformen referenzieren mit `coarse_face` den Slot von
`DownLod`. Sie teilen exakt dessen Albedo-, Normal- und ORM-Handles und dessen
Roughness. Es wird keine ungefaehr passende Ersatztextur generiert.
Greedy-Flaechen werden vor der Kontaktbeleuchtung nur dort lokal unterteilt,
wo die tatsaechliche Halo-Verschattung von der Dreiecksinterpolation abweicht.
Gleichmaessig beleuchtete Flaechen bleiben unveraendert; pro Eingangsrechteck
entstehen hoechstens 64 Teilrechtecke. Damit faerbt eine einzelne erhoehte
Nachbarzelle nicht mehr eine ganze grosse Grasflaeche dunkel.
Die affine Atlasmessung erfolgt weiterhin auf den unveraenderten
Originalflaechen, bevor das Nahrelief hinzugefuegt wird. Die LOD-Texturen
werden dagegen aus der ausgewaehlten Appearance-Albedo, Normalmap und ORM
gebacken; ohne Appearance-Override verwenden sie die Original-PBR-Bilder.
Eine getrennte `Coarse`-Szene referenziert die sechs LOD-Meshes und haelt deren
PBR-Subassets im Bevy-Asset-Lebenszyklus. Die Standardszene `Near` referenziert
das aufbereitete Nahmesh; keine der beiden Szenen wird als Terrain-Entity gespawnt.

Nur fuer die affine Atlasmessung werden Koordinaten im 0.02-Randbereich auf
0 oder 1 gesetzt. Ohne diesen Schritt passen die gewoelbten Rand-Vertices nicht
zum Atlasrechteck. Die maximale gemessene UV-Abweichung nach dem Snap liegt
beim Original unter 0.000001; bei groesseren Fehlern bricht der Import ab.

Je Seite werden BaseColor, Normal und ORM in drei eingebettete 512x512-PNGs
gebacken. BaseColor wird fuer bilineare Filterung linearisiert; Normal und ORM
bleiben linear. Die XY-Komponenten der Normalmap werden in die neue UV-Basis
rotiert/gespiegelt und danach normalisiert. Alle LOD-Sampler verwenden Repeat,
lineare Vergroesserung und trilineare Minifizierung. UV-Wiederholung wiederholt
den ausgewaehlten Blockatlas, sie erfindet keine neuen nahtlosen Texturinhalte.

## Grenzen

Die beiden neuen Bitmap-Assets wurden mit dem eingebauten ImageGen-Werkzeug
erstellt. Referenzrollen, vollstaendige Prompts und tatsaechliche Bildgroesse
stehen in [grass_reference_art.json](../../../../tools/assets/grass_reference_art.json).
Die Prompts forderten 2048 Pixel; geliefert und unvergroessert eingebunden
wurden 1254 Pixel. Die technische Normalmap-Aufbereitung ist reproduzierbarer
Rust-Code und keine erneute Bildgenerierung beim Spielstart.

Der Import akzeptiert bewusst nur einen statischen, identitaetstransformierten
Mesh-Knoten mit einem indizierten Dreiecks-Primitive, einem PBR-Material,
eingebetteten PNGs und untransformierter UV0. Animationen, Skins, Morphs,
weitere Vertexattribute, glTF-Erweiterungen und mehrdeutige Face-Normalen werden
abgewiesen. Die LOD setzt einen achsenparallelen affinen Atlas und eine gemeinsame
ORM-Textur fuer Occlusion und Metallic/Roughness voraus. Ein neues Asset bekommt
ein eigenes Rezept und muss dieselben Validierungen erfuellen.
