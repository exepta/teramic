# Feineres Gras und Main-Terrain

## Darstellung

Die neuen Atlanten sind tatsaechlich 1254x1254 Pixel gross, gegenueber
1024x1024 beim gelieferten Original. Entscheidender als diese Pixelerhoehung
ist die feinere neue Zeichnung: kleine unregelmaessige Grasplatten,
Erdklumpen und helle Steinchen statt des groben gleichmaessigen Rasters.
Die Dateien werden nicht lediglich hochskaliert.

Die [Albedo](../../../../assets/terra/blocks/textures/grass-reference-hd-v1.png)
und [Normalmap](../../../../assets/terra/blocks/textures/grass-reference-normal-v1.png)
wurden mit dem eingebauten ImageGen-Werkzeug erstellt. Vollstaendige Prompts,
Referenzrollen und die Abweichung vom angefragten 2048px-Format stehen in
[grass_reference_art.json](../../../assets/grass_reference_art.json).
Die Normalmap wird im Rust-Importer mit linearer XY-Staerke 0.45 normalisiert.
Die Oberflaeche ist matt (Roughness 0.88, Metallic 0).

Echtes Nahrelief bleibt erhalten: aktuell 20 Grasformen und 24 Steinformen,
440 zusaetzliche Dreiecke. Das vollstaendige Nahmodell hat 1208 Dreiecke und
2374 Vertices. Verdeckte Seiten entfernen ihre Details mit. Die Unterseite,
die aeusseren Join-Raender und die logische Blockzelle bleiben unveraendert.
Keine Detail-Entities, zusaetzlichen Materialslots oder Draw Calls pro Block.
Sechs grobe LOD-Seiten bleiben bei 512x512-PBR-Texturen und Greedy-Meshing.

Original-GLB und Original-PNGs bleiben unveraendert. Die aktiven Material-
Bindings verwenden den neuen Atlas; das erzeugte GLB enthaelt sieben
Materialien und 24 Bilder. Originaldatei Git-Blob:
`f048c4186320e95e406c693a4c15004079807cf8`.

## Main-Terrain

Neue Standardwelten verwenden wieder `main` / `origin/main` (`d2b7956`):
Landschaftsrevision 1, Meeresspiegel 8, kein spaeterer JSON-Biome-Snapshot.
Der Create-World-Dialog aktiviert wie in Main die Fluesse. Der allgemeine
WorldConfig-/Dedicated-Standard behaelt `rivers = false`.

Die spaeteren Terrainpfade werden nicht mehr automatisch fuer neue Welten
aktiviert. Sie bleiben ausschliesslich fuer explizite Konfigurationen und
gespeicherte Revisionen erhalten. Keine Umstellung vorhandener Welten,
keine geaenderten Welt-IDs, keine geloeschten Spieler-Edits. Damit entstehen
beim Nachladen gespeicherter Regionen keine neuen Terrain-Naehten.
Rendering-, Gras-, Bevy-0.20- und Bewegungsarbeit wurde nicht zurueckgesetzt.

Regressionen vergleichen die festen alten Hoehenanker, drei Seeds und
negative Sectiongrenzen. Storage-Pruefungen erhalten Revisionen 0..5,
Wasserspiegel, Flussoptionen, Profil-Snapshots und gespeicherte Edits.

## Native Sichtpruefung

RX 6950 XT / RADV / Vulkan, 1280x800, vorhandene Grafikoptionen.
Original links, aktueller Terrainblock rechts:
[Vorderseite](grass-front.png), [Schraegansicht](grass-side.png).
Beide Bilder wurden angesehen. Alle drei GPU-Pixelpruefungen und Captures
bestanden. Kein WARN/ERROR in `/tmp/teramic145-grass.log`.

Echte neue Welt ueber das Menue angelegt und geladen: 100%-Hold, Fade und
Welt-Audio-Reihenfolge bestanden, Exit 0, kein WARN/ERROR.
Das erzeugte Manifest unter `/tmp/teramic145-loading/worlds/` enthaelt
`landscape_revision = 1`, `sea_level = 8`, `rivers = true` und keinen
`biome_definitions`-Snapshot. Das war ein echter integrierter Client-/Server-
Durchlauf, nicht nur ein isolierter Generatoraufruf.

Keine pixelgenaue 1:1-Zusage: Referenzlicht, Umgebung und jedes einzelne
Steinchen wurden nicht identisch nachgebaut. Keine FPS-Vergleichsmessung
oder Langzeitprobe fuer diese Aenderung; die Sichtpruefung lief teilweise
waehrend des Cargo-Testaufbaus und ist kein Performance-Nachweis.

## Validierung

- Client und Dedicated Server gebaut.
- Import und `--check`: GLB/Provenienz bytegenau reproduziert.
- Beide nativen Kurzpruefungen erfolgreich.
- `cargo fmt --all --check`: erfolgreich.
- `cargo check --locked --workspace --all-targets -j1`: erfolgreich.
- `cargo test --locked --workspace --all-targets -j1 -- --test-threads=2`:
  2752 bestanden, 7 ignoriert, kein Fehler.
- `cargo clippy --locked --workspace --all-targets -j1 -- -D warnings`:
  erfolgreich.

Der erste Testlauf wies zu Recht eine ungueltige neue Test-Fixture ab:
ein pauschaler Hoehenversatz erzeugte ein Profil oberhalb der erlaubten
350 Bloecke. Der Test verwendet jetzt starke, aber gueltige Abweichungen
und validiert sie ausdruecklich vor dem Generatoraufruf. Keine gelockerte
Produktionsvalidierung und keine neu berechneten alten Hoehenanker.

Logs: `/tmp/teramic145-{tests-verified,check,clippy,fmt-final,build-final,
grass,loading}.log`.
