# Schotterfundamente, Wagenbeladung und F58

Pruefung vom 11.11.2026, Rust 1.97.1, lokale Workspace-Abhaengigkeiten.

## Automatisierte Pruefung

```bash
cargo fmt --all --check
cargo check --locked --offline --workspace -j 4
cargo clippy --locked --offline --workspace --all-targets -j 4 -- -D warnings
cargo test --locked --offline --workspace --lib --bins --no-fail-fast -j 4 -- --test-threads=4 --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
```

Ergebnis nach den nativen Korrekturen: 4650 Tests bestanden, 0 fehlgeschlagen, 10 bestehende Tests ignoriert,
1 gezielt ausgefiltert. Formatierung, Workspace-Check und striktes Clippy bestanden.
Lokale UDP-Tests liefen mit erlaubtem Netzwerkzugriff.

Der ausgefilterte Test benoetigt die bereits fehlende Originaldatei
`FIVERR/VoxelGrassBlock glb file.glb`. Diese Asset-Abnahme ist weiterhin offen;
kein Ersatzasset und keine vorgetaeuschte erfolgreiche Abnahme wurden angelegt.

Abgedeckt sind unter anderem tiefe Schotterfundamente einschliesslich
Section-Grenzen, fehlender/exakter Vorrat und atomarer Abbruch; Beladung,
Item-Metadaten, Wagenzuordnung, Ueberlauf und Creative; Arbeitsgebiete,
Reservierungen, reale Laufwege, Werkzeug-/Saatgutentnahme, Abbau/Ernte,
Anlieferung, volle Lager, fehlende Rechte/Wege, Pause/Nachtruhe und Save/Reload.
Ein UDP-Test prueft mit zwei Clients die Auftragsreplikation und die Ablehnung
fremder Aenderungsversuche.

Der erste Gesamtlauf fand eine ungueltige neue Migrations-Testlok: aktivierter
Gleisleger ohne angebautes Modul. Nach Korrektur der Testvorlage bestanden auch
alle 174 Storage-Tests. Produktionsvalidierung wurde nicht abgeschwaecht.

Die erste native Pruefung deckte ausserdem die fehlende Behandlung frisch
gesetzter Truhen ohne ContainerRecord und nicht greifende Parent-Selektoren in
der neuen TiltUI-Unterkomponente auf. Der Lagerpfad verwendet jetzt denselben
Leerinventar-Fallback wie der Spielerpfad; zwei Regressionstests pruefen reale
Anlieferung und Erhalt vorhandener Inhalte. Eigene Styles der Unterkomponente
ersetzen die unbeabsichtigten Standardfarben und Mindestbreiten.

## Binaries Und Native Pruefung

`cargo build --locked --offline -j 4 -p terra-client -p terra-server` bestanden.
Der Dedicated-Server-Smoke-Test lief ohne DISPLAY und WAYLAND_DISPLAY mit
27 geladenen Sections erfolgreich bis zum geordneten Ende. Die Doctest-Phase
bestand ebenfalls; im Workspace sind derzeit keine ausfuehrbaren Doctests.

Der native Clienttest verwendete einen echten integrierten Server, einen per
Befehl gespawnten NPC und per bestaetigter Blockaktion gesetzte Truhe. Zuweisung,
Pause und Wiederaufnahme wurden anhand der Serverantworten geprueft. Eine neue
leere Truhe wird vom Sammler akzeptiert; der Status ist korrekt "Keine Ziele",
solange keine einzusammelnden Drops vorliegen.

Aufnahmen und Bounds-Pruefungen bei 1680 x 936, 1024 x 768 und 3840 x 2160
bestanden. Alle drei Aufnahmen wurden visuell geprueft; Inputs, Beschriftungen,
Speichern und Aktivierung bleiben innerhalb des Dialogs. Reproduzierbare
Testwelt, Logs und alle Aufnahmen liegen unter `target/f58-native/`;
eine [kompakte Aufnahme mit pausiertem Auftrag](npc-work-compact-paused.png)
ist hier beigefuegt. Die normale Spielwelt wurde nicht veraendert.

Die Aufnahme prueft den echten Client-/Server-Ablauf und das Layout. Die
Arbeitsaktionen selbst (Abbau, Ernte, Einsammeln und Anlieferung) werden in den
zwoelf Welt-Regressionsfaellen ueber reale Voxeldaten und Inventare geprueft.
Es gab keine grafische Fahrpruefung des Zuges; dafuer liefen die autoritativen
Bau-/Beladungstests einschliesslich exaktem Materialverbrauch.

Bedienung und konkrete Grenzen: [NPC-Arbeit](../../../../docs/npc-work.md),
[Gleisleger](../../../../docs/train-excavation.md).
