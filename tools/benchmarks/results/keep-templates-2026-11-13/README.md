# Bergfried und Bauvorlagen

Phase 240, Abnahme vom 13.11.2026.
[Bedienung und Grenzen](../../../../docs/keep-and-building-templates.md).

## Umfang

Sechs eigene UI-Komponenten zeigen Allgemein, Mitglieder, Diplomatie, Gebaeude,
Arbeitskraefte und Armee im bestehenden braun-goldenen Stil mit gruenen
Aktionsakzenten. Listen und zugehoerige Detailansichten verwenden wirkliche
Mitglieder und Beziehungen. Armeezahlen umfassen nur geladene lebende eigene
Soldaten; keine erfundene globale Kriegssimulation.

Der autoritative JSON-Katalog enthaelt Dorfwohnhaus und Fichtenhuette.
Die ausgewaehlte Vorlage zeigt Abmessungen, aus echten Zellen erzeugte
SVG-Planskizze sowie genaue permanente Materialmengen mit Item-Symbolen.
Manuelles Bauen und automatische Wohngebiete verwenden dieselbe explizite ID.
Bodenvorbereitung und bezahlte Gerueste ergaenzen den standortabhaengigen Auftrag.
ID24 migriert zu ID25 beim naechsten Speichern; Protokoll 140 ist erforderlich.

## Automatisierte Pruefungen

**4.874 Tests bestanden: 4.870 Library-/Binary-Tests und vier Integrationstests.**
Zehn bestehende Tests bleiben ignoriert. Nur der bekannte Originaltest
`original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images`
ist wegen der fehlenden externen Datei `FIVERR/VoxelGrassBlock glb file.glb`
ausgefiltert. Die Doctest-Ziele bestehen (keine ausfuehrbaren Beispiele).
Striktes Workspace-Clippy besteht mit allen Targets und `-D warnings`.


Die physische Welt-Suite baut beide Vorlagen in allen vier Ausrichtungen.
Default: 390 permanente Schritte / 392 Zellen / 17 Materialarten;
Fichtenhuette: 388 / 390 / 16. Die vollstaendigen Baustellen enthalten jeweils
832 beziehungsweise 830 Schritte einschliesslich bezahlter Hilfskonstruktionen.
Materialtransport, Kosten und Rueckgabe von Geruestholz werden bilanziert.
Ein bereits aufgeloester Auftrag wird auch nach Entfernen der Quelldatei fertig.

Ein echter Zwei-Client-UDP-Test prueft serverseitige Katalogdaten, exakte
Vorschaumengen, gewaehlte manuelle/automatische Vorlagen, spaeten Beitritt und
Abweisung manipulierter IDs. UI-Tests pruefen reine lokale Vorschau ohne Auftrag,
fortbestehende gespeicherte Auswahl und Stoppen einer entfernten Auto-Vorlage.
Migrationstests verwenden einen tatsaechlich eingefrorenen ID24-Binaerwert.

## Build- und Testbefehle

```sh
cargo fmt --all --check
cargo check --locked --offline --workspace
cargo test --locked --offline --workspace --lib --bins --test headless --test textiles --test generated_sources --no-fail-fast -j 4 -- --test-threads=4 --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
cargo test --locked --offline --workspace --doc
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
cargo build --locked --offline -p terra-client
cargo build --locked --offline -p terra-server
env -u DISPLAY -u WAYLAND_DISPLAY target/debug/terra-server --smoke-test --no-save
```

## Native Pruefungen wiederholen

Client zuerst bauen. Die Runner verwenden wegwerfbare Profile, Konfigurationen
und Welten unter `target/` und veraendern keine vorhandene Spielwelt.

```sh
cargo build --locked --offline -p terra-client
bash tools/benchmarks/run-keep-redesign.sh
bash tools/benchmarks/run-building-local.sh
```

Die neue Galerie oeffnet alle sechs echten Tabs und waehlt die Fichtenhuette
ueber den wirklichen HTML-Kartenhandler. Ein zweiter UDP-Client tritt wirklich
bei. Die Galerie kontrolliert serverseitige Materialmengen und sichtbare
Button-/Rahmengrenzen bei Deutsch 1600x1000 und Englisch 1024x768.
Die Eingaben erfolgen ueber Gameplay-/HTML-Handler, keine OS-Mausautomation.

Der getrennte Bauablauf prueft reale Truhe, Materialtransfer, Lagerwahl,
Build-Claim, Hausauftrag, Handwerker, eine echte bezahlte Nicht-Air-Platzierung
und Pause. Er verwendet absichtlich `fly_speed=4.0`; der normale Standard bleibt
20.0. Vollstaendige 30/15/10/5-Minuten-Mannschaftszeiten werden durch die CPU-Suite
geprueft und nicht im kurzen Grafiklauf ausgesessen.

Die visuelle Editor-App bleibt F82. Benutzerdefinierte Vorlagen muessen die
vorhandenen Wohnhaus-, Platzierungs- und Navigationsgrenzen einhalten.
Die Simulation arbeitet weiterhin nur in geladenen Weltbereichen.


## Finale Ergebnisse und Aufnahmen

Alle Pruefungen bestehen: Formatierung, Workspace-Check, Clippy aller Targets,
getrennte Client-/Server-Builds, Doctest-Ziele und Headless-Start mit 27 Sections.
Der Server-Dependency-Baum enthaelt keine Windowing-, GPU-, Render- oder UI-Crates.
Die beiden nativen Laeufe bestehen mit insgesamt **18 Aufnahmen**.

| Bereich | Deutsch 1600x1000 | Englisch 1024x768 |
| --- | --- | --- |
| Allgemein | [Aufnahme](keep-general-de-wide.png) | [Aufnahme](keep-general-en-compact.png) |
| Mitglieder | [Aufnahme](keep-members-de-wide.png) | [Aufnahme](keep-members-en-compact.png) |
| Diplomatie | [Aufnahme](keep-diplomacy-de-wide.png) | [Aufnahme](keep-diplomacy-en-compact.png) |
| Gebaeude und Materialliste | [Aufnahme](keep-buildings-de-wide.png) | [Aufnahme](keep-buildings-en-compact.png) |
| Arbeitskraefte | [Aufnahme](keep-workers-de-wide.png) | [Aufnahme](keep-workers-en-compact.png) |
| Armee | [Aufnahme](keep-army-de-wide.png) | [Aufnahme](keep-army-en-compact.png) |

Die Galerie wechselt tatsaechlich zum zweiten Mitglied, zum zweiten Reich und
zur Fichtenhuette. Vor der Aufnahme wartet sie den DOM-/Style-Zyklus ab und
prueft die wirklichen `ElementClasses`: genau die ausgewaehlte Zeile muss
`selected` tragen. Bildschirmstichproben bestaetigen auch die sichtbare Farbe.

Der Bauablauf verwendet Deutsch 1600x900 und Englisch 960x640:
[Bau-Tab](building-local-de-wide.png), [Lagerauswahl](building-storage-selection-de-wide.png),
[gespeicherte Weltbereiche](building-saved-areas-de-wide.png),
[Auftrag](building-job-en-compact.png), [echte Platzierung](building-progress-en-compact.png),
[bestaetigte Pause](building-paused-en-compact.png).

- [Workspace-Tests](tests.txt), [Doctest-Ziele](doc-tests.txt)
- [Formatierung](fmt.txt), [Check](check.txt), [Clippy](clippy.txt)
- [Client-Build](client-build.txt), [Server-Build](server-build.txt)
- [Headless](headless.txt), [Server-Dependencies](server-features.txt)
- [3398 DE/EN-Meldungen, 309 Parameterpatterns, keine offenen Uebersetzungen](localization.txt)
- [Native Galerie](native-keep.txt), [Galerie-Pruefdaten](native-keep.json)
- [Nativer Bauablauf](native-building.txt), [Bau-Pruefdaten](native-building.json)
- [Gesamtpruefung](verification.json), [Quellpruefsummen](source-sha256.json)

Die Pruefsummen erfassen Cargo-Dateien, Konfigurationen, Quellen, JSON-Assets,
UI-Dateien und Dokumentation. Generierte Laufartefakte und historische
Abnahmeordner werden nicht in den neuen Quellstand eingerechnet. Ihre bisherigen
Nachweise bleiben unveraendert historische Staende.
