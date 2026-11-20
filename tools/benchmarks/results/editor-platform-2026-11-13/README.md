# Phase 242: geschützte Struct-Bauhilfe

Die Bauhilfe verwendet `MaterialId::StructureStone` (ID43) auf Y−3, unter dem echten Fundament auf Y−2. Sie ist kein Dokumentinhalt. Abbau, Ersetzen und seitliches Erweitern werden vor jeder Transaktion abgewiesen. Bereits echte Fundamentblöcke bleiben erhalten.

ESC → Optionen → Größe der Bauhilfe bietet 9×9, 17×17, 25×25 und 33×33. Die Einstellung gilt für die laufende Sitzung, bleibt bei Laden/Neuanlegen erhalten und ändert weder Kamera noch Dirty-Status oder Dateipfad. Die Schema-1-Vorlagengrenzen bleiben unverändert.

24 Editor-Tests, Workspace-Check aller Targets, striktes Workspace-Clippy, Formatierung und eigenständiger Editor-Build sind erfolgreich. Regressionen prüfen echte Platzierungsabweisung einschließlich atomarer Mehrfach-Edits, alle Größen und ungültige Eingaben, verschobene Anker, Import/Neuanlegen und exakten JSON-/Material-Roundtrip.

Der native Runner aktiviert tatsächlich das Dropdown und dessen Option über TiltUIs `ControlActivated`. Er prüft den angezeigten Wert **17 × 17**, neue physische Randblöcke, unveränderten Dokumentzustand sowie die identische JSON-Datei nach erneutem bestätigtem Überschreiben. Der Lauf erzeugt zehn Aufnahmen; die beiden neuen Ansichten sind hier archiviert.

- [Größeneinstellung](editor-expanded-platform-options.png)
- [Vergrößerte Bauhilfe](editor-expanded-platform.png)
- [Native Prüfwerte](native.json), [Log](native.txt), [exportiertes Modell](exported-template.json)
- [24 Tests](editor-tests.txt), [Check](check.txt), [Clippy](clippy.txt), [Build](editor-build.txt), [Format](fmt.txt)

Wiederholen:

```sh
cargo test --locked --offline -p terra-building-editor
cargo check --locked --offline --workspace --all-targets
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo build --locked --offline -p terra-building-editor
bash tools/benchmarks/run-building-editor.sh
```

Keine Abhängigkeits-, Protokoll- oder Schemaänderung. Der native Runner benötigt eine echte Grafiksession und verwendet eine neue Wegwerfmappe unter `target/building-editor/`.
