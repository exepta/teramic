# Spirit Blossom und Gebietsnebel – Abnahme 13.11.2026

[Funktionsbeschreibung](../../../../docs/spirit-blossom.md).
Die Laufzeitaufnahmen stammen vom nativen Client auf der AMD Radeon RX 6950 XT
(Vulkan/RADV). Saemtliche Grafiktests verwenden eigene temporaere Konfigurationen,
Profile und Welten. Die neun Dateien der Originalwelt `worlds/Test` behalten ihre
SHA-256-Pruefsummen (`original-world-before.json`, `original-world-after.json`).

## Baunebel

`construction-run-final.txt` und `construction/verification.json` belegen einen
erfolgreichen nativen Ablauf mit echtem TAB, Kamerabewegung, Bauvorlagenauswahl,
Auftragsbestaetigung, NPC-Inventar, ESC und Save/Reload. Sieben Aufnahmen pruefen
DE/EN, gespeicherte Gebiete, H aus/ein und ein zeitlich versetztes Animationsbild.
Die Animation veraendert Dichte und Drift; Mesh-/Material-Handles bleiben gleich.

- [Gebiete](construction/construction-areas-de-wide.png)
- [Spaetere Animationsphase](construction/construction-areas-animated-de-wide.png)

## Spirit Blossom und Kreaturen

`spirit-run-final.txt` und `spirit-verification.json` bestaetigen fuenf native
1920x1080-Aufnahmen ohne Shaderfehler. Der Generator laedt 4.851 echte Sections.
Die Biomauswahl verwendet einen auf 1536x768 verkleinerten Ausschnitt dieser
Aufnahme; Quell- und Zielpruefsumme stehen in `src-ui/assets/biome-previews.json`.

- [Generiertes Biom](biome/picker-spirit-blossom-overview.png)
- [Alle zehn Kreaturenvarianten](creatures/spirit-creatures-0.png)
- [Schmetterlinge](creatures/spirit-creatures-1.png)
- [Geisterlichter](creatures/spirit-creatures-2.png)
- [Geisterfuchs](creatures/spirit-creatures-3.png)

Die Readiness-Diagnose fand einen bisherigen Leerlauffehler im Setzlingssystem:
Schon das Weiterreichen leerer Wachstumsvorgaenge markierte Welt und Journal
jeden Frame als veraendert. Der Pending-Guard beseitigt die Ursache. Die
native Pruefung wartet weiterhin auf vollstaendige Renderer-Freigabe.

## Einordnung

Die Referenzbilder geben Gestaltung und Inhalte vor. Die Spielassets sind eigene
prozedurale GLB-/PNG-Dateien und animierte Kreaturenmeshes. Vorhandene Fluesse und
Seen liefern die Gewaesser; die konkreten Szenen aus den Referenzen sind keine
vorgefertigten Landschaften. Die Kreaturengalerie ist ein Renderer-Test;
autoritative Spawns, Flug, Kollision, Aura und Persistenz haben separate Rust-Tests.

Die Protokolle `*-first.txt`, `check-initial.txt`, `construction-run.txt` und
`spirit-run.txt`, `spirit-run-readiness.txt`, `workspace-tests-before-recipe-count.txt`
und `*-before-catalog-fix.txt`
koennen Diagnosefehler frueherer Zwischenstaende enthalten.
Massgeblich sind die unten festgehaltenen Abschlusslaeufe.

## Befehle und externe Fixture

```sh
cargo fmt --all --check
cargo check --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo build --locked --offline -p terra-client
cargo build --locked --offline -p terra-server
cargo test --workspace --locked --offline --no-fail-fast -- \
  --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
```

Der ungefilterte Testlauf wurde ebenfalls ausgefuehrt (`workspace-tests.txt`).
Die Abdeckung der neuen Schrittgeraeusch-Materialien wurde danach korrigiert.
Der externe Original-GLB-Test benoetigt die nicht vorhandene Datei
`FIVERR/VoxelGrassBlock glb file.glb`. Ausschliesslich dieser Test wird im
abschliessenden Lauf explizit ausgefiltert; es wurde keine Ersatz-Fixture erzeugt.

`headless.txt` prueft den echten Server mit entfernten DISPLAY-/WAYLAND_DISPLAY-
Variablen, lokalem UDP-Socket, 27 Sections und geordnetem Ende.
`server-dependencies.txt` enthaelt keine bevy_render-/bevy_winit-/wgpu-/tilt-ui-
Abhaengigkeit. `assets.json` prueft alle 18 neuen GLB2-Dateien, JSON-IDs und
Texturpfade.

## Testergebnis

Der abschliessende Workspace-Testlauf besteht mit **5.019 bestandenen Tests,
0 Fehlern, 10 bereits vorhandenen ignorierten Tests und einer explizit
ausgefilterten externen Fixture**. Alle Doctest-Ziele bestehen ebenfalls.
Enthalten sind unter anderem 1.413 Welt-, 1.112 Renderer-, 657 UI-,
632 Netzwerk-, 536 Core-, 209 Client- und 190 Speichertests.

Die Regressionen pruefen deterministische Spirit-Regionen und Section-Grenzen,
natuerliche Spawns/Bewegung aller neuen Tierarten, begrenzte Regeneration,
Block-/Licht-/Wasserpflanzenvertraege, neun Biomgewichte im Protokoll, alte
Kartendateien, echte alte Welt-TOMLs ueber mehrere Neustarts sowie das
Unterbleiben falscher ECS-Aenderungsmarkierungen bei leerer Setzlingsarbeit.

Formatpruefung, Workspace-Check aller Targets, striktes Clippy aller Targets
(`-D warnings`), Client-Build und unabhaengiger Server-Build bestehen.
Die abschliessend gebaute Server-Binary besteht erneut den Headless-Test;
alle neun Originalwelt-Dateien sind weiterhin unveraendert. Abschlussprotokolle:
`fmt.txt`, `check-final.txt`, `clippy-final.txt`, `workspace-tests-final.txt`,
`binaries-build-final.txt`, `server-build-independent.txt`, `headless.txt`.
