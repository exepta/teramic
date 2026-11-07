# Biome und Landschaften einstellen

Neue Welten verwenden `landscape_revision = 5` und standardmaessig `sea_level = 22`.
Die acht Biome-Dateien und optional `terrain_profiles.json` werden bei der Erstellung
unter `world.biome_definitions` gespeichert; fehlt die optionale Datei, gelten eingebaute Profile.
Altwelten 0..4 behalten Revision, Meeresspiegel und Profile auch in neuen Gebieten; keine Retrogen.
Auch bestehende Revision-5-Snapshots behalten ihre urspruenglichen Berge.
Die niedrigeren, breiteren Bergprofile gelten nur fuer neue Welten. Fehlende
alte Profilmetadaten werden aus einem eingefrorenen Legacy-Snapshot ergaenzt.

## Acht Biome-Dateien

Alle acht Dateien sind noetig; `biome` muss zur Datei passen. Falsche IDs, Felder oder Werte verhindern die Erstellung.
Die folgenden `terrain.min_height`/`max_height` gelten fuer den Hoehenvertrag von
Revision 4, nicht fuer die neue Landschaftsform von Revision 5:

| Datei / Biome-ID | Y-Band in Revision 4 |
| --- | --- |
| `plains.json` / `plains` | 32 bis 60 |
| `highlands.json` / `highlands` | 80 bis 200 |
| `desert.json` / `desert` | 50 bis 100 |
| `forest.json` / `forest` | 40 bis 90 |
| `cherry_forest.json` / `cherry_forest` | 45 bis 115 |
| `ocean.json` / `ocean` | -98 bis 22 |
| `mushroom_park.json` / `mushroom_park` | 38 bis 82 |
| `cliffted_desert.json` / `cliffted_desert` | 65 bis 145 |

Auch `horizontal_scale`, `detail_strength`, `relief_strength` und `terrace_strength`
formen den alten Revision-4-Hoehenpfad. `soil_depth` (0..32) und `tree_density`
(0..4) bleiben fuer Boden und Vegetation aktueller Welten wirksam; Masken, Hanglage
und globale Einstellungen bestimmen die tatsaechliche Schichtstaerke und Dichte.
Die zehn aktuellen Landschaftsprofile in `terrain_profiles.json` beschreibt [TERRAIN.md](../../TERRAIN.md).

## Ocean-Lebensraeume

Nur `ocean.json` enthaelt `ocean`. Klima und Kuestennaehe mischen seine Lebensraeume:

| Habitat | `floor`: sand / gravel / mud / dirt / stone | Korallen | Seegras | Tiefe |
| --- | --- | --- | --- | --- |
| `warm` | 70 / 5 / 4 / 1 / 20 | 0.80 | 0.55 | 90 |
| `cold` | 8 / 47 / 35 / 2 / 8 | 0 | 0.70 | 110 |
| `swamp` | 20 / 3 / 50 / 25 / 2 | 0 | 0.88 | 85 |

`floor` sind nichtnegative relative Gewichte mit positiver Summe; sie werden
normalisiert und raeumlich geclustert, nicht als exakte Flaechenanteile garantiert.
`coral_density`/`seagrass_density` gelten in 0..1, `max_vegetation_depth` in 1..128.
`cold_temperature`, `warm_temperature`, `swamp_humidity`: 0..1; kalte Schwelle < warme Schwelle.
Clay-Cluster: `clay_frequency` (0..1), `clay_cluster_scale` (2..128), `clay_depth` (0..8).
Tiefes Ocean-Terrain behaelt eine geschuetzte Hoehlendecke.
