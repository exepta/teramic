# Niedrigere Berge und Original-Grassblock

## Terrain

Nur die Defaults von Mountain Forest, Fantasy Mountains, Snowy Highlands und
Spirit Forest wurden angepasst: niedrigere Hoehenbaender, breitere Skalen,
weniger Rauheit und Ueberhaenge. Fantasy-Gewicht 1.0 -> 0.35,
Snowy-Highlands-Gewicht 1.0 -> 0.75. Keine zweite Engine oder neue Block-IDs.
Zielrichtung der drei Referenzen: nutzbare Taeler und zusammenhaengende
Bergraender statt extremer einzelner Erhebungen; keine behauptete 1:1-Kopie.

Identischer 32x32-Scan pro Seed, +/-8192 m, 512 m Raster:

| Seed | Grundterrain-Maximum vorher | Nachher | Meeresminimum vorher/nachher |
| --- | ---: | ---: | ---: |
| 42042 | 324.348035 | 167.008280 | -95.914723 |
| 7 | 320.695255 | 177.041740 | -95.708099 |
| 99173 | 315.623517 | 171.690324 | -97.960035 |

Dies sind Stichproben vor Fluss-/Seeerosion, keine globalen Maximalgarantien
und keine FPS-Messungen. Zusaetzlicher Inlandtest (Continentalness >=0.24):
p95 sinkt fuer die drei Seeds von 229.60/241.73/230.68 auf
136.62/144.77/147.27 m; p10..p95-Relief bleibt 78.53/86.10/90.33 m.

Vollstaendige alte Welt-Snapshots bleiben erhalten. Fehlende alte Profile oder
fehlende Biome-Metadaten verwenden den eingefrorenen `legacy_v5.json`-Stand.
Frische persistente Welten und neue `--no-save`-Dedicated-Welten erhalten die
aktuellen Profile. Kein Retrogen gespeicherter Chunks oder neuer Gebiete in Altwelten.

## Grassblock

`FIVERR/VoxelGrassBlock glb file.glb` wird direkt ueber einen eigenen read-only
AssetSource geladen, nur mit `--smoke-test-fiverr-grass`. Die Datei wurde nicht
veraendert (Git-Blob `f048c4186320e95e406c693a4c15004079807cf8`).
Die Vorschau teilt einen Original-Mesh und ein Original-PBR-Material zwischen
vier Instanzen; daneben stehen vier ueber echte Welt-Edits platzierte Grassbloecke.

- GLB 2, ein Mesh/Primitive/Material, 1494 Vertices, 768 Dreiecke.
- Exakt 1x1x1 m; Original-Pivot -0.5..0.5 wird per Transform ausgeglichen.
- Drei eingebettete 1024x1024-PNGs: Base Color, Normal, gemeinsames ORM.
- UVs und durch Bevy generierte Tangents bleiben erhalten.
- Opaque PBR, Backface-Culling, kein Metallanteil in der ORM-Textur.

Noch kein globaler Terrain-Ersatz: Das Original besitzt keine sechs getrennten
FaceGroups; die meisten Dreiecke liegen nicht auf den aeusseren Zellflaechen.
Verdeckte Nachbarflaechen werden in dieser unveraenderten Vorschau nicht entfernt.
Die bisherige Terrain-Template-Batchpipeline uebernimmt zudem keine Tangents;
sie wird fuer diesen Test nicht umgebaut. Terrain-LOD und eingebettete
Textur-Mipmaps benoetigen eine gesonderte Integration. 768 statt 12 Dreiecken
pro nacktem Vollwuerfel sind kein kostenloser Welt-Ersatz.

## Pruefungen

- `cargo fmt --all --check`: erfolgreich.
- `cargo check --locked --workspace --all-targets -j1`: erfolgreich.
- `cargo test --locked --workspace -j1 -- --test-threads=2 --nocapture`:
  2716 bestanden, 7 ignoriert, einschliesslich Doc-Tests.
- `cargo clippy --locked --workspace --all-targets -j1 -- -D warnings`: erfolgreich.
- `cargo build --locked --workspace -j1`: erfolgreich, Client und Dedicated Server.
- Neuer GLTF-Loader-Test prueft Originaldatei, benannten read-only Reader,
  UVs/Tangents, PBR-Bindings und Bilder; Framing und frische Flatconfig separat getestet.
- Determinismus und Original-Geometrie-Bits bei alten Snapshots sowie negative
  Section-/Tile-Grenzen werden durch die Terrain-Tests abgedeckt.

Rohdaten: `/tmp/teramic142-before-survey`, `/tmp/teramic142-after-survey`;
Testlog `/tmp/teramic142-tests-final.log`. Der Snapshot-Export
`terrain_profiles --world-snapshots --output DIR` schreibt exakt die gescannte
Konfiguration, damit native Tests nicht versehentlich den Legacy-Fallback laden.

## Native Sichtpruefung

RADV/Vulkan, echte Clientfenster mit 1280x800, hohe Wasser-/Wetter-/Schattenqualitaet.
Keine parallelen Cargo-Laeufe waehrend der nativen Aufnahmen.

- Grassblock: Vorderseite, Schraegansicht und oben; alle drei GPU-Pixelpruefungen
  bestanden und Bilder manuell angesehen. Original links, Terrain-Grass rechts.
  Deutliche Originaldetails und passende PBR-Bindings; dunkle Join-Fugen im 2x2-
  Originalfeld bleiben sichtbar. Kein globaler Terrain-Austausch.
  Bilder `/tmp/teramic142-grass/fiverr-grass-{front,side,top}.png`,
  Log `/tmp/teramic142-grass.log`.
- Seed 42042, Mountain Forest: Kamera 4300,155,310 -> 4352,110,256;
  `/tmp/teramic142-mountain-forest.png` und gleichnamiges Log.
- Seed 42042, Fantasy-Rand: 3750,195,-5265 -> 3840,123,-5375;
  `/tmp/teramic142-fantasy-ridge.png` und gleichnamiges Log.
- Seed 7, Fantasy-Mischung: 200,183,845 -> 256,120,768;
  `/tmp/teramic142-seed7-ridge.png` und gleichnamiges Log.

Die drei Terrainlaeufe verwenden je 6 Sekunden Messfenster nach Laden der
Welt, Sichtweite 12, den vollstaendigen aktuellen Snapshot aus dem Export und
zusaetzlich aktivierte Baeume/Hoehlen. Keine 10-/15-/30-Minuten-Tests und kein
neuer Nachweis einer allgemeinen FPS-Verbesserung. Alle nativen Laeufe Exit 0,
keine WARN-/ERROR-Meldungen in diesen Logs. Die Bilder wurden tatsaechlich angesehen.
Die niedrigeren Bergruecken sind sichtbar; teils weiterhin regelmaessige
Schneehaenge und grosse geschlossene Senken erreichen die komplexe
Fels-/Wasserfall-/Talgestaltung der Referenzen noch nicht 1:1.

`terra-server --no-save --smoke-test` ebenfalls erfolgreich headless ausgefuehrt;
Log `/tmp/teramic142-server-smoke.log`. Die no-save-Profilwahl ist zusaetzlich
mit aktuellen Defaults, expliziten alten Profilen und Revision 4 getestet.
