# Grasfarbe und Streaming-Spitzen

## Reproduktion

Ryzen 9 5900X, RX 6950 XT, Vulkan/RADV, 1280x800, Sichtweite 20,
hohe Schatten-/Wasser-/Wetterqualitaet, unveraenderte Geometriedistanzen,
Micro-Geometrie und Props. Main-Landschaftsrevision 1, Meeresspiegel 8.
Alle FPS-Laeufe ohne parallele Compiler, Tests oder CPU-Benchmarks.
Keine 10-/15-/30-Minuten-Langzeitproben.

Die Offline-LOD-Fixture bewegt die echte Renderkamera samt Chunk-Fokus
zweimal hin und zurueck. Sie misst Rendering/Streaming, nicht autoritative
Spielerphysik oder das integrierte HUD. Die integrierte Standprobe nutzt
dagegen den echten Server, HUD und Minimap. Absolute Werte dieser beiden
Modi sind nicht unmittelbar vergleichbar.

Ein zusaetzlicher integrierter Explorationsversuch blieb unter einer
Baumkrone bei der Aufstiegsphase haengen. Er gilt ausdruecklich NICHT als
erfolgreicher Flugtest und wird nicht fuer einen Flug-FPS-Nachweis benutzt.

## Gemessene Ursachen

- Der Renderer hielt im Play-Modus an unbudgetierten Startup-Vollscans fest:
  `initial_ready` verlangte, dass alle CPU-Meshes verbraucht sind. Portal-
  ausgeblendete Hoehlensektionen behalten ihre Meshes jedoch absichtlich.
  Im Seed-42042-Flug: 1082 Vollscans, durchschnittlich 3.62 ms, im 32-s-Fenster.
  In der Standprobe: 30 Scans mit durchschnittlich 7.33 ms in 25 Sekunden.
- Einzelne Terrain-Materialmeshes erreichten 293942 Vertices, 21.16 MB
  Vertexdaten plus 1.20 MB Indizes. Das Vertexbudget erlaubte beliebig grosse
  erste Materialbatches. Zusammen wurden im Flug bis 359142 Vertices pro
  Frame uebergeben. Ein spaeterer grosser Buffer-Copy ist nicht automatisch
  identisch mit dieser CPU-Uebergabe; die Logs unterscheiden beides.
- Identische Gras-Nachbarformen berechneten Dreiecksnormalen, UV-Tangenten
  und deduplizierte Eckpunkte fuer jeden Block erneut.
- CustomMesh-Gras umging den bestehenden vereinfachten ShadowProxy-Pfad.
  Das ist ein konkreter unnoetiger Geometriepfad, aber kein separat
  nachgewiesener Anteil am FPS-Gewinn.

CPU-/GPU-Diagnose in der integrierten Standprobe: Main-Schedule-Wallzeit
11.14 ms, Render-Schedule 10.94 ms; einzelne GPU-Paesse ca. 1.83 ms
(Opaque) und 1.49 ms (Early Prepass). Verschachtelte/ueberlappende Zeiten
werden nicht addiert; daraus wird keine exakte Gesamt-GPU-Zeit abgeleitet.

## Codeaenderungen

- Die Startup-/Play-Umschaltung akzeptiert konservativ verborgene Meshes
  genauso wie der vorhandene Render-Ready-Vertrag. Sichtbare, noch nicht
  uebernommene Geometrie blockiert weiterhin. World-Switch-Reset bleibt aktiv.
- Pro Worker-Komposition begrenzter Cache fuer authored Face-/Nachbarmasken:
  maximal 384 Eintraege; keine globalen, unbegrenzt wachsenden Mesh-Caches.
- Worker teilen grosse Terrain-/Prop-Materialbatches in maximal 64000
  Vertices und 4 MiB. Alle Attribute, Dreiecke, Schattenmaterialien und
  Growth-/Foliage-Metadaten bleiben erhalten. Keine Riesenkopie im Main Thread.
- Upload-Admission zaehlt echte konservative Attribute-/Indexbytes zusaetzlich
  zum bisherigen Vertexbudget: 8 MiB Terrain, 6 MiB Props je Frame, keine
  unbegrenzte First-Batch-Ausnahme. Andere Domaenen behalten eigene Budgets.
- Nur validierte opaque authored Wuerfel erhalten die bestehenden sparsamen
  Schattenhuellen; sichtbare Farbgeometrie bleibt vollstaendig erhalten.
- Der Worker tauscht authored Gras ausserhalb der bestehenden Micro-Detail-
  Hysterese bereits gegen dieselben gebackenen PBR-Wuerfelflaechen aus.
  Diese flachen Near-Faces liefen trotzdem noch durch den Block-fuer-Block-
  Pfad statt Greedy-Meshing. Validierte opaque Fallbacks verwenden dort jetzt
  denselben Greedy-, Biometint- und Wetter-Split-Pfad wie Middle. Das aendert
  weder Detaildistanz noch Feinrelief oder Materialien. Originale CustomMesh-
  Nahgeometrie sowie ungepruefte/transparente Fallbacks bleiben unveraendert.
- Die Grasoberseite mischt ihre Albedo zu 78% mit hellerem ruhigem Gruen.
  22% Texturvariation, Normalmap und Relief bleiben. Biometint, Seiten,
  Erde/Steine, Jahreszeiten, Naesse und Schnee behalten ihre bisherigen Pfade.

## Verworfene Versuche

Bevys oeffentliches `NoCpuCulling` wurde isoliert mit GPU-Support-Check
gegen den damaligen Zwischenstand verglichen: 44.14 statt 47.91 AVG FPS,
p95 36.05 statt 33.25 ms. Die Zahl
sichtbarer Meshes stieg von 28527 auf 33497. Der komplette neue Versuchscode
wurde wieder entfernt; der Produktionspfad verwendet ihn nicht.

Ein weiterer isolierter Vergleich nutzte die bereits vorhandene Smoke-Option
`TERRA_MESH_SLAB_MIB=32` statt des Bevy-Defaults 512 MiB. Ergebnis: 23.66 AVG,
13.78 Min, p95 61.82 ms; Render-Schedule 41.39 statt 19.46 ms. Kleinere
Kopierspitzen loesten viel mehr Slabs, Bindings und Render-Batches aus.
Diese Einstellung wird NICHT fuer normales Spielen uebernommen. Die
Allocator-Wallzeit allein ist daher kein ausreichendes Optimierungsziel.

Auch der oeffentliche `PbrPlugin::use_gpu_instance_buffer_builder = false`
wurde als isolierte Smoke-Option ausprobiert. Dieser Lauf brach die
Bildintegritaetspruefung ab: `teramic::grass inventory preview remained blank
after shader readiness grace`. Es gibt dafuer keinen gueltigen FPS-Wert.
Die Pruefung wurde nicht umgangen. Der komplette neue Schalter wurde wieder
entfernt; Produktion und abschliessende Tests verwenden weiterhin den
normalen GPU-Aufbereitungspfad. Kein Patch an Bevy oder TiltUI.

## Native Vorher-/Nachher-Messungen

Die Zwischenmessungen enthalten Vollscan-, Cache-, Schatten- und
Uploadkorrekturen. Final enthaelt ausserdem den Coarse-Near-Greedy-Fix.
Gleiche Szenarien, Konfigurationen und Instrumentierung je Vergleich.

| Szene | Stand | AVG FPS | Min FPS | p95 ms | p99 ms | Frames unter 32 FPS |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Seed 42042, Flug, klar, 32 s | Vorher | 44.64 | 16.59 | 37.69 | 44.97 | 215 / 1429 |
| Seed 42042, Flug, klar, 32 s | Zwischenstand | 47.91 | 23.65 | 33.25 | 37.45 | 140 / 1533 |
| Seed 42042, Flug, klar, 32 s | Final | 46.72 | 24.49 | 33.31 | 36.97 | 139 / 1495 |
| Seed 7, Flug, Regen, 24 s | Vorher | 54.94 | 27.87 | 30.31 | 32.91 | 48 / 1318 |
| Seed 7, Flug, Regen, 24 s | Zwischenstand | 55.16 | 27.34 | 29.86 | 32.77 | 38 / 1323 |
| Seed 7, Flug, Regen, 24 s | Final | 55.37 | 27.63 | 30.07 | 33.10 | 44 / 1328 |
| Seed 42042, integriert, Stand, 25 s | Vorher | 78.18 | 26.41 | 21.03 | 27.70 | 6 / 1955 |
| Seed 42042, integriert, Stand, 25 s | Final | 80.02 | 34.82 | 19.56 | 21.58 | 0 / 2001 |

Diese Diagnose-Laeufe verwenden CPU-Scopes; die Standprobe zusaetzlich
GPU-Passdiagnostik. Nachmessungen verwenden jeweils identische Optionen.
Konfigurationen: `/tmp/teramic146-config` und `/tmp/teramic146-seed7-config`.
Rohlogs: `/tmp/teramic146-before-{lod,profile}.log`,
`/tmp/teramic146-before7-lod.log`, `/tmp/teramic146-after-lod.log` und
`/tmp/teramic146-after7-lod.log`.
Abschliessend: `/tmp/teramic146-final-lod.log`,
`/tmp/teramic146-final7-lod.log`, `/tmp/teramic146-final-profile.log`.
Kopien der Rohlogs liegen dauerhaft in [native](native/), die relevanten
Client-/Grafik-/Weltkonfigurationen in [configs](configs/).

Seed 42042, Zwischenstand: Vollscan-Mittelwert sinkt von 3.62 auf 0.64 ms; das begrenzte
inkrementelle Play-Sampling ist jetzt aktiv. Groesster einzelner Upload
bleibt unter 4 MiB statt ueber 22 MB. Peak-Vertices aller Upload-Domaenen
sinken von 359142 auf 220854. Die initiale Ladezeit bleibt bei 35.53/35.64 s,
der abschliessend gemessene RSS bei 2581372/2581364 KiB. Die Zahl sichtbarer
Meshes und die Grafikoptionen bleiben praktisch gleich.

Finaler Seed-42042-Flug: 212806 Peak-Vertices, 14432720 Peak-Uploadbytes ueber
alle Domaenen, 35.58 s initiales Laden, RSS 2605176 KiB. Im integrierten Stand
39.47 s initiales Laden und RSS 2494336 KiB statt 39.46 s/2542984 KiB vorher.
Das sind kurze Momentaufnahmen, kein Langzeit-/Leak-Nachweis.

Es wird kein grosser allgemeiner FPS-Gewinn behauptet: Beim zweiten Seed
bleibt AVG praktisch gleich, das schlechteste Einzelbild wird minimal
schlechter. Der Greedy-Schritt verbessert AVG nicht separat nachweisbar;
er entfernt dennoch redundante koplanare Geometrie unter Erhalt der
bestehenden Detaildistanz. Die feste Mindestgrenze 32 FPS wird in beiden
Fluegen weiterhin unterschritten. Verbleibende gemessene Last im
Seed-42042-Flug liegt vor allem im Renderpfad (Mesh-Allocator im Mittel
5.01 ms final beziehungsweise 4.84 ms im Zwischenstand und dynamisch neu
aufzubauende Preprocessing-Bindings). Keine pauschal reduzierte Grafik,
keine Framework-Patches, keine neuen Langzeittests und keine Behauptung
einer abgeschlossenen Streaming-/FPS-Abnahme.

## Validierung

- `cargo fmt --all --check`: bestanden.
- `cargo check --locked --workspace --all-targets -j1`: bestanden.
- `cargo test --locked --workspace -j1`: 2749 bestanden, 7 ignoriert,
  0 fehlgeschlagen. Der erste Lauf vor dem Greedy-Schritt hatte 2747 bestandene
  Tests. Neue Tests pruefen Cache-Aequivalenz fuer alle Nachbarmasken,
  atomare Teiluploads/Abbruchfreigabe, Growth-/Foliage-Metadaten, Bytegrenzen,
  Shadow-Fallbacks, Wetterdaten und negative Section-Koordinaten.
- Der neue Oberflaechen-Wettertest isoliert explizites Wetter von zusaetzlicher
  natuerlicher Cave-Feuchtigkeit. Deren bestehendes Sampling an Greedy-Teilen
  wurde nicht neu gestaltet; daraus wird kein lueckenloser Hoehlen-Feuchte-
  Nahtnachweis abgeleitet.
- `cargo clippy --locked --workspace --all-targets -j1 -- -D warnings`: bestanden.
- `cargo build --locked --workspace --bins -j1`: Client und Dedicated Server
  erfolgreich; keine Dependency-Aenderungen fuer diese Korrekturen.
- Drei native GPU-Aufnahmen mit Pixelpruefungen und anschliessender visueller
  Kontrolle bestanden: [vorne](native/fiverr-grass-front.png),
  [seitlich](native/fiverr-grass-side.png), [oben](native/fiverr-grass-top.png).
  Links das unveraenderte Original-FIVERR-Material, rechts das aktive Weltgras.
  Aufnahmen sind keine FPS-Messungen und wurden separat erstellt.
- Finale normale Renderlaeufe und Gras-Fixture ohne Errors/Warnungen.
