# Referenzbiome, Flora und F14 — Nachweise vom 14.11.2026

## Abschlussvalidierung

| Prüfung | Ergebnis / Rohprotokoll |
| --- | --- |
| `cargo fmt --all --check` | erfolgreich, [fmt-final.txt](fmt-final.txt) |
| `cargo check --workspace --all-targets` | erfolgreich, [check-complete.txt](check-complete.txt) |
| Workspace-Tests mit lokalen UDP-Integrationstests | **5.128 bestanden**, 0 fehlgeschlagen, 10 bestehende ignoriert, 1 ausdrücklich ausgefiltert; [tests-final.txt](tests-final.txt), [Zählung](test-summary.json) |
| `cargo clippy --workspace --all-targets -- -D warnings` | erfolgreich, [clippy-final.txt](clippy-final.txt) |
| Client, Server, Gebäude-Editor | erfolgreich, [builds-final.txt](builds-final.txt); nach den neuen Biomauswahlfotos erneut [gebaut](builds-with-previews.txt) |
| Python-Assetprüfungen | **32 bestanden**: Pflanzen 3, Biomoberflächen 8, Voxel-Props 5, Blood-Assets 3, Modellaudit 13; jeweilige `*-tests-final.txt` |
| Eigenständiger Headless-Server | mit echtem lokalem UDP gestartet, 27 Sections erzeugt und regulär beendet; [server-smoke.txt](server-smoke.txt) |
| Vorhandene Welten | alle **23 Dateien SHA256-identisch**, keine hinzugefügt; [worlds-integrity.json](worlds-integrity.json) |

Ausführung der Tests:

```sh
CARGO_INCREMENTAL=0 RUST_TEST_THREADS=4 cargo test --workspace --no-fail-fast -- \
  --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
```

Der ausgefilterte Importtest benötigt die externe Originaldatei
`FIVERR/VoxelGrassBlock glb file.glb`, die in diesem Checkout fehlt.
Die ausgelieferten GLBs samt aktiver Grasableitung werden separat durch
Katalog-/Geometrie-/PBR-Tests und die nativen Aufnahmen geprüft. Die zehn bereits
ignorierten Tests werden durch diesen Bericht nicht als ausgeführt dargestellt.
Die zunächst fehlgeschlagenen Modellbudgets, doppelten Wappen-Innenflächen und
veralteten Vegetationsfixtures sind im finalen Lauf korrigiert und geprüft.

Die acht Fotos der Biomauswahl stammen aus dem finalen nativen Biomlauf;
[biome-previews.txt](biome-previews.txt) protokolliert den Zuschnitt von
1920×1080 auf 1536×768. Die Bilder werden verkleinert, nicht hochskaliert.
Gras-PBR-Ableitung und unveränderte Ursprungsgeometrie sind zusätzlich unter
[base-materials-provenance.json](base-materials-provenance.json) belegt.
F14 ist aus der offenen TODO-Liste entfernt. Die gestalterische Abnahme **A01**
bleibt offen: Die Bilder sind noch keine 1:1-Umsetzung der Illustrationen.

## Finaler nativer Biomlauf

`target/reference-biomes/run.ZkiVMP` schließt nach den neuen Grundmaterialien
und der überarbeiteten Panoramaauswahl ebenfalls mit **12 bestandenen Szenen
und 16 PNGs in 1920×1080 Pixeln** ab. Die unveränderten Rohbelege sind separat
unter [final/captures/](final/captures/) und [final/native/](final/native/)
archiviert; die kopierten Dateien wurden per SHA256 mit den Originalen
verglichen. Der [finale Bericht](final/native/verification.json) enthält
Bildprüfsummen, globale Proben, Kameras und Ökologierevision 6.
Auch dieser Lauf verwendet keine künstlichen Weltedits oder Spielerwelten.

Alle acht finalen Biompanoramen wurden visuell geprüft, außerdem die
Pflanzen-Nahaufnahmen für Ebenen und Kaktus. Das
[Hochlandbild](final/captures/picker-highlands-overview.png) zeigt jetzt eine
tatsächlich verschneite Kuppe mit Nadelbäumen. Schnee am Anker und in mehreren
Nachbarspalten ist durch die geladenen globalen Materialproben bestätigt.
Das [Ozeanbild](final/captures/picker-ocean-overview.png) zeigt gleichzeitig
Meerwasser, einen trockenen Gras-/Sandküstenstreifen und Unterwasserpflanzen.
Der Report bestätigt neben der marinen Wassersäule echten trockenen Küstenboden.
Die Produktions-Schneegrenze und das Gelände wurden für diese Fotos nicht geändert.

Die [Ebenen-Nahaufnahme](final/captures/reference-plains-close.png) zeigt die
erneuerte ruhige Grasoberseite, Erdschichten und gruppierte Blütensträucher;
der [Kaktus](final/captures/reference-cactus-close.png) besitzt weiterhin
zusammenhängende Stamm-/Armblöcke. Es sind keine fehlenden Materialien oder
abgetrennten Kaktussegmente erkennbar. Der starke Distanznebel und die gegenüber
den Illustrationen blockigere, stellenweise sparsamere Bepflanzung bleiben
sichtbare Grenzen. Die Aufnahmen beweisen keine identische Referenzbildqualität
und sind keine Messung allgemeiner Laufzeitperformance.

## Finaler nativer Modelllauf

`target/voxel-creatures/run.fbMsGs` ist nach den letzten Modellkorrekturen mit
**sechs bestandenen Galerien und 43 PNGs in 1920×1080 Pixeln** abgeschlossen.
Alle 43 Bilder wurden einzeln visuell geprüft. Unter [mobs/](mobs/) liegen
die unveränderten Rohbilder, Prozesslogs, isolierten Konfigurationen und der
[Prüfbericht mit Bildprüfsummen](mobs/verification.json). Die archivierten
Bilder wurden nochmals gegen die SHA256-Werte des ursprünglichen Laufs geprüft;
Spielstände und Binärdateien wurden nicht kopiert.

| Galerie | Aufnahmen | Geprüfter Inhalt / Beispiel |
| --- | --- | --- |
| Standardtiere | 13 | Sechs Arten mit zwei Fischvarianten; Tierdetails und Bärenposen einschließlich [Angriff](mobs/standard/creatures-10.png) |
| Spirit | 4 | Sieben Schmetterlingsvarianten, Gloom-Varianten und Spirit-Fuchs; [Nahansicht](mobs/spirit/spirit-creatures-1.png) |
| Wald | 4 | Drei Skelettausrüstungen und fünf Blutwald-Arten; [Nahansicht](mobs/forest/forest-creatures-1.png) |
| Wasserwesen | 13 | Tintenfisch, Quallen und Haie; [Hammerhai](mobs/aquatic/creatures-7.png) |
| Pferde | 4 | 16 Fellvarianten und ausgewählte Posen; [Bewegung](mobs/horse/horses-2.png) |
| Bewohner | 5 | Zehn repräsentative Rollen in beiden Geschlechtern, Werkzeuge und Schlafpose; [Arbeitsansicht](mobs/villagers/villagers-4.png) |

Die gestuften Konturen und charakteristischen Details bleiben in den geprüften
Ansichten erkennbar. Es sind keine offenen Flächen, explodierten Normalen,
fehlenden Materialien oder abgerissenen Gelenke sichtbar. Rippenzwischenräume,
Waffen, acht Stalker-Beine, Flügel und Flossen sind vorhanden. Die sehr hellen
Spirit-Fox-Schweife verlieren Schattierungsdetails; HUD-Elemente verdecken in
einzelnen Nahansichten Teile der Köpfe. Diese Galerien zeigen ausgewählte Posen,
keine vollständige Rundumprüfung jeder Art. Wasserwesen stehen zur Modellprüfung
über einer Testfläche; Pferde sind überwiegend von hinten sichtbar. Die Bilder
belegen weder serverseitige KI noch Navigation oder Schwimmverhalten.

Der Client-SHA256 blieb vom Start bis zum Ende dieses Laufs unverändert.
Der anschließende Build für neue Biomauswahlfotos änderte keine Modellquellen.
Das [Wiederholungsskript](../../run-voxel-creatures.sh) sichert inzwischen
zusätzlich einen temporären Hardlink auf das gewählte Executable; es baut nicht
und öffnet keine Spielerwelten. Der frühere 30-Bilder-Lauf bleibt in der
[Modelldokumentation](../../../../docs/voxel-creature-models.md) ausdrücklich
als historischer Zwischenstand beschrieben.

## Erster nativer Lauf

`target/reference-biomes/run.41jkJI` lief mit dem tatsächlich gebauten Client
durch: **12 Szenen, 16 PNGs mit 1920×1080 Pixeln**, Status `passed`.
Die kopierten Rohbilder liegen unter [captures/](captures/), die zugehörigen
Berichte, Konfigurationen und Prozessprotokolle unter [native/](native/).
Es wurden keine Spielstände oder ausführbaren Dateien in diesen Nachweis kopiert.

Der Lauf verwendet Seed 42042, die frischen Generatorrevisionen einschließlich
`ecology_revision = 6` und die aktuellen Biom-JSONs. Es wurden ausschließlich
neue isolierte Testwelten angelegt. **Keine künstlichen Weltblöcke und keine
Spielerweltänderungen** (`fixture_edits = 0`). Die Aufnahmezeit ist 10 Uhr.
Die vollständige Szenenliste, Bildabmessungen und SHA256-Werte stehen in
[verification.json](native/verification.json).

| Szene | Tatsächlich geprüfter Inhalt |
| --- | --- |
| Acht `picker-*`-Szenen | Ebenen, Wüste, Kirschwald, Hochland, Blutwald, Spiritwald, Ozean und Wald; Biomgewichte und globale Geländeproben |
| `reference-cactus` | Zusammenhängender natürlicher Kaktus bei `[-1817, 28, -1862]`, 8 Blöcke hoch, 12 Voxel einschließlich Armen, Sand darunter und Luft über der Spitze |
| `reference-plains` | Tatsächliche regionale Blumen-/Strauchgruppen, Panorama und Nahaufnahme |
| `reference-desert` | Tatsächliche Trockensträucher auf Wüstenboden, Panorama und Nahaufnahme |
| `reference-spirit` | Tatsächliche gruppierte Spirit-Leuchtblumen, Panorama und Nahaufnahme |

Die vier Pflanzenansichten vergleichen die erwarteten Zellen mit den durch
normales Streaming geladenen Voxeln. Beide Kameras stehen in geladener Luft,
ihre Sichtlinien wurden überprüft. Die Kaktusprüfung erfasst alle Segmente und
die Verbindung über Blockflächen; eine einzelne Aufnahme beweist nicht die
gesamte Verteilung aller Höhen 2–8. Dafür sind die Generatorregressionen zuständig.

## Sichtprüfung und Grenzen

Alle acht Panoramen und die vier Pflanzen-Nahaufnahmen wurden visuell geprüft.
Es sind keine fehlenden oder magentafarbenen Materialien, schwebenden Pflanzen
oder abgetrennten Kaktusarme erkennbar. Die Gruppen, unterscheidbaren Baumkronen
und Biomfarben sind sichtbar. Die Referenzillustrationen sind dichter bepflanzt
und weicher beleuchtet; dieser Nachweis behauptet keine identische Bildwirkung.
Der deutliche Distanznebel verdeckt in mehreren Panoramen Teile des Hintergrunds.

Der erste Hochland-Picker wählte eine grasbewachsene Stelle bei rund 110 m.
Das erste Ozeanbild zeigt überwiegend offenes Wasser. Diese beiden Fotos
belegen deshalb **weder Schneelandschaft noch Küsten-/Unterwasserflora**.
Der Picker wurde anschließend auf vorhandene echte Schneefelder bzw. eine
nachgewiesene trockene Küste neben Meerwasser umgestellt. Gelände und
Schneegrenze wurden dafür nicht verändert. Die obigen Rohbilder bleiben Belege
des ersten Laufs; eine spätere Aufnahme muss mit ihrem eigenen Bericht geprüft
werden, bevor diese Einschränkung für den endgültigen Lauf entfällt.

## Quellen und Wiederholung

- [Generatorbeschreibung](../../../../docs/biome-flora-ecology-6.md): Habitatregeln, Material-IDs 422–429, Revisionserhalt und gemeinsame Section-/Globalpfade.
- [Prüfskript](../../run-reference-biomes.sh): startet die zwölf nativen Szenen ohne eigenen Cargo-Build; einzelne Szenen können als Argumente übergeben werden.
- Native Pflanzenprüfung: `apps/client/src/biome_smoke/reference_scene.rs`.
- Panoramaauswahl: `apps/client/src/biome_smoke/picker.rs`.
- Pflanzen-/Kaktusplanung: `crates/terra-world/src/generation/natural_features/`.
- Historische Ökologierevisionen 0–5 bleiben beim Laden erhalten; die neue Gruppierung gilt für Revision 6.

Der erste Volltest meldete zwei veraltete World-Fixtureannahmen (Blume in vier
beliebigen Sections und fest angenommene Vier-Meter-Wurzel). Die korrigierten
Tests prüfen tatsächliche Pflanzengruppen und echtes Holz an negativen
Sectiongrenzen einschließlich Edit-/Padding-/Reload-Erhalt für Ecology 0–6.
Die abschließende Workspace-Suite bestätigt beide Korrekturen.
