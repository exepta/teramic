# Voxel-Props, hellerer Blutwald und Naturgruppen – 14.11.2026

## Umsetzung

18 Propmodelle aus Blut- und Geistwald bestehen jetzt aus gestuften Mikrovoxeln
mit achsenparallelen Außenflächen. Verdeckte Flächen einschließlich Grenzen
zwischen unterschiedlichen Materialien werden entfernt, ebene Außenflächen
zusammengeführt. Die 24³-Zellen werden weder Laufzeit-Entities noch einzelne
Collider. IDs, Platzierungsflächen und die vorhandenen wenigen Collider bleiben
erhalten. Zusammen benötigen die Modelle 9.204 statt 23.193 Dreiecke, maximal
1.404 pro Modell. [Modellbericht](../../../assets/voxel_props/report.json),
[Generator und Grenzen](../../../assets/voxel_props/README.md).

Die Blutwald-Texturen haben hellere Rosen-/Karmin-, Rinden- und Mauvetöne.
Wasser, Gras, Karte und Atmosphäre passen dazu. BloodLeaves 406 erhalten keine
zusätzliche saisonale Blattfarbe mehr. Sonnenstärke, Ambientstärke, Belichtung,
Schattensystem und Darstellungsbudgets wurden nicht erhöht; der frühere kleine
Helligkeitsverlust durch die Blutwald-Ambientfarbe entfällt. Andere Biome behalten
ihre Farbregeln. Nächte und Baumschatten bleiben Teil der Beleuchtung.

Ecology5 verteilt kleine Pflanzen in lockeren Gruppen mit freien Bereichen und
große Einzelprops mit mindestens zwölf Metern Abstand zwischen ihren Ankern.
Neue Felsgruppen bestehen aus tatsächlichen abbaubaren Weltblöcken. Mehrere
versetzte Felskörper formen unregelmäßige, örtlich eingebettete Silhouetten.
Wasser, Höhleneingänge, Baumbelegung und erzeugte Bauflächen werden geprüft.
Algorithmus und Speicherschutz: [Naturgeneration](../../../../docs/natural-features.md).

## Validierung

| Prüfung | Protokoll |
| --- | --- |
| Workspace-Check mit allen Targets | [check-all-targets.txt](check-all-targets.txt) |
| Striktes Clippy aller Workspace-Targets | [clippy.txt](clippy.txt) |
| Client, Server, Gebäude-Editor bauen | [builds.txt](builds.txt) |
| Abschließender Client-Build und Clippy nach Korrektur des Foto-Harness | [build-native.txt](build-native.txt), [clippy-native.txt](clippy-native.txt) |
| Formatprüfung | [fmt.txt](fmt.txt) |
| Vollständige Workspace-Tests | 5.093 bestanden, 0 Fehler, 10 ignoriert, eine externe GLB-Fixture ausgefiltert; [tests.txt](tests.txt) |
| Isolierte vollständige World-Bibliothek | 1.445 bestanden, fünf vorhanden ignoriert; [world-tests.txt](natural-features/world-tests.txt) |
| Neue Voxelgeometrie | Fünf bestanden; [voxel-props-tests.txt](voxel-props-tests.txt) |
| Blutwald-Katalog und Modellgültigkeit | Drei bestanden; [blood-assets-tests.txt](blood-assets-tests.txt) |
| Bestehende Modellprüfungen | 13 bestanden; [existing-model-tests.txt](existing-model-tests.txt) |
| Reproduzierbare GLBs | [prop-assets-determinism.txt](prop-assets-determinism.txt) |
| Headless-Start mit Ecology5 | [headless.txt](headless.txt) |

Die Workspace-Suite verwendet echte lokale Netzwerkverbindungen:

```sh
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=4 cargo test --workspace --no-fail-fast -- \
  --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
```

Der ausgefilterte Test benötigt die nicht mitgelieferte Datei
`FIVERR/VoxelGrassBlock glb file.glb`. Vorhandene ignorierte Tests werden nicht als
bestanden gezählt. Die Naturtests prüfen Mindestabstände, echte freie Bereiche,
Habitat-Ausschlüsse, negative Section-Grenzen, Abbau/Edit-Überlagerung, Reload und
erhaltene alte Revisionsverträge. Physik, Navigation und Mining verwenden dieselben
Blockdaten; eine gesonderte native Spielfigur-Begehung wurde nicht aufgenommen.

## Messung und Darstellung

Die abschließende native Abnahme ist bestanden: neun kontrollierte PNGs zeigen
Blutwaldwasser und Laub, die hellere Biomauswahl, gestufte Blood-/Spirit-Props
sowie natürliche mehrzellige Felsformationen. Der Harness bestätigt Ecology5,
unverändertes Terrain und tatsächliche geladene Materialproben:
[native-final.txt](native-final.txt),
[verification.json](native/verification.json),
[Aufnahmen](native/captures/).

In der festen 36.864m²-Blutwald-Stichprobe sank die Zahl kleiner Props von 5.283 auf
365 (146,75 auf 10,14 je 1.024m²), die großer Props von 363 auf acht. Kalte
Spaltenvorbereitung brauchte 306 gegenüber 319ms, warme 241 gegenüber 243ms;
Aufbau von 181 Oberflächen-Sections 76,7 gegenüber 73,0ms. Das ist eine einzelne
lokale CPU-Messung ohne allgemeine Leistungs- oder FPS-Garantie.
[Rohdaten](natural-features/generation-measurement.jsonl).

Die Biomauswahl verwendet eine tatsächliche Aufnahme der helleren Baumkronen,
beschnitten und verkleinert. Originalaufnahme, Kameraprüfung und Export:
[preview-source](preview-source/), [preview-export.txt](preview-export.txt).

Der native Prüflauf fotografiert tatsächliche generierte Props und Felsblöcke,
ohne Weltblöcke einzusetzen. Der erste Versuch verwendete für die neuen
Prop-/Felsszenen versehentlich die alte Ecology0-Fotoeinstellung und wurde nicht
als Nachweis der neuen Verteilung übernommen. Der korrigierte Harness bewahrt
die Konfiguration und prüft Ecology5 sowie geladene Materialproben ausdrücklich.
Die Farb-/Modellaufnahme der Biomauswahl aus dem ersten Versuch war davon nicht
betroffen.

Reproduktion nach dem Client-Build auf einem Rechner mit GPU und Fenster:

```sh
bash tools/benchmarks/run-surface-ecology.sh
```

Texturen und Modelle aktualisieren die Darstellung vorhandener Welten. Neue
Propverteilung und Felsgruppen verwenden Ecology5 in neuen Welten; gespeicherte
Revisionen 0–4 behalten ihre Verteilung. Wire-Layout und Protokoll144 sind
unverändert. Ältere Programme ohne Unterstützung für Ecology5 lehnen diesen
Weltvertrag bei der Konfigurationsvalidierung ab.

Die Originalwelt `worlds/Test` blieb unverändert: Alle zehn Dateien besitzen
die anfangs erfassten SHA256-Prüfsummen, keine Datei kam hinzu.
[Prüfsummenvergleich](original-world-verification.json).
