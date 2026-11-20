# Laub mit freigestellten Blattgruppen

Die zehn vorhandenen Laubtypen verwenden zwanzig eigene Seiten-/Oberseitenbilder
in 512 × 512 Pixeln. Die neue Gestaltung ersetzt das regelmäßige 3×3-Raster durch
unregelmäßige Zweige, geschichtete Einzelblätter, gebogene Adern, Licht-/Schatten-
flächen und einzelne Blütengruppen. Die Geister- und Blutwaldpaletten bleiben erhalten.

Zwischen den Blattkonturen liegen echte transparente Zwischenräume. Je Bild sind
rund 30–33 % der Pixel freigestellt; Blattinnenflächen sind vollständig deckend.
Auch die vier Bildränder enthalten Lücken. Es gibt keine künstlich verteilten
Zufallslöcher im Blattinneren. Getrennte Oberseiten zeigen anders angeordnete
Blattgruppen. Die 20 Pfade und zehn Block-IDs bleiben erhalten. Zusätzlich liefern
elf separate 512px-Bilder mit vier vollständigen Motiven: zwei Blattvarianten
und zwei Blüten-, Frucht- oder Nadel-/Zapfenmotive. Transparente Zellränder
trennen die Motive. Die bestehenden `*_cluster.png`-Pfade bleiben erhalten;
eigene Datenpakete mit diesem Metadatum müssen das Viermotiv-Layout liefern.

## Darstellung und Entfernung

`meshing/tree_details/broadleaves.rs` erzeugt pro exponierter Blockseite zwölf
einzeln gefaltete und geneigte Blätter sowie zwei vorgeschobene Blüten, Früchte
oder Zapfen. Die sechseckigen Blätter besitzen einen gewölbten Mittelpunkt und
verschiedene Tiefen, Größen und Ausrichtungen. Sie überlappen benachbarte Kanten
innerhalb von 25 cm Überstand. Die Motive werden vollständig innerhalb ihrer
Texturzelle abgebildet. Es gibt keine großen bedruckten Gruppenkarten, keine
geschlossene Würfelschale und keine innere Füllfläche. Die einzelnen Motive
bleiben beidseitige Cutout-Flächen, keine geschlossenen Blattvolumen.
Bei rosa und weißem Kirschlaub sind auch die zwölf Hauptmotive vollständige
Blüten; die Farben sind rosa beziehungsweise cremeweiß. Flachere Faltungen
vermeiden auffällige spitze Blattmitten und harte Selbstschatten.

Nahe freistehende Laubzellen benötigen maximal 504 Dreiecke und 588 Vertices,
die mittlere Detailstufe 168 Dreiecke und 336 Vertices. Je Nahmotiv teilen sich
sechs Dreiecke sieben Eckpunkte; Middle verwendet vier Eckpunkte. Das kostet
mehr Geometrie als die früheren 192/48-Gruppenkarten und bleibt auf die
vorhandenen Nah-/Mittelbereiche begrenzt.
Entfernte/Low-Zellen und Inventaransichten behalten den vorhandenen Cube-Pfad mit
den vollständigen Seiten-/Oberseitentexturen. Die zusätzliche Geometrie bleibt gecacht und im Chunkbatch;
es entstehen keine Entities pro Blatt. Meshgrenzen enthalten den echten Überstand
und die bestehende Windreserve. Spruce/Fir verwenden auf dem vorhandenen grünen
Laubblock eigene Nadel- und Zapfenmotive. Datenpakete ohne
`foliage_cluster`-Metadatum behalten die vorherige Blatt-/Nadelgeometrie.

Maskierte Materialien zeichnen beide Blattseiten, einschließlich der Vorschau-
materialien. Die aktiven PNGs durchlaufen abdeckungsbewahrende Mipmaps mit
vormultiplizierter linearer Farbfilterung: Lücken sollen in der Entfernung nicht
ungewollt zuwachsen oder schwarze Farbsäume erzeugen. Farb-, Tiefen- und Schatten-
pass verwenden dieselbe Texturmaske und Windbewegung. Laub bekommt keinen flachen
Würfel als Schattensurrogat; die sichtbare Geometrie wirft selbst Schatten.
Dies gilt auch für bestehende Nadelkronen, deren Schatten dadurch mehr Geometrie
verarbeiten als zuvor. Die genannten Dreieckszahlen sind Geometriebudgets und
keine FPS-Messung.

## Prüfung und Grenzen

Regressionen prüfen verdeckte Seiten, UV-Ausrichtung und -Varianten, Dreieckswinding,
Überstand, LOD-Budgets, Eigentümerbereiche sowie echte PNG-Lücken und deren
Abdeckung bis zur 16px-Mipstufe. Schattenregressionen prüfen alle zehn IDs und
Materialslots, die mit anderen Blöcken geteilt werden. Vorhandene Tests erhalten
Stammflächen hinter transparentem Laub und entfernen verdeckte Blattinnenflächen.
Die neuen Gruppenprüfungen kontrollieren außerdem vollständige Sprite-UVs,
unterschiedliche Tiefen und Neigungen, gemeinsame LOD-Motivanker und UV-Zellen, Far/Low-Fallback,
separate Sprite-Materialslots und unveränderte Eigentümerdaten der Ursprungszelle.
Ein zusätzlicher Test projiziert die gecachte Geometrie zusammen mit der
Alpha-Maske der ausgelieferten PNGs auf jede einzelne Fläche. Dieser frontale
Test allein prüft keine räumlichen Kantenlücken. Ein weiterer Regressionstest
verwendet deshalb die endgültigen Batch-Indizes und UVs aller sechs Seiten mit
perspektivischen Strahlen aus der tatsächlichen Galeriekamera sowie zwei
weiteren Kamerapositionen. Er prüft alle vier Varianten auf dichte Kronenmitten
und begrenzt große offene Löcher.
Die räumliche Prüfung verwendet die echten Motivzellen und die tatsächlichen
Batch-Indizes. Sie erkennt offene Kronenmitten auch bei schrägem Blick. Die
Assettests prüfen außerdem, dass die vollständigen Motive innerhalb der
hexagonalen Geometrie liegen und keine benachbarten Zellen hineinfiltern.

Die native Galerie entsteht mit `tools/benchmarks/run-foliage-creatures.sh` über
den normalen Chunkrenderer; Wald-/Kirschwaldansichten entstehen mit
`tools/benchmarks/run-reference-biomes.sh picker-forest picker-cherry-forest`.
[Aktueller Prüfbericht und Aufnahmen](../tools/benchmarks/results/fantasy-sandworm-ambience-2026-11-16/README.md).

Die Blockpositionen und die grobe Kronensilhouette bleiben voxelartig. Die Bilder
sind das Gestaltungsziel, keine Behauptung pixelgleicher Einzelblattmodelle.
[Texturerstellung](../tools/assets/biome_surfaces/FOLIAGE.md) und
[Rabenflug](raven-flight.md) beschreiben die zugehörigen Änderungen.
