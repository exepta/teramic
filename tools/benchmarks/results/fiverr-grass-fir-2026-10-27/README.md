# Originalgras und volle Tannenkronen

## Tatsaechliche Weltintegration

Grass-ID 1 verwendet jetzt in allen Welten das aufbereitete Fiverr-Modell.
Nicht nur die Vergleichsszene, sondern auch generierte und gespeicherte
Grassbloecke laufen durch diese Pipeline. Block-IDs, Kollision, Abbau und
Spielstanddaten bleiben unveraendert. Die Terrainprofile wurden in diesem
Schritt nicht erneut veraendert.

Die Originaldatei bleibt bytegleich (Git-Blob
`f048c4186320e95e406c693a4c15004079807cf8`). Das reproduzierbare
[Importwerkzeug](../../../../assets/terra/blocks/models/FIVERR_GRASS.md)
partitioniert die vorhandenen 768 Dreiecke in sechs sichtbarkeitsgesteuerte
Flaechengruppen. Original-UVs, PBR-Texturen und Material bleiben erhalten.
Der bestehende Joinbereich schliesst die Kanten benachbarter Bloecke.
Verdeckte Seiten werden nicht emittiert; es entstehen keine Block-Entities.

Tangenten werden durch Import, Transformation, Connected-Meshing, Worker-
Packing und GPU-Upload weitergereicht. Middle/Far und abgeschaltete
Micro-Geometrie verwenden flache, Greedy-kompatible Flaechen mit aus dem
Original gebackenen Base-/Normal-/ORM-Texturen, nicht die alten Turfmaterialien.
Die Texturen erhalten einmalig asynchrone Mipmaps; originale Wrap-/Filter-
Einstellungen bleiben erhalten. Near- und Coarse-Szenen halten die PBR-
Subassets, ohne als Terrain-Szenen gespawnt zu werden.

Biomfaerbung betrifft nur die Grasoberflaeche. Der bestehende Kalender- und
Schneeshader behandelt die gruene Atlasflaeche gesondert und uebermalt nicht
das ganze Modell mit dem alten prozeduralen Blockmuster.

## Tannen

Fir- und Spruce-Laub haben geschlossene dreidimensionale Kronenkoerper mit
kurzen, darin verwurzelten Nadelspitzen. Fichten behalten etwas schmalere
Spitzen; Tannen wirken dichter. Near und Middle behalten denselben
Laubkoerper; nur die Spitzenanzahl wird reduziert. Keine Alpha-Planes,
zusaetzlichen Entities oder neuen Materialinstanzen je Nadel.

Bei vollstaendig freistehendem Laub sinkt die maximale Vertexanzahl pro
Fir-Blattzelle von 720 auf 348 (Near) bzw. 432 auf 204 (Middle); bei Spruce
von 360/216 auf 348/204. Das ist eine
Geometriemessung, kein FPS-Benchmark. Die bestehenden begrenzten 1024
Templatevarianten, Exposure-Masken und Section-Batches bleiben erhalten.
Die erste native Aufnahme hatte die weiterhin duennen benachbarten Fichten
sichtbar gemacht; deshalb gilt der geschlossene Aufbau jetzt fuer beide
Nadelbaumarten. Weltgenerierung und Artenzuordnung bleiben unveraendert.

## Reproduktion

Grasvergleich: `terra-client --smoke-test-fiverr-grass`, links unveraendertes
Original, rechts echte Terrain-Edits. Tannentest:
`TERRA_BIOME_SCENE=connected-fir terra-client --smoke-test-biomes`.
Die begrenzte Suche bestaetigt Fir-Appearance und tatsaechliche Blatt-/
Stamm-Bloecke, bevor sie Nah- und Gesamtansicht aufnimmt. Fuer Seed 42042
liegt eine geeignete natuerliche Tanne bei `[4486, 166, -5756]`.

Die nativen Tests verwenden temporaere Konfigurationen und Offline-Welten;
Spielstaende werden nicht veraendert. Keine 10-/15-/30-Minuten-Tests und
kein neuer allgemeiner FPS-Nachweis in diesem Schritt.

## Pruefergebnisse

- `cargo fmt --all --check`: erfolgreich.
- `cargo check --locked --workspace --all-targets -j1`: erfolgreich.
- `cargo test --locked --workspace --all-targets -j1 -- --test-threads=2`:
  2738 bestanden, 7 ignoriert. Keine Fehler.
- `cargo clippy --locked --workspace --all-targets -j1 -- -D warnings`:
  erfolgreich.
- `cargo build --locked --workspace --bins --example import_block_faces -j1`:
  erfolgreich, einschliesslich Client und Dedicated Server.
- Importer `--check`: Derived-GLB und Provenienz bytegenau reproduziert.
- Original-GLB-Hash erneut unveraendert bestaetigt.

Die Regressionen pruefen unter anderem Originalmaterial/-UVs, Tangenten,
verdeckte Grasflaechen, geschlossene Nachbarkanten, Greedy-Fern-LOD,
negative Sectionkoordinaten, Material-/Mipmap-Bindings und originale Sampler.
Fuer beide Nadelbaumarten werden alle Exposure-Masken/Varianten/LODs,
geschlossene zusammenhaengende Kronen, Mindestvolumen und verwurzelte
Spitzen geprueft. Die natuerliche Tannenfundstelle hat einen eigenen Test.

Native Aufnahmen auf RX 6950 XT, RADV/Vulkan, 1280x800:
[Grasvergleich](grass-side.png), [Nadelbaumkronen](conifers-overview.png),
[Nahansicht](conifers-close.png). Alle Bilder wurden tatsaechlich angesehen.
Gras: drei bestandene GPU-Pixelpruefungen und echte benachbarte Welt-Edits.
Nadelbaeume: tatsaechlich generierte Fir-/Spruce-Bloecke, zwei fertige Ansichten.
Beide Fixtures Exit 0. Beim automatischen Beenden erschien gelegentlich noch
`Failed to send screenshot: sending on a closed channel` aus einer weiteren
ausstehenden Screenshot-Readback; die dokumentierten Aufnahmen und ihre
Bildpruefungen waren bereits vollstaendig. Dieser Shutdown-Hinweis ist nicht
als behobener allgemeiner Bevy-Fehler ausgegeben.

## Kurzer Kostenvergleich

Identische Konfiguration wie der vorherige Terrainlauf: Seed 42042,
Sichtweite 12, vertikal 4, hohe Schatten-/Wasserqualitaet, klares Wetter,
Kamera `[4300,155,310]` nach `[4352,110,256]`, 6 Sekunden nach Ladephase.
Keine parallel laufenden Cargo-Builds oder CPU-Probes. Ein Lauf je Stand,
also keine statistische Performancegarantie und kein Langzeittest.

| Messwert | Bisheriges Terrain-Gras | Neues Originalgras |
| --- | ---: | ---: |
| Durchschnitt FPS | 152.98 | 127.66 |
| Minimum FPS | 73.83 | 73.73 |
| p95 Framezeit ms | 8.91 | 10.53 |
| p99 Framezeit ms | 10.18 | 12.31 |
| Frames unter 32 FPS | 0 | 0 |
| Prozess-RAM KiB | 1382280 | 1510992 |
| Sichtbare Mesh-Entities | 3729 | 4652 |
| Mesh-Assets | 8393 | 10301 |
| Terrain-Composition Worker s gesamt | 1.826 | 2.067 |

Das neue Modell ist damit in dieser Probe rund 16.6% teurer bei AVG-FPS.
Die sechs unterschiedlichen Coarse-PBR-Faces sind keine identischen
Textur-/Materialduplikate und erzeugen zusaetzliche Section-Batches.
Der Austausch wird nicht als FPS-Optimierung dargestellt. Near-Geometrie
bleibt auf sichtbare Seiten und den bestehenden Detailradius begrenzt;
Far verwendet weiterhin Greedy-Quads. Einzelne GPU-Passzeiten und VRAM
wurden in diesem kurzen Lauf nicht gemessen.

Logs: `/tmp/teramic143-{tests-final-current,clippy-current,check-current,
build-current,import-current,grass-verified,fir-current,world-final}.log`.
Vorheriger Vergleich: `/tmp/teramic142-mountain-forest.log`.
