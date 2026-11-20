# Ecology 5: Naturgruppen und Felsformationen

Die Quelldateien sind unter `crates/terra-world/src/generation/natural_features/`
aufgeteilt. Algorithmus, Speicherschutz und Finder-Bedienung stehen in
`docs/natural-features.md`.

Validierung im isolierten `target/world-validation`, offline, zwei Buildjobs:

- `targeted-tests.txt`: drei gezielte Naturpatch-Regressionen erfolgreich.
- `world-tests.txt`: vollständige World-Bibliothek, **1445 bestanden, 0 Fehler,
  5 ignoriert**, 35,83 Sekunden.
- `clippy.txt`: `cargo clippy -p terra-world --features bevy --lib -- -D warnings`
  erfolgreich. Zwei zunächst gemeldete Stil-Lints wurden vor dem protokollierten
  Abschluss behoben; die Korrektur verändert keine generierten Werte.
- `rock-sites-default.jsonl`: zwei tatsächlich generierte Steinformationen mit
  unverändertem `WorldConfig::default()`, Seed42042, Ecology5, inklusive Kameras.
- `generation-measurement.jsonl`: tatsächliche Blood-Propanzahl und CPU-Zeiten
  für 144 Spalten, 181 Oberflächen-Sections und 36.864m².

| Messgröße | Ecology 4 | Ecology 5 |
| --- | ---: | ---: |
| Kleine Props | 5283 | 365 |
| Kleine Props je 1024m² | 146,75 | 10,14 |
| Große Props | 363 | 8 |
| Kalte Spaltenvorbereitung | 306,15ms | 319,10ms |
| Warme Spaltenvorbereitung | 241,09ms | 242,78ms |
| Aufbau der Oberflächen-Sections | 76,68ms | 73,04ms |

Ein einzelner lokaler CPU-Vergleich, kein allgemeiner Leistungsbenchmark und
keine GPU-/FPS-Messung. Die Szene liegt bei `[-6656,-8192]`; Bäume und natürliche
Habitatgrenzen bleiben aktiv. Native Sichtprüfungen führt der übergeordnete
Arbeitsablauf separat aus. Bestehende Nutzerspielstände wurden nicht geöffnet
oder verändert.
