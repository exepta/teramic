# Baumodus, Stadtlager, Truhen und Building Editor

Phase 241, Stand 13.11.2026. [Bedienung und Grenzen](../../../../docs/construction-mode-and-editor.md).

## Umsetzung

Die Ressourcenleiste in eigenen Bau-Chunks summiert echte, geladene und
berechtigte Stadtlagertruhen. Vorlagen zeigen genaue Items als Lagerbestand /
permanent benoetigte Menge. Doppelte Truhenhaelften werden korrekt bilanziert.
Die abgeloeste TAB-Baukamera bewegt die Spielfigur nicht. Clip-Distanz und
Nebel beruecksichtigen ihre Hoehe, ohne die normale Spielkamera zu veraendern; orange Vorschau,
vier Drehungen, echte Grundauswahl und NPC-Dialog verwenden bestaetigte Daten.
Handwerkerauftraege bleiben serverautoritaer. Creative-Finanzierung ist an den
Reichseigentuemer gebunden, erzeugt keine Inventarstapel und uebersteht Reload.

Der rekursive JSON-Katalog verwendet Unterordner als Kategorien. Berechtigte
Manager koennen ihn ohne Neustart neu laden; die HUD-Abfrage liest keine Dateien.
Bestehende aufgeloeste Jobs bleiben bei Katalogfehlern erhalten.
ChestMerge unterscheidet automatische, getrennte und gezielte Verbindungen.
Doppeltruhen blockieren neue Nachbarn nicht, es entstehen keine Dreiergruppen.

Die eigenstaendige App startet ohne Server oder Client-Hauptmenue auf einer
schwebenden 9x9-Bauhilfe. Gemeinsame Voxel-/Objektplatzierung, wirklicher
Spielinventarkatalog, JSON-Dateibrowser, validierter Export und bestaetigtes
atomares Ueberschreiben verwenden das vorhandene Schema 1.

## Automatisierte Pruefungen

4.941 Library-/Binary-/Integrationstests bestanden. Zehn bestehende Tests sind
ignoriert. Der Original-GLB-Test ist wegen der fehlenden externen Datei
`FIVERR/VoxelGrassBlock glb file.glb` ausgefiltert. 21 Editor-Tests pruefen
unter anderem das wirkliche Standardhaus, alle Teppichfarben, native
Holzvarianten, Tueren, Truhen, Dateinamen und stabile Ueberschreib-IDs.
629 Netzwerk- und 1.368 Welt-Tests decken echte Zwei-Client-UDP-Transaktionen,
Privateinhalte, Rollen, Auswahl/Vorschau, Live-Katalogreload, Materialverbrauch,
Creative-Bau, alle Ausrichtungen und echte Speicherroundtrips ab.

## Wiederholen

```sh
cargo fmt --all --check
cargo check --locked --offline --workspace --all-targets
cargo test --locked --offline --workspace --lib --bins --test headless --test textiles --test generated_sources --no-fail-fast -j 4 -- --test-threads=4 --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
cargo test --locked --offline --workspace --doc
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
cargo build --locked --offline -p terra-client
cargo build --locked --offline -p terra-server
cargo build --locked --offline -p terra-building-editor
bash tools/benchmarks/run-construction-mode.sh
bash tools/benchmarks/run-building-editor.sh
```

Die grafischen Runner brauchen eine native Grafiksession. Beide legen bei jedem
Lauf neue Wegwerfverzeichnisse unter `target/` an; vorhandene Spielwelten und
Profile bleiben unberuehrt. Die native Bauabnahme setzt reale Eingaben und
HTML-Handler gegen die UDP-Autoritaet ab. Die Bauabnahme fordert zusaetzlich einen bestaetigten asynchronen Save an und
liest `level.data` mit `terra_storage::load_edits` wieder ein: der Creative-Auftrag
und sein passendes Eigentuemer-Settlement muessen wirklich gespeichert sein.
Speicherfehler machen den Runner fehlerhaft. Die Editorabnahme setzt/reisst echte
Voxel, laedt eine vorhandene Hausdatei und liest die tatsaechlich exportierte
JSON-Datei wieder ein.

## Grenzen

Vorschauen zeigen orange Voxelbelegung; feine GLB-Formen sind darin vereinfacht.
Der Editor unterstuetzt aktuell das validierte Wohnhausvolumen von Schema 1.
Nicht darstellbare RGB-/Holz-/Schienensonderzustaende werden gemeldet.
Bodenvorbereitung und Hilfsgerueste kommen zum permanenten Vorlagenpreis hinzu.
Die Simulation arbeitet weiterhin nur in geladenen Bereichen; die Baukamera
ist keine globale Fernsimulation. Protokoll 141, Speicherindex ID26 mit
Migration der eingefrorenen ID24-/ID25-Staende. Keine Dependency-Upgrades.


Der erste Creative-Auftrag ohne vorher angelegtes Lager erzeugt seinen
Siedlungseintrag zusammen mit dem Auftrag. Die Registry bleibt damit sofort
speicherbar. Fehlerhafte Standorte oder ausgeschöpfte Limits legen weder
Auftrag noch Siedlung an. Regression und nativer Save-/Reload-Pfad prüfen das.


## Finale Abnahme

Format, Workspace-Check aller Targets, striktes Clippy, 4.941 Tests und die
Doctest-Ziele bestehen. Client, Server und Editor bauen jeweils getrennt.
Headless-Start ohne DISPLAY/Wayland endet erfolgreich nach 27 Sections; der
Server-Abhaengigkeitsbaum enthaelt keine Render-, Windowing- oder UI-Crates.
Beide nativen Laeufe bestehen mit insgesamt **12 Aufnahmen**.

Die Bauabnahme prueft den real gespeicherten Creative-Auftrag nach bestaetigtem
Save durch Wiedereinlesen des Weltindexes, einschliesslich Owner-Settlement
und weiterhin fehlendem Lager. Beim abschliessenden Shutdown tritt kein
Speicherfehler auf. Die Materialmengen bleiben sichtbar neben dem Scrollbalken.

| Ablauf | Aufnahme |
| --- | --- |
| Baukatalog, DE 1600x900 | [Katalog](construction-catalog-de-wide.png) |
| Privater NPC-Dialog | [NPC](construction-npc-de-wide.png) |
| Orange Vorlage auf echtem Terrain | [Blueprint](construction-blueprint-de-wide.png) |
| Bestaetigter Auftrag, EN 960x640 | [Auftrag](construction-queued-en-compact.png) |
| Leere 9x9-Editorplattform | [Plattform](editor-empty-platform.png) |
| Echte Platzierung und Abbau | [Setzen](editor-placed-block.png), [Abbauen](editor-removed-block.png) |
| Wirklicher Spielinventarkatalog | [Katalog](editor-real-inventory-catalog.png) |
| Geladene Hausvorlage | [Haus](editor-loaded-house.png) |
| Dateibrowser und Name | [Speichern](editor-save-browser.png) |
| Bestaetigtes Ueberschreiben | [Bestaetigung](editor-overwrite-confirmation.png) |
| Englische Optionen | [Optionen](editor-options-english.png) |

[Tests](tests.txt), [Format](fmt.txt), [Check](check.txt), [Clippy](clippy.txt),
[Client](client-build.txt), [Server](server-build.txt), [Editor](editor-build.txt),
[Doctest-Ziele](doc-tests.txt), [Headless](headless.txt),
[Server-Dependencies](server-features.txt), [Sprachen](localization.txt),
[Baumodus-Log](native-construction.txt), [Baumodus-Pruefdaten](native-construction.json),
[Editor-Log](native-editor.txt), [Editor-Pruefdaten](native-editor.json),
[tatsaechlich exportiertes JSON](exported-template.json),
[Gesamtpruefung](verification.json), [Quellpruefsummen](source-sha256.json).

Die Quellpruefsummen erfassen textbasierte Workspace-Quellen, Cargo-Dateien,
JSON-Assets, UI und Dokumentation. Generierte Laufdateien und alle historischen
Pruefordner sind ausgeschlossen. Der lokale TiltUI-Pfad bleibt dieselbe in der
Projekt-README dokumentierte externe Workspace-Voraussetzung; Cargo.lock pinnt
Registry-Versionen und nicht den uncommitteten Inhalt dieses Nachbarprojekts.
