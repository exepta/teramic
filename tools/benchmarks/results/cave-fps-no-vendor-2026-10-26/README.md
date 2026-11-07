# Hoehlenausblick, FPS-Erfassung Und Dependencies (Phase 134)

## Stand

Bevy 0.20.0 kommt unveraendert aus crates.io. `vendor/` und alle Cargo-
Quellpatches sind entfernt. TiltUI-Fixes liegen im echten Repository unter
`../../tilt-us/tilt-ui`, Branch `preview-020`, nicht in einer Teramic-Kopie.
Die beiden TiltUI-Workspace-Dependencies verwenden vorerst diesen lokalen
Pfad. Die Aenderungen sind noch nicht auf GitHub veroeffentlicht. Ein fremder
Checkout benoetigt deshalb auch das benachbarte TiltUI-Repository mit diesen
Aenderungen; der alte Git-Commit alleine enthaelt sie noch nicht.

Dies ist **kein abgeschlossener allgemeiner Low-FPS-Fix**. Die unten gemessenen
kurzen Vergleiche zeigen keinen belastbaren Gesamt-FPS-Gewinn. A05 bleibt offen.
Sichtweite, Schattenseiten, Vegetationsdichte und Wasserqualitaet wurden nicht
zur Leistungssteigerung reduziert. Keine neuen Langzeitlaeufe durchgefuehrt.

## Codekorrekturen

- Session-Minimum/-Maximum warten zehn reale Sekunden ab `SessionInfo.started`,
  das nach der Ladeeinblendung gesetzt wird. Nur vollstaendig danach liegende
  Frameintervalle zaehlen: Auch ein langer Frame von 9,8 bis 12 Sekunden bleibt
  ausgeschlossen. Menue-/Weltwechsel setzen zurueck; Inventar und Pause nicht.
  Reale, ungeklemmte Framezeit statt virtueller Spielzeit erfasst echte Stalls.
- `MeshQueue::remove_unloaded` dedupliziert Section-Unloads und bereinigt die
  Warteschlangen einmal pro Nachrichtenbatch. Zuvor wurde die Positionsliste
  fuer jede einzelne entladene Section erneut durchlaufen. Bereits neu geladene
  Sections bleiben erhalten. Der Regressionstest zaehlt exakt 25.600 Abfragen
  fuer 8192 Queue-Eintraege und 1024 unterschiedliche Removal-Nachrichten,
  unabhaengig von doppelten Nachrichten; kein Scan bei nur neu geladenen Sections.
- Temporaere Bindings-/Gruppierungsmarker nutzen SparseSet statt Table-Storage.
  Gruppierte Punktlicht-Schattenansichten behalten RootNonCameraView; eine leere
  Geschwister-Schedule verhindert doppelte Zeichnung, ohne die dichte
  ExtractedView-Iteration durch Tabellenwechsel umzusortieren. Alle sechs Seiten
  bleiben erhalten. Fehlerpfad und naechster Frame stellen Core3d sicher wieder her.
  Die Tests decken stabile Reihenfolge und genau einmalige Fallback-Zeichnung ab.
- Konstante Lampen schreiben PointLight und Transform nur bei geaenderten
  Sollwerten. Das vermeidet unnoetige Change-Detection und erneute Frustum-
  Vorbereitung. Flackern und Reaktion auf geaenderte Belichtung bleiben erhalten.
- TiltUI: Borrowing/Shared-Snapshots statt vollstaendiger JSON-Kopien pro Binding,
  geparkte UI-Zweige, korrekte reaktive Phasen, begrenzte Hierarchietiefen-
  Berechnung sowie Pointer-/Tooltip-Korrekturen sind in das reale TiltUI-Repo
  uebertragen. Teramic nutzt oeffentliche ParkedUi-/ParkedUiBindings-Exporte.

Kein Aussetzen des Stock-Bevy-Uploads: write_binned_instance_buffers baut auch
Unpacking-Jobs und globale Metadata-Offsets neu auf und setzt GPU-beschriebene
Daten zurueck. Den ganzen Aufruf zu cachen waere inkorrekt. Keine Aenderungen an
Cargo-Registry-Quellen oder am Renderer-Crate vorgenommen.

## Hoehlenlicht

Die bisherige Implementierung verdunkelte die gesamte Ansicht gleich dreimal:
kameraglobal ausgeschaltete Sonne/Mond, schwarzer Nahnebel und schwarze
Himmelspixel im Cave-Fullscreenpass. Das betraf auch den Blick aus einer Hoehle.

Direktlicht bleibt jetzt belichtungsneutral erhalten; vorhandene Dachschatten
sperren es in geschlossenen Raeumen. Aussen-/Wassernebel bleiben erhalten und
der Cave-Shader schwaerzt den Himmel nicht mehr. Der reine Schwarzblenden-Pass
entfaellt, wenn kein anderer Effekt erforderlich ist. Ambient/IBL werden
weiterhin lokal reduziert; Unterwasser-Caustics bleiben in geschlossenen
Hoehlen abgeschaltet. Keine neue GI-/Voxel-Skylight-Engine und keine neuen
Punktlicht-Entities.

Grenzen: Abgeschaltete Schatten oder ein noch nicht geladenes Dach koennen
weiterhin Lichtlecks erzeugen. Ambient/IBL bleiben kamerabezogen; daher ist die
Aussenansicht von innen nicht in jedem indirekten Lichtbeitrag pixelidentisch.

## Native Messungen

Ryzen 9 5900X, RX 6950 XT, Vulkan/RADV, Mesa 26.2.5. Optimiertes Dev-Profil,
1280x800, VSync aus, Sichtweite 12 sofern nicht anders angegeben.
Keine parallelen Builds/Tests waehrend der
FPS-Messfenster. Beide Seiten sind Bevy 0.20; vorher ist die zu Beginn gesicherte
Binary mit den inzwischen entfernten Vendor-Patches. Deshalb kein isolierter
Mikrobenchmark nur eines Code-Fixes, sondern Vergleich des gesamten Aenderungsstands.
Gleiche Root-Assets; der Cave-Effekt ist in diesen oberirdischen Szenen inaktiv.

| Szene | Stand | FPS Mittel | p95 ms | p99 ms | Schlechtester Frame ms | Laden s | RSS KiB |
|---|---|---:|---:|---:|---:|---:|---:|
| Dorf nachts, Seed 42042, 8 s | Vorher | 61.33 | 20.43 | 21.74 | 22.47 | 23.89 | 2101888 |
| Gleiches Dorf | Ohne Vendor, stabile Views | 59.91 | 20.22 | 21.66 | 22.67 | 24.75 | 2122052 |
| Wasser-/Regenflug, Seed 7, 24 s | Vorher | 100.45 | 15.10 | 18.09 | 28.13 | 8.57 | 1305144 |
| Gleicher Hin-/Rueckflug | Ohne Vendor, Batch-Unload | 101.26 | 15.16 | 17.37 | 30.47 | 8.63 | 1332892 |
| Regenflug, Sichtweite 20, Seed 7, 24 s | Vorher | 70.36 | 25.53 | 27.96 | 39.16 | 19.83 | 1732272 |
| Gleicher Flug, Sichtweite 20 | Abschliessender Build ohne Vendor | 70.77 | 24.89 | 27.15 | 40.53 | 19.91 | 1743628 |

Dorf: 5625 geladene / 2878 dargestellte Sections, 11386 Mesh-Assets, 80732
dargestellte Props, acht schattenwerfende Punktlichter. Flug: 6574 / 1336
Sections, 7543 bzw. 7546 Mesh-Assets am asynchronen Routenende, 10151 Props,
625 Wasser-Tiles. Kleine zeitliche Async-Unterschiede sind ausgewiesen.
Bei Sichtweite 20 auf beiden Seiten: 17512 / 3501 Sections, 18246 Mesh-Assets,
27522 Props und 1681 Wasser-Tiles. Der geringe Mittelwert-/p95-Unterschied
ist kein belastbarer allgemeiner FPS-Gewinn, der schlechteste Frame bleibt
schlechter. Das Low-FPS-Problem ist auch hier nicht als erledigt abgenommen.

Die reine Bindings-Vorbereitung im Dorf sinkt von 2,765 auf 0,764 ms pro Frame.
Gleichzeitig kostet Stock-Shadow-Bin-Vorbereitung 2,589 statt 1,200 ms; Graph-
Abschluss/Submit 7,452 statt 6,788 ms. Diese Wall-Intervalle koennen sich
ueberlappen und enthalten Treiber-/Scheduler-Wartezeit, sind keine GPU-Framezeit.
Der Bindings-Fix alleine beseitigt die Bevy-0.20-Dorfregression nicht.

Ein separater GPU-Diagnoselauf (nicht fuer die FPS-Tabelle verwendet) erfasst
neun Pass-Timing-Serien: u.a. Irradiance 0,602 ms, Radiance 0,317 ms. Es fehlt
eine vollstaendige instrumentierte Sicht-/Schatten-GPU-Summe. Daraus wird kein
Gesamt-GPU-Bottleneck oder vermeintlicher Draw-Call-Gewinn abgeleitet.

## Native Hoehlenpruefung

`TERRA_CAVE_PORTAL_INSPECT=1 --smoke-test-caves` prueft dieselben Outdoor-Ziele
von innerhalb und ausserhalb einer handgebauten Dach-/Portalfixture sowie
geschlossene Fackel- und Unterwasseransichten bei festem Mittag (phase 0,5).
Alle vier Aufnahmen gespeichert, Exit 0; Screenshot innen/aussen und geschlossene
Ansicht visuell betrachtet. Dies ist eine reproduzierbare Lichtfixture, kein
Referenzbild-Abgleich prozeduraler Hoehlen. Ihre weitreichenden Terrain-
Overrides unterdruecken keine Oberflaechen-Props; deshalb sind ausserhalb der
Testwand schwebende Fixture-Props sichtbar, nicht als korrekte Weltgeometrie abgenommen.

Ein erster Bodenvergleich wurde verworfen: Ein neuer Raycast-Test erkannte den
Messpunkt .995 m innerhalb des Bodenblocks. Die Projektionen trafen dadurch
unterschiedliche Stellen. Die korrigierte Fixture verwendet ein gemeinsames
Ziel .005 m ueber der tatsaechlichen Bodenoberseite; der strenge Test verlangt
von beiden Kameras denselben getroffenen Block. Die native Wiederholung mit
dieser Korrektur ist bestanden (`teramic134-caves-final.log`), ihre Bilder
innen/aussen wurden erneut betrachtet. Gueltige linear-tonemapped Messwerte:

| Blick | Cave-Blend | Boden | Himmel |
|---|---:|---:|---:|
| Innen, Stage 18 | 1,0 | 0,191414 | 0,372243 |
| Aussen, Stage 19 | 0,0 | 0,245654 | 0,377534 |

Innen erreicht der Boden 77,9 %, der Himmel 98,6 % der Aussenhelligkeit.
Geschlossene Fackelansicht: Mittelhelligkeit 0,01747, ueberstrahlte Pixel 0,138 %;
Unterwasser: 0,00940 und 0,012 %. EV100 innen bleibt 6,451563. Die Hoehle wird
nicht global taghell gemacht. Keine Shaderfehler oder Slab-Use-after-free.
Im ersten Lauf erschien nach erfolgreichen Captures beim App-Exit die bekannte
Screenshot-Abschlussmeldung ueber einen geschlossenen Kanal. Die abschliessende
Wiederholung hatte diese Meldung nicht; ein allgemeiner Fix dafuer wird nicht behauptet.

## Reproduktion

Flug: `target/debug/terra-client --smoke-test-lod` mit
`TERRA_CONFIG_DIR=tools/benchmarks/results/tree-structures-performance-2026-10-25/configs-seed7`,
`TERRA_BENCH_GRAPHICS=1`, `TERRA_BENCH_WEATHER=rain`, `TERRA_BENCH_SECONDS=24`,
`TERRA_BENCH_LOAD_SECONDS=240`, `TERRA_CPU_SCOPES=1`, `WGPU_BACKEND=vulkan`.
Sichtweite 20 erfordert sowohl client.toml view_distance als auch graphics.toml
render_distance auf 20; die Graphics-Einstellung hat Vorrang.

Dorf: `--smoke-test-benchmark`, night-lights-Konfiguration aus Phase 127,
`TERRA_BENCH_MODE=integrated`, `TERRA_BENCH_UI=1`, Kamera `-574,39,962`, Blickziel
`-574,33,970`, klares Wetter, acht Sekunden. Alle Testwelten/-Menues liegen in
separaten temporaeren Verzeichnissen, keine bestehenden Spielstaende veraendert.
`TERRA_BENCH_DEBUG_OVERLAY=1` prueft gefuellte Session-Extrema am nativen Testende;
die exakte Zehn-Sekunden-Grenze wird durch Unit-Tests geprueft.

## Validierung

- `cargo fmt --all --check`: erfolgreich, auch im echten TiltUI-Repository.
- `cargo check --workspace --all-targets --locked -j2`: erfolgreich.
- `cargo test --workspace --locked -j2 -- --test-threads=2`: 2534 bestanden,
  sieben ignoriert. Einschliesslich aller neuen FPS-, Queue-, Schatten-,
  Change-Detection- und Portal-Regressionstests.
  Im ersten Lauf mit unbeschraenkten Testthreads ueberschritt ein vorhandener
  Shoreline-Streamingtest sein 15-s-Limit. Die vollstaendige Wiederholung mit
  zwei Testthreads bestand ohne Aenderung des Limits oder des Streamingcodes.
- `cargo clippy --workspace --all-targets --locked -j2 -- -D warnings` und
  `cargo build --workspace --locked -j2`: erfolgreich; Client und Server gebaut.
- `cargo test -p tilt-ui-runtime --lib --locked -j2`: 322 bestanden, direkt
  aus dem separaten TiltUI-Quellverzeichnis. Dieselbe Runtime besteht auch
  striktes Clippy mit `-p tilt-ui-runtime --all-targets`. Kein Test einer
  inzwischen entfernten Vendor-Kopie als Upstream-Test ausgegeben.
- Native Hoehlen-/Portalansichten und integriertes F1-Overlay bestanden.
  F1 meldete gefuellte Session-Extrema nach der Anlaufphase. Die genauen
  10-s- und uebergreifenden Framegrenzen werden in Unit-Tests geprueft.
- `--smoke-test-inventory` mit `TERRA_NATIVE_OS_POINTER=1`: elf Captures,
  reale Mausbedienung, Katalog, Ziehen/Teilen/Verteilen, Tooltips, Muelltonne,
  geparkte/vorgewaermte Inventarhuelle sowie erhaltene HUD-/Minimap-Entities
  bei Inventar/Pause/Optionen bestanden, Exit 0. Das ist eine Funktionspruefung,
  kein zusaetzlicher Inventar-FPS-Vergleich.

Der Sichtweite-20-Vergleich ist abgeschlossen. Kein allgemeiner FPS-
oder Langzeit-Abschluss behauptet. Die messbare Bindings-Verbesserung ersetzt
keine weitere Arbeit an den verbleibenden Stock-PBR-/Submit-/Uploadkosten.
