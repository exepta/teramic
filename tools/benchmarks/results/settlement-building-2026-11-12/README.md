# Handwerker, Stadtlager Und Lokaler Hausbau

Phase 238, Abnahme vom 12.11.2026. Bedienung und Grenzen:
[Siedlungsbau](../../../../docs/settlement-building.md).

## Automatisierte Pruefung

**4800 Library-/Binary-Tests und vier Integrationstests bestanden.** Zehn
bestehende Tests bleiben ignoriert. Nur
`original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images`
wird wegen der bereits fehlenden externen Datei
`FIVERR/VoxelGrassBlock glb file.glb` ausgefiltert. Die Dokumentationstest-Ziele
laufen erfolgreich durch und enthalten derzeit keine ausfuehrbaren Beispiele.

Dreizehn Konstruktionstests pruefen unter anderem vier vollstaendige Dorfhaeuser:
alle vier Ausrichtungen erreichen jeweils nach **832 wirklichen Bauschritten**
`Complete`. Die NPCs laufen, holen echte Materialien, erreichen tatsaechliche
erhoehte Standflaechen, bauen Dach und gepaarte Einrichtung, entfernen das
bezahlte Geruest und liefern dessen Holz wieder ab. Der Test prueft die letzte
Weltbelegung jeder Planzelle, leere Handwerkerinventare, die genaue Materialbilanz
und das Wiederherstellen des laufenden Auftrags aus einem serialisierten Register.
Die Bauzeitpruefung misst die verlangten 30/15/10/5 Minuten aktiver Echtzeit fuer
ein/zwei/drei/vier anwesende Handwerker. Laufwege und Warten kommen hinzu.
Auch fuenf gleichzeitig arbeitende Baustellen erhalten jeweils die vollen zwei
Sekunden Fortschritt nach zwei Sekunden. Der vorherige Vier-Projekt-Scheduler
lieferte nur 1,6 Sekunden; alle maximal 32 aktiven Projekte werden jetzt pro Tick
verarbeitet. [Regression vorher](project-clocks-before-fix.txt),
[gesamte Welt-Testsuite nachher](world-project-clocks.txt).

Weitere Regressionen pruefen getrennte Lagertruhen mit echten Rueckwegen,
Abbruch bei vollem Lager, Berechtigungsentzug, blockierte/ungeladene Zellen,
Spielerkoerper, gepaarte Truhen und stabile Steuerrevisionen. Der Holzfaellertest
verwendet eine natuerliche Fichte auf terrassiertem Boden, Ruby-Axt und 128
Fichtensetzlinge im separaten Berufstisch. Er prueft wirkliche Ernte, Ablieferung
und Nachpflanzung. Wanderpruefungen kontrollieren gelaufene Strecke und Build-
Grenzen; Bewohner verlassen ihre Haeuser ueber berechtigte Tueren.

- [Workspace-Tests](tests.txt), [Integrationstests](integration.txt)
- [Komplette Konstruktionen](tests-building.txt), [Dokumentationstests](doc-tests.txt)
- [Cargo-Check](check.txt), [Clippy](clippy.txt), [Formatierung](fmt.txt)

## Native Bau-Abnahme

Ein isolierter Grafikclient mit echtem lokalem UDP-Server gruendet ein Kingdom,
uebertraegt 128 Stein und 128 Bretter aus einem echten Inventar in eine wirkliche
Truhe, markiert den Stadtlagerbereich per Raycast und speichert ihn. Anschliessend
beansprucht er einen zweiten Build-Chunk, laeuft dorthin und waehlt den Hausboden
innerhalb der normalen Auswahlreichweite. Die eigentliche Hausposition wird durch
den normalen HUD-Auswahlmodus bestimmt.

Der HTML-Button erzeugt einen wirklichen Hausauftrag mit Materialliste und
zunaechst fehlenden Handwerkern. Ein weiterer echter Button meldet genau den
durch dessen `$set` ausgewaehlten NPC als Handwerker an. Ein eigenes
Arbeitsgebiet wird nicht erfunden. Der Lauf wartet auf eine bestaetigte
**Nicht-Air-Blockplatzierung** und pausiert den Auftrag anschliessend ueber die
revisionsgepruefte UI. Reiner Aushub gilt nicht als Baunachweis.

Fuenf Aufnahmen und tatsaechliche Button-/Textgrenzen pruefen Deutsch bei
1600x900 sowie Englisch bei 960x640. Die Eingaben sind framegesteuerte
Gameplay-Tasten und echte TiltUI-HTML-Handler; dieser Lauf ist kein OS-Maus-
Automatisierungstest. Der kurze Grafiklauf prueft die erste Blockplatzierung
und Pause, waehrend die CPU-Tests den vollstaendigen Bau pruefen.

- [Lokales Bauen, Deutsch](building-local-de-wide.png)
- [Auswahl-HUD, Deutsch](building-storage-selection-de-wide.png)
- [Auftrag, Englisch kompakt](building-job-en-compact.png)
- [Echte Blockplatzierung](building-progress-en-compact.png)
- [Bestaetigte Pause](building-paused-en-compact.png)
- [Natives Protokoll](native-building.txt), [Maschinenlesbare Pruefung](native-building.json)

Der Lauf liegt unter `target/building-local/run.n7LV5B/`. Welt, Profil und
Konfigurationen sind temporaer und isoliert. GPU: AMD Radeon RX 6950 XT,
Vulkan/RADV. Ein separater Server-Build startet ohne Display und generiert 27
Sections; sein Dependency-Baum enthaelt keine Render-, Windowing- oder UI-Crates.
[Headless-Protokoll](headless.txt), [Server-Features](server-features.txt),
[Server-Build](server-build.txt), [Client-Build](client-build.txt).
Nach der Parallelbau-Zeitkorrektur wurden beide Binaries erneut gebaut:
[finaler Build](final-build.txt). Der native Bau-Lauf und Headless-Start wurden
mit diesem Stand wiederholt. [Gesamtpruefung](verification.json) und
[Quellpruefsummen](source-sha256.json).

Auch der vorhandene direkte NPC-Lauf besteht mit diesem Build erneut:
Rechtsklick, privates Inventar, HUD-Auswahl/Abbruch, Zuweisung/Pause und wirklicher
Holzfaeller-Stationsbesuch mit unveraendertem Geschlecht. Neun Aufnahmen entstehen
unter `target/npc-work-direct/run.KFZEUT/`; drei davon sind hier archiviert.
[Protokoll](native-npc.txt), [Pruefdaten](native-npc.json),
[Deutscher NPC-Dialog](npc-direct-de-wide.png),
[Fehlende Station](npc-lumberjack-missing-station-compact.png),
[Berufswechsel](npc-lumberjack-profession-compact.png).

## Wiederholen

```sh
cargo fmt --all --check
cargo check --locked --offline --workspace -j 4
cargo clippy --locked --offline --workspace --all-targets -j 4 -- -D warnings
cargo test --locked --offline --workspace --lib --bins --test headless --test textiles --test generated_sources --no-fail-fast -j 4 -- --test-threads=4 --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
cargo test --locked --offline --workspace --doc -j 4
cargo build --locked --offline -p terra-server --bin terra-server -j 4
cargo build --locked --offline -p terra-client --bin terra-client -j 4
env -u DISPLAY -u WAYLAND_DISPLAY target/debug/terra-server --smoke-test --no-save
bash tools/benchmarks/run-building-local.sh
bash tools/benchmarks/run-npc-direct.sh
```

Es gibt keine neuen Dependencies oder Versions-Upgrades. Protokoll139 und
Speicherindex ID24 sind auf Client und Server gemeinsam zu verwenden. Die
erste Vorlage ist das bestehende Dorfwohnhaus; Auftraege laden entfernte Chunks
noch nicht selbst. Fuer eigene JSON-Vorlagen ist die separate Editor-App als
F82 in der TODO erfasst.
