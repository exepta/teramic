# Biome-Profile, Nadelbaeume Und Shader-Wolken

## Weltvertrag

Neue Welten verwenden Landschaftsrevision 4 und standardmaessig Meeresspiegel
Y22. Die acht JSON-Dateien in `assets/biome/` werden einmal geladen, validiert
und als unveraenderlicher Snapshot in der Welt gespeichert. Parameter und
Hoehenbereiche sind in [assets/biome/README.md](../../../../assets/biome/README.md)
beschrieben. Gespeicherte alte Revisionen, Meeresspiegel und fehlende alte
Snapshots bleiben erhalten, auch beim Dedicated-Server-Neustart.

Der gemeinsame WorldConfig-Snapshot wird an Clients uebertragen. Deshalb ist
die Protokollversion 76 erforderlich. Der UDP-Test prueft neue konfigurierte
Profile und alte Welten ohne Snapshot, einschliesslich spaet beitretender Clients.

Das Terrain verwendet weiterhin die vorhandenen Klima-, Landform-, Fluss- und
Seenfelder. Begrenzte Hoehenprofile ersetzen das unbeschraenkte Aufaddieren
von Bergen. Weiche Profiluebergaenge bleiben erhalten; Fluss- und Seeerosion
kann Terrain unter die Untergrenze eines reinen Landbioms absenken.
Meeresboden behaelt nach der gesamten Hoehlen-CSG eine sechs Meter starke
geschlossene Dachschicht. Tiefere Hoehlen bleiben moeglich.

Warme, kalte und sumpfige Ocean-Lebensraeume verwenden unterschiedliche,
konfigurierbare Boden- und Vegetationsgewichte. Clay liegt in verbundenen
Clustern statt unabhaengigen Einzelvoxeln. Suesswasser bleibt davon getrennt.

## Nadelbaeume

Fichten sind dunkler; neue Tannen wachsen in Forest und Highlands ab Revision 4.
Sie verwenden die vorhandenen Stamm-/Laub-Block-IDs und eigene Kronenformen.
Gebundelte, gecachte Nadelgeometrie hat hoechstens 360 nahe beziehungsweise 216
mittlere Vertices je exponierter Laubzelle; der Cache bleibt auf 1024 Vorlagen
begrenzt. Es entstehen keine einzelnen Entities pro Nadel.

## Wolken

Die Wolken sind ein Shader-Volumen zwischen Y200 und Y312 mit einem festen
24-Vertex-/12-Dreieck-Proxy, einem Material und einer 64-cubed-RG8-Textur.
Low/Medium/High/Ultra integrieren maximal 10/14/20/28 Raymarch-Schritte.
Wetter, Wind, Beleuchtung und Dichte aendern Uniforms statt Geometrie.
Die alten Sunshafts sind entfernt; ihre Kompatibilitaetszaehler bleiben null.

Terrain-Prepass-Tiefe begrenzt die Integration. Der erste Wolkentreffer bestimmt
die physische Fragmenttiefe. Der Himmel wird mit einem konstanten Hintergrund-
Sortierschluessel zuerst gezeichnet: Bei kurzer Sichtweite konnte zuvor die
Sonne nachtraeglich auf die Wolken gezeichnet werden. Regressionen pruefen die
echten Bevy-Sortierschluessel und einen stabilen Material-Pipelinekey.

## Gemessener CPU-Vergleich

Ryzen 9 5900X, optimierter Dev-Build; kein gleichzeitiger GPU-Test oder Cargo-Lauf.
Seed 42042, dieselben positiven und negativen Koordinaten, sieben warme Runden.
Revisionen 3 und 4 unterscheiden sich in der Terrainform, nicht nur im Aufwand.

```bash
cargo run --locked -p terra-world --example terrain_regions -j2 -- \
  --seed=42042 --revision=4 --benchmark
```

| Operation | Abfragen | Revision 3 gesamt | Revision 4 gesamt | Differenz |
| --- | ---: | ---: | ---: | ---: |
| surface_height | 458752 | 585.635 ms | 487.736 ms | -16.72 % |
| surface_column | 1792 | 11.683 ms | 11.380 ms | -2.59 % |

Die Spaltenmessung trifft einen vorgewaermten Cache. Das ist weder eine Messung
kalter Chunk-Generierung noch ein Nachweis verbesserter In-Game-AVG/Min-FPS.
Rohdaten stehen in `terrain-benchmark.log`.

## Validierung

- `cargo fmt --all --check`: bestanden.
- `git diff --check`: bestanden.
- `cargo check --locked --workspace --all-targets -j2`: bestanden.
- `cargo test --locked --workspace --lib --bins --no-fail-fast -j2 -- --test-threads=2`:
  2651 bestanden, 7 bestehende Tests ignoriert, keine Fehler.
- `cargo test --locked --workspace --doc -j2`: bestanden.
- `cargo clippy --locked --workspace --all-targets -j2 -- -D warnings`: bestanden.
- `cargo build --locked --workspace -j2`: Client und Dedicated Server bestanden.

Der native Wetter-Smoke lief etwa 135 Sekunden auf RX 6950 XT / Vulkan / Mesa
26.2.5 durch, mit Seed 42042, Sichtweite 3, High-Wetter/-Wasser und 1920x1200.
Er pruefte Regen, Schnee, wechselnde Wolkendichte, Sonne/Mond sowie Ansichten
unterhalb, innerhalb und oberhalb der Wolkenschicht. Keine Renderfehler;
ein Wolken-Batch und ein initialer Wolken-Mesh-Upload ueber den gesamten Lauf.
Bei Himmelsaufnahmen pruefen zusaetzliche Strahlen, dass kein Vordergrundterrain
die Messpunkte verdeckt. Klarer Himmel und dichte Bewoelkung wurden auch manuell
als Screenshots verglichen; die Sonne wird hinter der Wolkendecke abgeschwaecht.

Diese kurzen Wetterfenster sind keine normalen Gameplay-FPS-Benchmarks.
Ein Forest-Smoke lief ebenfalls durch, seine Aufnahme zeigt jedoch keine
eindeutig identifizierbare Tanne; sie gilt nicht als visuelle Tannenvalidierung.
Nadelgeometrie, Artenverteilung, Materialfarben, LOD, Cachegrenzen sowie alte
Baumselektionen sind durch Unit-Tests geprueft. Es wurde kein Langzeittest
durchgefuehrt. Hoehen-/Determinismusregressionen verwenden mehrere Seeds.
