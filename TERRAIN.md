# Terrain-Vertrag

Neue Welten verwenden wieder den Terrain-Vertrag von `main` (`d2b7956`):
Landschaftsrevision 1, Meeresspiegel 8 und die urspruenglichen Klima-, Hoehen-,
Terrassen-, Hoehlen- und Flussfelder. `Create World` aktiviert wie in `main`
die Hauptfluesse; der allgemeine/Dedicated-Standard bleibt `rivers = false`.
Die neue Grassblock-Darstellung und die volumetrischen Nadelkronen bleiben erhalten.
[HD-Gras, Main-Rueckbau und Pruefungen](tools/benchmarks/results/grass-hd-main-terrain-2026-10-27/README.md).
Bestehende Welten werden nicht umgestellt: Revisionen 0..5, Meeresspiegel,
Flussoption und gegebenenfalls JSON-Snapshots bleiben in ihren Manifesten.
Damit entstehen keine Terrain-Naehten in gespeicherten Welten durch den Rueckbau.

Die folgenden JSON-Profile und erweiterten Generierungspfade bleiben als
Kompatibilitaetscode fuer bereits gespeicherte Revisionen 4/5 erhalten.
Die acht [Biome-Dateien](assets/biome/README.md) steuern dort Oekologie;
ihre Hoehenparameter gelten fuer Revision 4, nicht fuer die Main-Landschaft.
Der erweiterte Snapshot verwendet Netzwerkprotokoll 77; Client und Server muessen denselben Stand nutzen.

## Landschaftsprofile

`assets/biome/terrain_profiles.json` enthaelt zehn geordnet gespeicherte Profile:

| `kind` | Standard-Y-Band |
| --- | --- |
| `plains` | 32..60 |
| `rolling_plains` | 35..105 |
| `forest` | 40..90 |
| `mountain_forest` | 50..160 |
| `fantasy_mountains` | 65..200 |
| `snowy_highlands` | 75..200 |
| `cherry_grove` | 45..115 |
| `spirit_forest` | 45..145 |
| `desert` | 50..100 |
| `ocean` | -98..22 |

`min_height`/`max_height`: geordnet in -120..350; Uebergaenge, Kuesten und Flusserosion formen die Mischung.
`horizontal_scale` (32..8192), `roughness`, `ridge_strength`, `erosion`, `overhangs`
(je 0..1) und `weight` (0..10; Ocean >0) steuern Form und relative Eignung.
Beim Ocean formen `roughness`, `ridge_strength` und `erosion` das Meeresbett;
`overhangs` bleibt dort absichtlich wirkungslos, damit keine unterseeischen 3D-Ueberhaenge entstehen.
Global: `warp_scale` (64..8192), `warp_strength` (0..512), `region_scale` (128..16384), `dune_wind_angle` (-360..360 Grad).
`spirit_forest` nutzt vorhandene Forest-/Mushroom-Oekologie, keine neuen Spirit-Assets.

Die Bergprofile verwenden breitere horizontale Skalen (Mountain Forest 1050 m,
Fantasy Mountains 1500 m, Snowy Highlands 1300 m), weniger Rauheit und weniger
Ueberhaenge. Fantasy Mountains haben Gewicht 0.35, Snowy Highlands 0.75:
ruhigere, breitere Bergformen statt der frueheren extremen 348-m-Profile.
Diese Werte gelten nur fuer explizite Revision-5-Profile, nicht fuer neue
Standardwelten und nicht als Retrogen-Schalter.
Vollstaendige gespeicherte Snapshots bleiben unveraendert. Fehlt in einem alten
Snapshot `landscapes` oder die gesamte `biome_definitions`, verwendet Revision 5
die eingefrorenen urspruenglichen Profile aus
`crates/terra-core/src/config/terrain/legacy_v5.json`, nicht wechselnde neue Defaults.

## Generierung und Grenzen

Klima-/Biomfelder sind von der lokalen Hoehe entkoppelt. Domain Warp kombiniert
OpenSimplex2-/OpenSimplex2S-FBm, Ridged, Hybrid-Multifractal, Billow, Cellular Distance2Sub.
Ein gebuendelter, begrenzter 128-Tile-Cache teilt Feldberechnungen. Seine
121 Stuetzpunkte pro Tile werden einzeln bedarfsgesteuert initialisiert:
Eine erste Abfrage benoetigt nur ihr 4x4-Stencil. Aktive Worker teilen diese
Punkte und werden bei Cache-Eviction nicht verdraengt.
Selektive CSG-Ueberhaenge schneiden nur 4..27 m unter der Hoehenhuelle; Wasserbetten bleiben geschuetzt.
Analytische und vorhandene Flusserosion, keine volle hydraulische/thermale Simulation und kein Gabor.
RiverNetwork: hoechstens drei Zufluesse pro 2048-m-Region, kein unbegrenzt globaler Graph.
Zufluesse nutzen absteigendes D8-Routing und feuchtegewichtete Einzugsflaechen;
ungeeignete Routen werden verworfen. Einzelne Laeufe koennen vorhandene
Karstpassagen unter hoeherem Terrain nutzen; dies ist kein rein oberirdisches
globales Drainagemodell. Wasserfaelle verwenden die vorhandenen
Reservoirquellen und begrenzten Fluessigkeitsupdates. Natuerliche Gewaesser
bleiben erneuerbare Quellen, kein geschlossener Niederschlags-/Wasserkreislauf.
Explizite flache Welten behalten `base_height`.

## Debug und Bewegung

F1 schaltet standardmaessig das Debug-Overlay; F1+G wechselt zehn Terrain-Heatmaps.
Ohne Tastatureingabe waehlt zum Beispiel `TERRA_TERRAIN_DEBUG=height cargo run -p terra-client`
die Hoehenkarte und aktiviert das Debug-Overlay beim Start. Gueltige Namen sind
`continentalness`, `erosion`, `temperature`, `humidity`, `peaks_valleys`, `height`,
`density`, `biome`, `rivers` und `water`. Ohne Variable oder mit `off` bleibt der
Debug-Zustand unveraendert; ungueltige Werte erzeugen nur eine Warnung.
Creative-Flug: schnelles Doppel-Space mit Kollision. Nur Spectator (Gamemode 2): Noclip und verborgene Hotbar.
Explizit erlaubter Survival-Flug nutzt denselben Doppeltipp; standardmaessig ist er aus.

## Reproduzierbare Pruefungen

`revision_one_heights_match_the_pre_f40_executable_snapshot` vergleicht die
urspruenglichen `main`-Hoehenanker des Seeds 42042. Der Regressionstest
`main_terrain_defaults_ignore_later_profiles_at_negative_boundaries_and_on_reload`
prueft Seeds 7, 42042 und 99173 an positiven/negativen Sectiongrenzen:
Hoehen, Oberflaechen, Dichte, Materialien und Vegetation ignorieren fuer den
Main-Vertrag spaetere JSON-Profile. Storage-Tests erhalten separat alle
gespeicherten Revisionen 0..5 und Spieler-Edits beim Laden/Speichern.

```bash
cargo run --locked -p terra-world --example terrain_profiles -- \
  --seed=42042,7,99173 --output=/tmp/teramic-terrain --benchmark
cargo run --locked -p terra-world --example terrain_sections --features bevy
cargo run --locked -p terra-world --example terrain_profiles -- \
  --seed=42042,7,99173 --output=/tmp/teramic-terrain --world-snapshots
cargo run --locked -p terra-client -- --smoke-test-fiverr-grass
```

Das erste Beispiel misst Hoehenabfragen ohne Rendering oder Hydrologie,
das zweite generiert echte Sections mit Voxeln, Props, Hoehlen und Wasser,
aber ohne Meshing/GPU. Unterschiedliche Revisionen erzeugen unterschiedliche
Geometrie; CPU-Zeiten sind keine In-Game-FPS.
`--world-snapshots` exportiert exakt die gescannte Konfiguration inklusive
Profilen, aber mit den im Diagnosebeispiel abgeschalteten Hoehlen und Baeumen.
Die [Grassblock-Vorschau](FIVERR/README.md) laedt das Original-GLB nur im
expliziten Testmodus direkt aus `FIVERR/`. Im normalen Spiel verwendet ID 1
das daraus erzeugte Asset aus `assets/terra/blocks/models`: geteilte PBR-Faces
im Nahbereich, gekachelte PBR-Quads mit Greedy-Meshing in der Ferne.
Details, native Aufnahmen und offene Einschraenkungen stehen im
[Messbericht](tools/benchmarks/results/advanced-terrain-2026-10-27/README.md).
Die spaetere Hoehenkorrektur und der native Grassblock-Test stehen im
[aktuellen Pruefbericht](tools/benchmarks/results/natural-terrain-grass-2026-10-27/README.md).
