# Siedlungen, Bergminen und Struktur-Streaming

## Umsetzung

- F9 verwendet eine eigene Gizmo-Gruppe: orange Vertikalen, bernsteinfarbene
  Ebenen und gelbe aktuelle Section. Bildschirmbreite 2,7 statt 2 Pixel (+35 %),
  ohne andere Debug-/Auswahllinien zu veraendern.
- Neue Welten verwenden `structures_revision = 2`. Platz-, Strassen- und
  Hofgrundrisse variieren mit dem Seed; Gebaeude drehen sich zu ihren Wegen.
  Kirchen, Schmieden, Wachtuerme, Wohnhaeuser und Heilerhaeuser erhalten
  Veranden, Ueberhaenge, Fensterrahmen, Zaeune, Slabs und eigene Einrichtung.
  Holzgerahmte Giebel und passende Dachfenster gliedern die oberen Fassaden.
  Marktstaende und kleine Einfassungen ergaenzen geeignete Freiflaechen.
- Wege verzweigen an passenden ebenen Kreuzungen. Drei breite Fahrspuren
  erhalten konsistente Hoehen, ausgerichtete Dirt-Treppen und Dirt-Slab-Raender.
  Seitenfundamente pruefen ihren eigenen Boden, Wasser und Hoehlenhohlraeume.
  Vorhandene Treppen werden bei neuen Abzweigungen nicht ueberschrieben.
  Brueckendecks duerfen nach der Hoehenanpassung weder in der Mitte noch an
  einer Seitenspur vom Wasser erreicht werden; ungeeignete Plaene entfallen.
- Minen erhalten einen seitlichen, abgestuetzten Stollen in einen geeigneten
  Berghang. Natuerliche Dichte, trockene Zugaenge, Boden und Felsueberdeckung
  werden geprueft. Ohne geeigneten Hang entsteht keine Mine; keine oberirdische
  Huette mit nach unten gegrabenem Loch als Ersatz.
- IDs, Weltidentitaeten und bestehende gespeicherte Strukturvertraege bleiben
  erhalten. Revision 0 erzeugt weiterhin keine Strukturen; Revision 1 verwendet
  weiterhin ihren bisherigen Plan. Neue Grundrisse/Bergminen deshalb in einer
  neu erstellten Welt pruefen. Kein automatisches Ueberschreiben alter Doerfer.

## Performance-Korrekturen

`initialize_structures` behandelte bereits generierte Treppen/Furniture wie
interaktive Blockaenderungen: `record`, dann erneut `record` aus
`set_object_state`. Neben unnoetigen Wasser-/Gras-Wakeups erhoehte das die
Mesh-Epochen angrenzender Sections. Schon fertig berechnete Geschwister eines
Generierungsbatches wurden dadurch beim Veroeffentlichen als veraltet verworfen.

Die Erstregistrierung speichert kanonische Materialien, Ausrichtungen und
Inventare nun ohne diese falschen Terrain-Edits. Vorpruefung verhindert
teilweise uebernommene fehlerhafte Batches. Oeffentliche Persistenzrevisionen
bleiben gueltig. Nur bereits geladene Nachbar-Meshes erhalten gesammelt eine
Aktualisierung; echte Spieler-Edits behalten ihre bisherigen Invalidierungen.

Weiterhin:

- Gleiche Treppen-/Slab-Profile werden innerhalb eines Kompositionsjobs
  wiederverwendet. Maximal 64 Cacheeintraege, danach korrekt berechneter
  ungecachter Fallback. Der gesamte Cache stirbt mit dem Job; keine neuen
  Mesh-/Material-Assets und keine Qualitaetsreduktion.
- Punktabfragen benoetigen kein temporaeres Regions-Vec und keine neun
  Regionsproben mehr. Negative Koordinaten und beide Strukturversionen sind
  gegen den bisherigen Spaltenpfad geprueft.
- Tuer-/Fensterdarstellung betrachtet nur geladene Sections, inklusive einer
  geteilten Tuer mit ungeladenem Primaerteil. Oeffnungsgeraeusche betrachten
  maximal 64 Sections im bestehenden 20-m-Hoerbereich. Das Durchlaufen der
  Treppenzustaende aller frueher besuchten Doerfer entfaellt.

## Gemessene CPU-Vergleiche

Ryzen 9 5900X, vorhandenes optimiertes Testprofil. Drei Durchlaeufe, Werte sind
Mediane der gesamten angegebenen Abfragemenge, keine FPS-Hochrechnung:

| Isolierter Codepfad | Bisheriger Pfad | Neuer Pfad |
| --- | ---: | ---: |
| 200.000 warme Struktur-Punktabfragen | 21,390 ms | 14,403 ms |
| 20.000 identische Dach-Treppenprofile | 86,468 ms | 1,532 ms |

Reproduktion:

```bash
CARGO_BUILD_JOBS=1 cargo test --locked -p terra-world --features bevy --lib benchmark_point_lookup_without_region_vector_allocation -- --ignored --nocapture
CARGO_BUILD_JOBS=1 cargo test --locked -p terra-render --lib benchmark_repeated_roof_profiles -- --ignored --nocapture
```

Die Geometrie des gecachten und ungecachten Pfades wird einschliesslich
Materialslots, Indizes, Normalen und UVs auf exakte Gleichheit geprueft.

## Kurzer Nativer Flugvergleich

Ryzen 9 5900X, RX 6950 XT / RADV Vulkan, optimiertes vorhandenes Dev-Profil,
physisch 1280 x 800, Sichtweite 12, unbegrenzt/VSync aus. Je ein 45-s-Sample
nach der Ladephase; integrierter Server, echtes TiltUI-HUD und reale
Controller-Eingaben. Identischer Seed 42042 und `structures_revision = 1`
isolieren die Laufzeitkorrekturen von den neuen Dorfgrundrissen. Die Route
steigt auf und fliegt anschliessend rund 345 m mit Chunk-Streaming.

| Messwert | Vorher | Nachher |
| --- | ---: | ---: |
| Durchschnittliche FPS | 115,47 | 119,71 |
| p95-Framezeit | 14,210 ms | 13,744 ms |
| p99-Framezeit | 18,477 ms | 18,898 ms |
| Anfaengliche Ladezeit | 18,189 s | 15,979 s |
| Prozess-RSS am Ende | 1.692 MiB | 1.686 MiB |
| Terrain-Kompositionsjobs, inklusive Start | 6.867 | 6.674 |
| Terrain-Workerzeit, inklusive Start | 6,918 s | 6,091 s |
| `streaming.remesh_loaded`-Aufrufe, inklusive Start | 24 | 10 |

Beide Laeufe enden mit 6.928 geladenen Sections und 44.087 dargestellten Props;
die Sichtweite und Vegetationsdichte wurden nicht gesenkt. Die durchschnittliche
Framerate und p95 verbessern sich leicht, p99 verbessert sich in diesem
Einzelvergleich **nicht**. Das ist kein Beleg, dass alle Streaming-Spitzen oder
ein langfristiger FPS-Verfall beseitigt sind. Die gezielten Lebenszyklus- und
Geometrietests sichern die konkreten Fehlerkorrekturen unabhaengig davon ab.
GPU-Gesamtframezeit, Draw Calls und gemessener VRAM lagen hier nicht vor.
Rohdaten: `explore-before.log`, `explore-after.log`.

```bash
TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/settlement-streaming-2026-10-24/configs-legacy" \
TERRA_MENU_CONFIG_DIR="$PWD/target/phase123-explore-ui-menu" \
TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 TERRA_SOAK_ROUTE=explore \
TERRA_SOAK_REPORT=1 TERRA_BENCH_CAMERA=160,100,-635 \
TERRA_BENCH_LOOK_AT=160,93,-621 TERRA_BENCH_SECONDS=45 \
TERRA_CPU_SCOPES=1 WGPU_BACKEND=vulkan \
target/debug/terra-client --smoke-test-benchmark
```

## Reproduzierbare Regionen

Die nativen Strukturaufnahmen verwenden Seed `42042`, die beigefuegte
Konfiguration unter `configs/` und unveraenderte echte Generatorplaene:

| Szene | Strukturregion | Mittelpunkt / Mineneingang |
| --- | --- | --- |
| Plains/Forest | `[2, -2]` | `[948, 87, -589]` |
| Highlands | `[-1, -1]` | `[-185, 90, -152]` |
| Wueste | `[-6, -5]` | `[-2120, 31, -1707]` |
| Bergmine | `[1, 0]` | `[609, 170, 195]` |

`structure-sites.log` dokumentiert zusaetzlich die Suche nach allen drei
Dorfstilen, Ruinen und Minen fuer Seeds `1337` und `99173`. Die Doerfer wurden
nicht eigens fuer Screenshots von Hand gebaut. Beispielaufruf:

```bash
TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/settlement-streaming-2026-10-24/configs" \
TERRA_CAPTURE_DIR="$PWD/tools/benchmarks/results/settlement-streaming-2026-10-24/native/temperate" \
TERRA_STRUCTURE_SCENE=temperate TERRA_STRUCTURE_REGION=2,-2 \
WGPU_BACKEND=vulkan target/debug/terra-client --smoke-test-villages
```

## Validierung

Erfolgreich ausgefuehrt:

- `cargo fmt --all --check`
- `CARGO_BUILD_JOBS=1 cargo check --locked --workspace`
- `CARGO_BUILD_JOBS=1 cargo test --locked --workspace`: 2.357 bestanden,
  5 ignoriert, keine Fehler. Zwei ignorierte CPU-Mikrobenchmarks wurden
  zusaetzlich explizit ausgefuehrt und bestanden.
- Nach der abschliessenden Brueckenpruefung erneut
  `CARGO_BUILD_JOBS=1 cargo test --locked -p terra-world --features bevy --lib`:
  630 bestanden, 2 ignoriert, keine Fehler. Check, Clippy und Build ebenfalls
  auf diesem letzten Stand ausgefuehrt.
- `CARGO_BUILD_JOBS=1 cargo clippy --locked --workspace --all-targets -- -D warnings`
- `CARGO_BUILD_JOBS=1 cargo build --locked --workspace`

Die neue Regionspruefung findet alle drei Dorfstile in normalen Welten fuer
Seeds 42042, 1337 und 99173 innerhalb Regionsradius 12. Sie hat eine
zwischenzeitlich zu strenge Kreuzungspruefung aufgedeckt; die sichere
Wegverzweigung wurde danach korrigiert. Weitere Regressionen pruefen unter
anderem alle Dach-/Turmdrehungen, Pfadsteigungen in vier Richtungen, getrennte
Lade-Reihenfolgen, gespeicherte Weltrevisionen und Minen-Felsueberdeckung.

Native Vulkan-Pruefungen auf der RX 6950 XT bestanden:

- `--smoke-test-villages` fuer Temperate, Desert, Highland und Mine: je eine
  Uebersicht und Nahaufnahme aus dem echten Generatorplan. Die Aufnahmen wurden
  tatsaechlich angesehen, nicht nur anhand der Dateiexistenz bewertet.
- `--smoke-test-materials`: alle 105 registrierten Bloecke/Previews bereit;
  auch F9 mit der eigenen 2,7-Pixel-Gruppe im laufenden Renderer geprueft.
- `terra-server --no-save --bind 127.0.0.1:0 --smoke-test` ohne DISPLAY und
  WAYLAND_DISPLAY: 27 Sections erfolgreich, siehe `headless.log`.
- Keine Panics oder Laufzeitfehler waehrend der geprueften Szenen. Beim
  automatischen Beenden der Desert-/Minen-Fixture meldet Bevy nach beiden
  erfolgreich gespeicherten Screenshots noch `Failed to send screenshot:
  sending on a closed channel`; Exit-Code jeweils 0. Dieser verbliebene
  Screenshot-Shutdownfehler ist nicht als behoben ausgewiesen. Temperate,
  Highland, Materialtest, Flugvergleich und headless Start sind ohne solche
  Warnungen/Fehler abgeschlossen.

Aufnahmen:

- [Temperate: Grundriss](native/temperate/temperate-overview.png),
  [Hausdetails](native/temperate/temperate-close.png)
- [Desert: Grundriss](native/desert/desert-overview.png),
  [Hausdetails](native/desert/desert-close.png)
- [Highland: Grundriss](native/highland/highland-overview.png),
  [Hausdetails](native/highland/highland-close.png)
- [Mine: Berghang](native/mine/mine-overview.png),
  [Stollen](native/mine/mine-close.png)
- [Orange-gelbe F9-Grenzen](native/materials/materials-chunk-borders.png)

## Grenzen

Die Referenzen dienen der abwechslungsreichen Strassen-/Hofaufteilung und
detaillierteren Bauweise. Die Doerfer sind keine pixelidentischen Nachbauten:
vorhandene Blockaufloesung und Materialien bleiben erhalten. Neue Bewohner,
Banner-/Bootsmodelle oder neue Texturen wurden hier nicht eingefuehrt.
Alte gespeicherte Doerfer und Minen werden bewusst nicht neu gebaut.

Lange 10-/15-/30-Minuten-Spieltests werden auf ausdruecklichen Wunsch nicht
ausgefuehrt; kurze Tests sind kein Nachweis fuer Langzeitstabilitaet.
