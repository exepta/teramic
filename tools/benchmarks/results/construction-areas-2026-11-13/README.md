# Phase 243: Bauvorlagen und animierte Gebiete

**Abnahme bestanden:** 4.958 Tests, Formatierung, Workspace-Check aller Targets,
striktes Clippy, Doctest-Ziele, getrennte Client-/Server-Builds und echter
Headless-Start mit 27 Sections. Der Server-Abhängigkeitsbaum enthält weder
Rendering noch UI. Zehn bestehende Tests sind ignoriert, eine fehlende externe
Original-GLB-Fixture ist ausgefiltert. Sechs native Aufnahmen und die echte
Platzierungs-/Speicherstrecke bestehen; keine Save- oder Runtime-Fehler im Log.

- [Gebiete sichtbar](construction-areas-de-wide.png), [mit H verborgen](construction-areas-hidden-de-wide.png)
- [Orange Bauvorlage](construction-blueprint-de-wide.png), [kompakte englische Ansicht](construction-queued-en-compact.png)
- [Katalog](construction-catalog-de-wide.png), [Bewohnerdialog](construction-npc-de-wide.png)
- [Native Prüfwerte](native.json), [Client-Log](native.txt), [Runner](native-runner.txt)
- [Validierung](validation.json), [Tests](tests.txt), [Check](check.txt), [Clippy](clippy.txt)
- [Client-Build](client-build.txt), [Server-Build](server-build.txt), [Headless-Start](headless.txt), [Server-Dependencies](server-tree.txt)
- [Quellstand](source-sha256.json), [Doctests](doctests.txt), [Lokalisierung](localization.txt), [Format](fmt.txt)

Bei aktiver Vorlage trifft der Bauklick den Boden statt den NPC-Dialog. Ohne aktive
Vorlage bleiben Bewohner anklickbar. NPCs, Tiere und Gras blockieren die Planung
nicht. Freigeräumte Luft und eine natürliche Geländestufe sind erlaubt; die Stufe
wird durch gespeicherte, tatsächlich ausgeführte Vorbereitungsschritte entfernt.
Bäume, Gebäude, gesetzte Blöcke, Wasser, unbekannte Sections und fehlende Rechte
bleiben geschützt. Körper in der späteren Baustelle pausieren den wirklichen Bau.

TAB zeigt die bestätigten eigenen Arbeits-, Lager-, Wohngebiete und Baupositionen
automatisch. Der äußere Build-Chunk-Rand erscheint rot, gemeinsame innere Kanten
entfallen. H blendet alle Konturen um; Dialoge und F1-H werden ausgenommen. Die
manuelle Gebietsauswahl bleibt erhalten. F2 und Disconnect verstecken die Anzeige.

Fünf stabile Mesh-/Materialgruppen erzeugen bodennahe, weich auslaufende Bänder
mit pulsierender Transparenz. Änderungen werden höchstens viermal pro Sekunde
aus geladenen Voxeln übernommen, mit begrenzten Gebiets-/Segmentzahlen; die
Animation erzeugt keine neuen Entities oder Meshes. Unbekannte Bodenabschnitte
und größere Klippen werden ausgespart. Es gibt keine vertikalen Drahtkästen.

Sechs neue Weltregressionen prüfen alle vier Ausrichtungen, NPCs/Tiere/Gras,
explizite Luft, natürliche Materialien, geschützte Hindernisse, echte Wartezeiten
bei Körperkollision ohne Materialverlust und einen tatsächlich fertig gebauten
Hausauftrag auf Lehmterrassen. Eine eigene JSON-Vorlage mit Fußboden auf relativer
Höhe 0 prüft zusätzlich deduplizierten Aushub, Save-Roundtrip und reale Platzierung.
Renderer- und UI-Tests prüfen negative Koordinaten, gemeinsame Chunk-Kanten,
weiche Bodenstreifen, unbekanntes/geändertes Terrain, stabile animierte Objekte,
H/F2/Disconnect, erhaltene manuelle Auswahl und unverdrängte Serverantworten.

Der native Runner verwendet eine isolierte lokale Welt, echte Tastatur-/Maus-
Ereignisse, HTML-Steuerungen und UDP-Serverantworten. Er prüft die ausgewählte
Fichtenhütte, deren orange Vorschau und Auftrag, NPC-Inventar, Kamera-Rückkehr,
bestätigtes Speichern mit Disk-Reload sowie reale Lager-/Wohngebiete. H muss die
wirklichen Mesh-Entities verstecken und wiederherstellen; die Animation muss
ihren Wert ändern. Zusätzlich werden deutsche und englische UI-Grenzen geprüft.

Wiederholen:

```sh
cargo fmt --all --check
cargo check --locked --offline --workspace --all-targets
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
cargo test --locked --offline --workspace --lib --bins --test headless --test textiles --test generated_sources --no-fail-fast -- --test-threads=4 --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
cargo build --locked --offline -p terra-client
cargo build --locked --offline -p terra-server
bash tools/benchmarks/run-construction-mode.sh
env -u DISPLAY -u WAYLAND_DISPLAY target/debug/terra-server --smoke-test --no-save
```

Die ausgefilterte Original-GLB-Fixture `FIVERR/VoxelGrassBlock glb file.glb`
fehlt bereits im Repository. Bestehende ignorierte Tests bleiben unverändert.
Netzwerktests benötigen lokale Sockets; die native Abnahme benötigt eine echte
Grafiksession. Abhängigkeiten, Protokoll 141, Speicherindex ID26 und JSON-Schema 1
bleiben unverändert.
