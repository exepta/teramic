# Natürliche Dekoration und Felsformationen – Ecology 5

Revision 5 führte die unten beschriebenen Naturpatches ein; neue Welten verwenden
inzwischen [Revision 6](biome-flora-ecology-6.md). Gespeicherte Revisionen 0–4 behalten
ihre bisherige Dekoration; fehlende Revisionsfelder bleiben 0. Höhenfelder,
Flussläufe und Wasserstände werden durch diese Änderung nicht angepasst.

Blutwald-Pflanzen entstehen in kleinen Gruppen einer Pflanzenart. Ein globales,
seedabhängiges Feld lässt zusammenhängende freie Bereiche übrig. Kleine Gruppen
liegen in einem versetzten 8-Meter-Raster; große Einzelobjekte verwenden ein
separates 16-Meter-Raster mit mindestens 12 Metern Abstand zwischen ihren Ankern.
Pilze und Wurzeldekoration bevorzugen Baumumgebungen, halten aber den Stammfuß
frei. Bodenmaterial, Hangneigung, Wasser, tatsächliche Bodenunterstützung,
Baumbelegung und natürliche Höhleneingänge begrenzen die Verteilung.

Felsgruppen bestehen aus echten abbaubaren Weltblöcken. Mehrere überlappende,
gedrehte und unterschiedlich hohe Felskörper ergeben unregelmäßige Umrisse. Die
unterste Lage ist örtlich in vorhandenen Boden eingebettet; es wird weder eine
rechteckige Plattform noch ein tiefer Fundamentsockel ergänzt. Hochland und
Klippen bevorzugen Felsaufschlüsse, Wälder und Ebenen erhalten vereinzelte Gruppen
in freien Bereichen. Verwendet werden bestehende Stein-, Moosstein-, Sandstein-,
Spirit-Stein- und Blutstein-Materialien.

Das System plant unveränderliche 32-Meter-Patches im vorhandenen Worker-Pfad.
Maximal 128 Patches bleiben gecacht. Globale Koordinaten und euklidische Division
verhindern negative Chunk-Nähte; Ankunfts- und Generierungsreihenfolge beeinflussen
die Ausgabe nicht. Grundterrain wird zuerst erzeugt, danach Naturpatches, Bäume,
Bauwerke und zuletzt gespeicherte Spieleränderungen. Naturpatches halten Abstand
zu den vollständigen Grundrissen erzeugter Bauwerke.

Für echte Inspektionspunkte ohne künstliche Weltänderungen:

```bash
cargo run -p terra-world --example inspect_rock_formations
TERRA_ROCK_USE_CONFIG=1 cargo run -p terra-world --example inspect_rock_formations
```

Ohne Umgebungsvariable verwendet der Finder `WorldConfig::default()` mit Seed42042;
mit Variable verwendet er `configs/world.toml`. Beide suchen mit Ecology5 und
geben Tile, Bounds, Material, Voxelanzahl und Kameravorschläge als JSONL aus.

Die Messung `measure_natural_features` untersucht 144 Spalten beziehungsweise
36.864m² bei `[-6656,-8192]`, Seed42042. Mit den tatsächlich vorhandenen Bäumen
und Geländebedingungen sank die Anzahl kleiner Blutwald-Props von 5.283 auf 365
(146,75 auf 10,14 pro 1.024m²), große Props von 363 auf 8. Kalte Spaltenvorbereitung
brauchte 306 gegenüber 319ms, warme 241 gegenüber 243ms; der Aufbau von 181
Oberflächen-Sections 76,7 gegenüber 73,0ms. Das sind einzelne lokale CPU-Messungen,
keine allgemeine FPS-Garantie. Rohdaten und der vollständige erfolgreiche
World-Testlauf liegen unter `tools/benchmarks/results/blood-forest-voxel-ecology-2026-11-14/natural-features/`.
