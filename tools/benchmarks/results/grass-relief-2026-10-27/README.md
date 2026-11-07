# Graslippen und Steinrelief

## Umsetzung

Das vorhandene FIVERR-Modell bleibt die Grundlage. Sein abgeleitetes
Nahmesh besitzt jetzt echte blockige Graslippen und kleine vorstehende
Steine, nicht nur Normalmap-Schattierung. Die Formen werden offline aus
passenden gruenen bzw. grauen Bereichen der Original-Albedo ausgewaehlt.
Erdseiten sind lokal vertieft, die Details sitzen darin und bleiben in
der logischen Blockzelle `[0,1]`. Keine schwebenden Zusatzobjekte und
keine groesseren Kollisionsboxen.

Aktuell 36 Grasformen und 24 Steinformen: 600 zusaetzliche Dreiecke und
1200 Vertices fuer das vollstaendige Nahmodell. Original: 768 Dreiecke,
1494 Vertices; aufbereitetes Nahmodell: 1368 Dreiecke, 2694 Vertices.
Verdeckte Seiten werden mitsamt ihren Details entfernt. Die Grasoberseite
bekommt nur ein flaches Relief auf vorhandenen Vertices, keine neuen
Dreiecke. Die Unterseite bleibt unveraendert.

Keine neuen Materialslots, Entities oder Draw Calls je Detail. Original-
BaseColor, Normal und ORM bleiben erhalten. Middle/Far sowie ausgeschaltete
Micro-Geometrie verwenden weiterhin die bisherigen flachen Greedy-Faces.
Die vorhandene Grafikoption und Detaildistanz begrenzen die Nahgeometrie.

Die erste innere Reihe des 8x8-Grundrasters bleibt unveraendert. Dies wurde
nach einem tatsaechlich fehlgeschlagenen gedrehten Vier-Section-Eckentest
korrigiert; der Eckentest selbst wurde nicht abgeschwaecht. Das separate
Tiefenbudget fuer Gras im allgemeinen Oberflaechentest wurde passend zum
tatsaechlichen Relief auf 0.0651 angepasst; alle Unit-Cell-Grenzen bleiben
unveraendert streng.

## Sichtpruefung

Echter Client, RX 6950 XT, RADV/Vulkan, 1280x800. Original links, bearbeitetes
Terrain rechts: [Seitenansicht](grass-side.png), [Vorderseite](grass-front.png).
Drei GPU-Pixelpruefungen bestanden, alle Aufnahmen vollstaendig erstellt,
Exit 0, keine WARN-/ERROR-Meldungen in `/tmp/teramic144-grass.log`.
Beide verlinkten Bilder wurden tatsaechlich angesehen.

Die gewuenschte Tiefenwirkung ist umgesetzt. Eine pixelgenaue 1:1-Kopie des
Referenzbildes ist es nicht: Die Farben, Steinverteilung und Texturzeichnung
stammen weiter aus dem gelieferten FIVERR-Asset; dessen Albedo unterscheidet
sich von der Referenz. Beleuchtung und Umgebung des Bildes wurden ebenfalls
nicht nachgebaut.

## Validierung

- `cargo fmt --all --check`: erfolgreich.
- `cargo check --locked --workspace --all-targets -j1`: erfolgreich.
- `cargo test --locked --workspace --all-targets -j1 -- --test-threads=2`:
  2744 bestanden, 7 ignoriert.
- `cargo clippy --locked --workspace --all-targets -j1 -- -D warnings`:
  erfolgreich.
- `cargo build --locked --workspace --bins --example import_block_faces -j1`:
  erfolgreich, Client und Dedicated Server.
- Importwerkzeug `--check`: GLB und Provenienz bytegenau reproduziert.

Neue Tests: Farbmasken, Budgets, verwurzelte Formen, gueltige UVs/Indizes,
unveraenderte Raender, deterministische Geometrie, echte Gras-/Steinformen
im Originalatlas sowie unveraenderte PBR-Bilder und grobe LOD-Meshes.
Die bestehenden Nachbar-, Negativkoordinaten-, Chunk- und LOD-Eckentests
sind ebenfalls erfolgreich. Original-GLB bleibt unveraendert.

## Kurze Laufzeitprobe

Identische Uebersicht wie zuvor: Seed 42042, Sichtweite 12, 1280x800,
klare Witterung, Kamera `[4300,155,310]` nach `[4352,110,256]`.
Jeweils nur 6 Sekunden Messfenster nach Laden der Welt, kein Langzeittest.

| Stand/Lauf | AVG FPS | Min FPS | p95 ms | p99 ms | Frames unter 32 FPS |
| --- | ---: | ---: | ---: | ---: | ---: |
| Ohne neues Nahrelief, vorheriger Lauf | 127.66 | 73.73 | 10.53 | 12.31 | 0 |
| Mit Nahrelief, erster Lauf | 139.53 | 25.79 | 9.97 | 10.74 | 1 |
| Mit Nahrelief, Wiederholung | 142.24 | 85.13 | 9.06 | 10.60 | 0 |

Der erste Lauf enthielt einen einzelnen 38.78-ms-Ausreisser, ohne aktive
Terrain-/Prop-/Generierungsjobs oder Uploads im vorherigen Frame. Eine
Ursache laesst sich daraus nicht ableiten. Parallel wurden waehrend dieses
Laufs Dokumentationsdateien geschrieben; bei der Wiederholung lief nur
der Client. Der Ausreisser wird nicht aus dem Ergebnis entfernt.

Alle Laeufe zeigen 4652 sichtbare Mesh-Entities. Die hoeheren Mittelwerte
belegen keine FPS-Optimierung; Desktop-Last und kurze Laufzeit schwanken.
Diese Uebersicht ist zudem kein gesonderter Nahbereichs-Stresstest.
Ein genereller FPS-Fix oder garantiertes 32-FPS-Minimum wird nicht behauptet.
GPU-Gesamtzeit und VRAM wurden nicht separat gemessen.

Logs: `/tmp/teramic144-{tests-verified,fmt-final,clippy,check,build-final,
grass,world,world-repeat}.log`.
