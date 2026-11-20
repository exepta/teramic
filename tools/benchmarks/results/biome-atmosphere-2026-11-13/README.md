# Biomnebel und Grasfarben – 13.11.2026

Spirit Blossom verwendet nun einen tuerkisfarbenen Grasboden mit passenden
seitlichen Graskanten. Kirschwald-Grasoberseiten und Grashalme erhalten einen
hellen Lilaton. Die Faerbung blendet mit den Biomgewichten; die bisherigen
Grenzen fuer die anderen Biome bleiben erhalten. Die Minimap verwendet dieselbe
Farbregel und verwirft veraltete Farbraster ueber den Paletten-Fingerprint.

Der Nebel entsteht aus geladenen Spirit-Biomspalten und der aktuellen Bodenhoehe.
Laternen und Schreine loesen keine eigenen Nebelbaenke mehr aus. Hoechstens
121 Spalten werden alle 0,5 Sekunden abgefragt, ohne Chunk-Generierung im
Renderer. Ueberlappende Volumen bewegen ihre Dichte zeitabhaengig und blenden
an Biom- und Sichtbereichsgrenzen aus. Baugebietskonturen behalten ihre Dichte.

Die native Pruefung fand eine falsche Freigabe ueber `ClientSession`:
Lokale Welten ohne aktive Netzwerkverbindung zeigten keinen Biomnebel.
`WorldAccess::ready()` ist jetzt die gemeinsame Freigabe fuer lokale und
bereite Multiplayer-Welten; ausstehende Verbindungen verwerfen alte Baenke.
`native-first.log` dokumentiert diesen inzwischen korrigierten Zwischenstand.

## Spielaufnahmen

`native-run-final.txt` und `verification.json` bestaetigen sechs echte
1920x1080-Aufnahmen ohne Shaderfehler. Spirit zeigt 77 lokale Nebelbaenke
mit geaendertem Animationsdrift zwischen den Bildern; Kirschwald zeigt keine
Spirit-Nebelbaenke. Die Nahaufnahmen pruefen den tatsaechlichen geladenen Boden.
Die zwei Biomauswahlbilder wurden aus diesen Aufnahmen neu exportiert.

- [Spirit-Uebersicht](captures/picker-spirit-blossom-overview.png)
- [Spirit, spaetere Animationsphase](captures/picker-spirit-blossom-animated.png)
- [Tuerkiser Boden](captures/picker-spirit-blossom-close.png)
- [Kirschwald-Uebersicht](captures/picker-cherry-forest-overview.png)
- [Kirschwald-Boden im Baumschatten](captures/picker-cherry-forest-close.png)

Die Aufnahmen zeigen auch die bestehende Materialgrenze: Beim alten kombinierten
Kirschwald-Gras-/Erdatlas bleibt der schmale seitliche Grasrand gruen. Die
Oberseite und Grashalme sind umgefaerbt; Boden, Mikroschattierung und Wettereinfluss
wurden nicht durch eine pauschale Einfaerbung des ganzen Seitenmaterials veraendert.

Alle Laeufe verwenden temporaere Konfigurationen, Profile und Welten.
Die zehn Dateien von `worlds/Test` behalten ihre SHA-256-Pruefsummen.

## Pruefung

```sh
cargo check --workspace --all-targets --locked --offline
cargo fmt --all --check
cargo test -p terra-core -p terra-render -p terra-ui -p terra-client \
  --lib --bins --locked --offline --no-fail-fast -- \
  --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo build -p terra-client --locked --offline
cargo test -p terra-render --lib --locked --offline work_areas -- --test-threads=4
bash tools/benchmarks/run-biome-atmosphere.sh
```

Der Haupttestlauf besteht mit 2.519 Tests: 209 Client-, 537 Core-, 1.116 Renderer-
und 657 UI-Tests. Drei vorher vorhandene Tests bleiben ignoriert. Nur der
bekannte externe GLB-Test ist ausgefiltert, weil
`FIVERR/VoxelGrassBlock glb file.glb` im Repository fehlt.
Workspace-Check, striktes Clippy und der abschliessende Client-Build bestehen.
Nach der WorldAccess-Korrektur bestehen alle 15 gezielt erneut ausgefuehrten
Gebiets-/Nebeltests, einschliesslich der neuen lokalen/Replica-Regression.
Die abschliessende Formatpruefung und `git diff --check` bestehen ebenfalls.
