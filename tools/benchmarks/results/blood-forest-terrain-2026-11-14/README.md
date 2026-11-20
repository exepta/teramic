# Gelände, Grabdungeons, Licht und Blood Forest – 14.11.2026

Implementierungsumfang und Grenzen:

- [Geländeangepasste Strukturen, Revision 10](../../../../docs/structure-terrain-revision-10.md)
- [Blood Forest, Kreaturen, Gegenstände und Bedienung](../../../../docs/blood-forest.md)
- [Blutwasser und animierte Tropfen](../../../../docs/blood-forest-rendering.md)
- [Modellreparaturen und Licht-Streaming](../../../../docs/block-graphics-and-light-streaming.md)

Neue Welten verwenden Strukturrevision 10 und Blutwaldrevision 1. Bestehende
Weltverträge behalten ihre Generatoren. Protokoll 144 verlangt passende Client-
und Serverstände. TRMAP005 liest die alten Kartenformate weiter. Es wurden keine
Dependency-Versionen geändert.

## Automatisierte Prüfung

| Prüfung | Ergebnis / Protokoll |
| --- | --- |
| `cargo fmt --all --check` | Erfolgreich, [fmt-complete.txt](fmt-complete.txt) |
| `cargo check --workspace` | Erfolgreich, [check-complete.txt](check-complete.txt) |
| `cargo clippy --workspace --all-targets -- -D warnings` | Erfolgreich, [clippy-complete.txt](clippy-complete.txt) |
| `cargo build -p terra-client -p terra-server -p terra-building-editor` | Alle drei Anwendungen erfolgreich, [builds-complete.txt](builds-complete.txt) |
| Vollständige Workspace-Tests | 5.089 bestanden, 0 Fehler, 10 ignoriert, 1 ausgefiltert; [tests-complete.txt](tests-complete.txt) |
| Zusätzliche vollständige World-Library-Prüfung nach Bodenübergangskorrektur | 1.442 bestanden, fünf vorhandene ignoriert, [tests-world-validation-final.txt](tests-world-validation-final.txt) |
| Bestehende Modell-/Geometrieprüfung | 13 bestanden, [existing-model-tests.txt](existing-model-tests.txt) |
| Neue Blutwald-Assets | Drei bestanden, [assets-tests-final.txt](assets-tests-final.txt) |
| Determinismus der finalen Blatt-/Rindentexturen | Byte-identisch und periodisch, [assets-texture-audit.txt](assets-texture-audit.txt) |
| Deutsch/Englisch | 3.700 Meldungen, 310 parametrisierte Muster, keine offenen Übersetzungen, [localization.txt](localization.txt) |

Die Workspace-Suite verwendet echte lokale Netzwerkverbindungen und läuft deshalb
außerhalb der Netzwerk-Sandbox. Ihr Aufruf lautet:

```sh
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=4 cargo test --workspace --no-fail-fast -- \
  --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
```

Der einzige ausdrücklich ausgefilterte Test benötigt die nicht mitgelieferte Datei
`FIVERR/VoxelGrassBlock glb file.glb`. Diese Datei wurde weder ersetzt noch
erfunden. Die zehn bereits ignorierten Workspace-Tests werden nicht als bestanden
gezählt. Frühere Diagnoseprotokolle bleiben erhalten; `*-complete.txt` bezeichnet
die abschließenden Codeprüfungen.

Die World-Tests prüfen unter anderem unveränderte ältere Generatorreferenzen,
negative Koordinaten, weiche Blutwald-Bodenübergänge einschließlich Sand/Schnee,
freie Naturhöhlen, unterschiedliche Raumpläne, tatsächliche Treppenkollisionen,
gefüllte Truhen und das erhaltene Gestein zwischen den Dungeonräumen.
Kreaturentests prüfen Navigation, Sichtlinien, Angriffspausen, Spawnidentitäten,
gespeicherte Todeszustände und Beute über die autoritative Simulation.

## Native Abnahme

Alle Szenen verwenden echte generierte Weltblöcke und isolierte Wegwerfverzeichnisse
unter `target/`. Die Kreaturengalerien stellen die Produktionsmodelle gezielt für
die Bildkontrolle auf; sie ersetzen keine Tests des natürlichen Spawnverhaltens.

| Lauf | Nachweis |
| --- | --- |
| Blutwald und Modellgalerien | Elf PNGs, [blood/verification.json](blood/verification.json), [native-blood-polished.txt](native-blood-polished.txt) |
| Friedhof und drei Grabtypen | Acht PNGs, echte geladene Blöcke/Truhen, [graves/verification.json](graves/verification.json), [native-graves.txt](native-graves.txt) |
| Königreich und Außenposten | Vier PNGs, echte belegte Betten und Terrain, [settlements/verification.json](settlements/verification.json), [native-settlements.txt](native-settlements.txt) |
| Eigenständiger Gebäude-Editor | Zehn PNGs, echte Blockaktionen, JSON-Import/Export, Überschreiben, Sprache und Plattformvergrößerung, [editor/verification.json](editor/verification.json), [native-editor.txt](native-editor.txt) |
| Dedicated Server | Fensterloser Start, echtes lokales UDP und geordnetes Ende, [native-server.txt](native-server.txt) |

Die PNGs liegen in den jeweiligen Unterordnern. Sichtgeprüft wurden insbesondere
die natürlichen Geländeanschlüsse, Krypten, das rote Wasser, die Laubunterseiten
mit Tropfen sowie Skelett-/Blutwald- und Geistkreaturen. Die finale Baumtextur hat
überlappende Burgunderblätter und lesbare rotbraune Rinde. Die neue Biomauswahl
verwendet die wirkliche Blutwaldaufnahme, lediglich beschnitten und verkleinert:
[Exportprotokoll](preview-export-final.txt).

Die Tropfenprüfung beobachtet über mehrere Sekunden echte Bewegung und
Wachstum/Fall/Auftreffen. Der Pool bleibt bei höchstens 192 Instanzen und die
Suche bei höchstens 32 geladenen Abschnitten pro Durchlauf. Wasser und Tropfen
wurden ohne nachträgliche Weltänderungen gefunden; die Kameraberichte prüfen
geladene Luft und freie Sichtlinien auf tatsächliches Wasser.

Für die Lichtprüfung sind Beobachterframes dokumentiert, keine gemessenen
GPU-Präsentationszeitpunkte: Ferne Lichtmeshes wurden in allen vier Grabszenen
ab Frame 3 nach geladenen Lampen ab Frame 2 registriert. Lokale PBR-Lichter
erschienen beim Erstladen erst zwischen Frame 125 und 145; nach dem Wechsel auf
die unterste Kryptaetage nach ein bis fünf Beobachterframes. Das ist kein Nachweis
für sofortige vollständige Beleuchtung beim Start und kein allgemeiner
FPS-Vergleich. Details und Quellprüfsummen:
[light-streaming-summary.json](light-streaming-summary.json).

## Reproduktion und Grenzen

Client zuerst bauen, dann auf einem Rechner mit Grafikfenster/GPU ausführen:

```sh
bash tools/benchmarks/run-blood-forest.sh
bash tools/benchmarks/run-grave-sites.sh
bash tools/benchmarks/run-terrain-settlements.sh
bash tools/benchmarks/run-building-editor.sh
```

Der Siedlungs-Harness schaltet für die weit entfernte Überblickskamera den
Distanznebel aus; normale Spielfog-Einstellungen bleiben erhalten. Dunkelrotes
Wasser bewegt sich optisch langsamer, erhält aber keine neue Viskositätsphysik.
Die Modelle übernehmen Formen und Farbgruppen der Vorlagen im facettierten
Spielstil. Es gibt keine neu implementierten Dungeonquests, Rätsel, Bosswellen,
Fernkampfangriffe oder ausziehbare Skelettausrüstung.

Die Originalwelt `worlds/Test` wurde nicht bearbeitet: Alle 13 vorhandenen Dateien
haben die ursprünglichen SHA256-Prüfsummen, keine Datei kam hinzu.
[Prüfsummenvergleich](original-world-verification.json).
