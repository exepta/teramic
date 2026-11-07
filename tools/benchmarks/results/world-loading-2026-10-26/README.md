# Weltstart Und Verdeckte Sections (Phase 135)

## Reproduktion

Der echte Frontend-Pfad wurde mit `--smoke-test-loading` nachgestellt:
neue Welt ueber die WorldLibrary erzeugen, world-play, integrierter Server,
Handshake, Weltgenerierung, Ladebildschirm und Uebergang ins Spiel. Anders
als die vorigen Benchmark-Fixtures setzt dieser Test Playing nicht direkt.
Die root-Konfiguration wurde nach /tmp kopiert; vorhandene Welten und die
Originalkonfiguration wurden nicht veraendert.

Seed 42042: Alle 7624 angeforderten Sections sind geladen. Ab etwa 15 s sind
Generierung, Renderer, Props und Wasser fertig, trotzdem bleibt LoadingProgress
bei 1084 / 1089 Chunk-Spalten (99,54 %). Fuenf CPU-Meshes gehoeren zu durch
Section-Portale verdeckten Hoehlen und werden absichtlich nicht hochgeladen.
Dieser Zustand besteht im unveraenderten Lauf bis etwa 70 s. Erst eine spaetere
Sichtbarkeitsaenderung loest die falsche Wartebedingung. Es ist ein belegter
langer Lade-Stall, kein nachgewiesener permanenter Server-Deadlock.

## Ursache Und Korrektur

Der Renderer akzeptiert bereits `mesh konsumiert ODER absichtlich verdeckt`.
Der Session-Ladebildschirm und die alte Smoke-Bereitschaftsabfrage prueften
dagegen nur `mesh konsumiert`. Beide Bedingungen widersprachen sich.

`RenderStatus::section_ready` ist jetzt die gemeinsame Abfrage fuer Renderer,
Session-Fortschritt und Smoke-Tests. Sie akzeptiert verdeckte CPU-Meshes, wartet
aber unveraendert auf fehlende Section-Daten und laufende Spalten-Jobs. Die
globalen Gates fuer Generierung, Terrain, Props und Wasser bleiben erhalten.
TerrainVisibility bietet dazu nur lesenden Zugriff; keine neuen Caches,
kopierten Hidden-Sets, erzwungenen GPU-Meshes oder deaktivierte Portaloptimierung.
Keine Registry-, Renderer-Dependency-, Server- oder Saveformat-Aenderung.

## Native Validierung

Ryzen 5900X / RX 6950 XT, Vulkan, optimiertes Dev-Profil. Gleiche kopierte
Benutzerkonfiguration, keine parallelen Builds/Unit-Tests. Nur kurze Laeufe.
Ungefaehre Zeit bis zum Screenshot der fertig eingeblendeten Welt, aus den
5-s-Fixture-Diagnosen und Capture-Zeitstempeln; keine allgemeine Worldgen-Benchmark.

| Seed | Stand | Fertig eingeblendete Welt | Ergebnis |
|---|---|---:|---|
| 42042 | Vorher | ca. 74 s | Falsches Warten bei 1084/1089 |
| 42042 | Korrigiert | ca. 15 s | 100 %, Fade-in, Welt-Audio danach |
| 7 | Korrigiert | ca. 15 s | 100 % trotz 16 absichtlich verdeckter CPU-Meshes |
| 5866727212944175842 (Seed der Benutzerwelt Test) | Korrigiert, separate neue Welt | ca. 14 s | 100 %, Fade-in, Welt-Audio danach |

Alle drei korrigierten Laeufe: Exit 0, 100%-Hold, monotones Reveal, Spielseite
und Audiostart danach geprueft. Die fertige Welt von Seed 42042 wurde auch
im Capture betrachtet. Der Seed-7-Lauf prueft negative Spawn-Koordinaten sowie
8550 geladene Sections. Die neue Fixture schlaegt fehl, wenn eine wirklich
fertige Szene laenger als zwei Sekunden auf Fortschritt unter 100 % wartet;
das bisherige 90-s-Limit fuer tatsaechliche Generierungsarbeit bleibt bestehen.

Im ersten korrigierten Lauf erschien beim Beenden die bekannte Screenshot-
Sendemeldung ueber einen geschlossenen Kanal, nach allen erfolgreichen Captures.
Der zweite Lauf hatte diese Meldung nicht. Keine allgemeine Behebung dieses
separaten Abschlussrennens behauptet.

## Regressionstests

Sechs neue Renderer-Tests decken ab: fehlende Section auch wenn verdeckt;
sichtbares noch unbenutztes CPU-Mesh; verdecktes Mesh und spaeter geoeffnetes
Portal; laufende Spalten-Jobs auch bei verdeckten/konsumierten Meshes;
fertige sichtbare Section; konservatives Verhalten ohne Visibility-Ressource.

- `cargo test --workspace --locked -j2 -- --test-threads=2`: 2540 bestanden,
  sieben ignoriert, einschliesslich der sechs neuen Regressionstests.
- `cargo fmt --all --check`: bestanden.
- `cargo check --workspace --all-targets --locked -j2`: bestanden.
- `cargo clippy --workspace --all-targets --locked -j2 -- -D warnings`: bestanden.
- `cargo build --workspace --locked -j2`: Client und Server erfolgreich gebaut.
- Drei native neue Weltstarts samt 100%-Hold, Fade-in und Audio-Reihenfolge bestanden.

Kein neuer allgemeiner FPS-Gewinn oder Langzeitnachweis behauptet.

## Reproduktion

`target/debug/terra-client --smoke-test-loading` mit isoliertem
`TERRA_CONFIG_DIR`, `TERRA_MENU_CONFIG_DIR`, `TERRA_WORLD_DIR` und
`TERRA_CAPTURE_DIR`; `WGPU_BACKEND=vulkan`. Die Fixture legt ihre neue Welt
unter `TERRA_CAPTURE_DIR/worlds` an. Fuer den zweiten Lauf wurde nur der
World-Template-Seed von 42042 auf 7 geaendert. Der dritte Lauf verwendet den
nur lesend aus worlds/Test/world.toml entnommenen Seed, wiederum in einer
separaten neuen Welt; die Originalwelt wurde nicht geoeffnet oder veraendert.

Rohlogs und Captures sind lokal neben diesem Bericht abgelegt. Wie die
bisherigen Benchmark-Artefakte werden sie durch vorhandene Gitignore-Regeln
nicht automatisch versioniert.
