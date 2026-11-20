# TERAMIC Building Editor

Eine eigenständige Bevy-/TiltUI-Anwendung für echte Gebäudevorlagen. Sie startet unmittelbar in einer lokalen leeren Bauwelt, ohne Spielhauptmenü, Netzwerkverbindung oder Serverprozess. Renderer, Blockmodelle, Inventarkatalog, Flugsteuerung und Platzierungsregeln stammen aus den bestehenden Workspace-Crates.

```bash
cargo run -p terra-building-editor
cargo run -p terra-building-editor -- --load assets/buildings/templates/village/residential.json
```

Die Toolchain und Dependency-Versionen werden vom Workspace übernommen.

## Bedienung

- Maus: umsehen; WASD: fliegen; Leertaste/Shift: steigen/sinken.
- Linksklick: Gebäudeblock abbauen; Rechtsklick: ausgewählten Block platzieren. Truhen verbinden sich normalerweise automatisch mit einem passenden freien Nachbarn. Shift auf Boden oder Truhenoberseiten hält sie getrennt; Shift an einer passenden freien Truhenseite wählt nur diesen Partner. Bestehende Doppeltruhen blockieren keine weiteren Nachbarn.
- `E`: den tatsächlichen Spielinventarkatalog durchsuchen, nach Kategorien filtern und Items auswählen. Werkzeuge und Nahrung werden nicht in fiktive Blöcke umgewandelt.
- `Esc`: Fortsetzen, Optionen, JSON laden, JSON speichern, neues Gebäude oder beenden.
- `Strg+S`: Speichern-Dialog.

Der goldene Umriss markiert das gültige Wohnhaus-Bauvolumen: relativ zum Anker X −4…4, Y −2…9 und Z −6…4. Die Achsen markieren den Anker. Die geschützte Struct-Bauhilfe verwendet den vorhandenen Block `MaterialId::StructureStone` (ID 43) und liegt separat auf Y −3. Sie lässt sich weder abbauen, ersetzen noch seitlich durch Blockplatzierung erweitern. Direkt darüber liegen die echte Fundamentebene Y −2 und der Hausboden Y −1; beide gehören zum Modell. Die Bauhilfe gehört niemals zu exportierten Voxeln oder Materialkosten.

**ESC → Optionen → Größe der Bauhilfe** wählt 9×9, 17×17, 25×25 oder 33×33. Die Änderung wirkt sofort und bewahrt Modell, Kameraposition, Dateipfad und Änderungsstatus. Die Größe bleibt beim Laden und bei „Neues Gebäude“ innerhalb derselben Editorsitzung erhalten; sie ist keine Modell- oder JSON-Eigenschaft. Die Schema-1-Vorlagengrenzen des goldenen Umrisses bleiben unverändert. Größere Bauhilfen passen ihre Streaming-/Sichtweite automatisch an.

Die Optionen ändern außerdem die tatsächliche Fluggeschwindigkeit, Mausempfindlichkeit, Sprache der Editoroberfläche, Fenstermodus und VSync. Die Editoroberfläche unterstützt Deutsch und Englisch; Itemnamen verwenden die vorhandenen Spielübersetzungen.

## Laden und speichern

Der eingebaute Dateibrowser zeigt reale Verzeichnisse und JSON-Dateien. Die Schaltfläche „Standard-Dorfhaus laden“ öffnet die vorhandene, vom Spiel verwendete Dorfhausvorlage. Ein gültiger Import rekonstruiert die tatsächlichen Voxel und Objektzustände; die bestehende Welt wird erst nach erfolgreicher Validierung ersetzt.

Beim Speichern werden Anzeigename und sicherer JSON-Dateiname getrennt angegeben. Bestätigtes Überschreiben einer vorhandenen gültigen Vorlage behält die ID der Zieldatei. Ein neuer, noch nicht existierender Dateiname erzeugt eine neue ID aus dem Dateinamen. Doppelte IDs im tatsächlichen Spielkatalog werden abgewiesen. Vorhandene Dateien benötigen eine ausdrückliche Überschreibbestätigung. Bestätigtes Überschreiben verwendet eine temporäre Datei und atomisches Umbenennen. Ein leerer Entwurf lässt sich nicht als fertiges Gebäude speichern.

Aus den gesetzten Blöcken entstehen geordnete Bauphasen und exakte Materialkosten. Zusammengehörige Betten und Türen bleiben ein atomarer Bauschritt; eine Doppeltruhe behält beide Zellen und kostet zwei Truhen. Alle 24 Treppenstellungen sowie senkrechte und gemischte Halbplatten verwenden die gemeinsamen Schema-Regeln für Rotation und Materialbedarf. Der Export wird mit `BuildingTemplate::validate()` und einem erneuten JSON-Roundtrip geprüft. Vorlagen enthalten keine Inventarinhalte, Beute oder NPCs.

Das aktuelle Schema 1 besitzt Grenzen für darstellbare Objektzustände. Individuelle RGB-Farben oder Holzvarianten ohne entsprechende Material-ID werden nicht stillschweigend entfernt. Der Editor meldet eine konkrete Einschränkung. Nicht platzierbare Inventaritems bleiben im Katalog verfügbar.

Die Unterordner unter `assets/buildings/` bilden die Kategorien des Spielkatalogs; sie stehen nicht als frei erfundene Kategorie im JSON.

## Struktur und Prüfungen

`src/workspace.rs` enthält die getrennte, validierte Bauhilfe-Konfiguration. `src/document/` enthält Voxelmodell, geordneten Export und Dateizugriff. `src/editor.rs` verarbeitet Eingaben und reale Platzierungstransaktionen. `src/scene.rs` setzt Bauhilfe und Weltzustand auf. `src/interface/` projiziert den Editor in die eigenen Komponenten unter `ui/components/building-editor-*/`.

```bash
cargo test -p terra-building-editor
cargo clippy -p terra-building-editor --all-targets -- -D warnings
cargo build --locked --offline -p terra-building-editor
bash tools/benchmarks/run-building-editor.sh
```

Der opt-in native Smoke-Ablauf nutzt die echte TiltUI-Oberfläche: leeres Dokument, echtes Platzieren/Abbauen über normale Mausereignisse und Voxel-Raycast, realer Inventarkatalog mit gerenderten Itemvorschauen, Standardhaus laden, JSON-Datei speichern und erneut validieren, Überschreibbestätigung und englische Optionen. Der Runner verwendet bei jedem Start ein frisches Prüfverzeichnis unter `target/building-editor/`; Screenshots und JSON landen ausschließlich dort. Die Befehle sind hier als ausführbare Prüfpfade dokumentiert; erfolgreiche Läufe werden im Projektbericht separat angegeben.
