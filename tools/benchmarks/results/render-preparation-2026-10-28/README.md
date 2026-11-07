# Render-Aufbereitung: Prioritaeten 1 bis 3

## Versuchsaufbau

Ryzen 9 5900X, RX 6950 XT, Vulkan/RADV, 1280x800, Sichtweite 20,
hohe Schatten-/Wasser-/Wetterqualitaet. Gleiche Landschaftsrevision 1,
Meeresspiegel 8 und unveraenderte Detaildistanzen wie Phase 146.
Die Konfigurationen liegen in `configs/`, unverkuerzte Logs in `native/`.
Keine Compiler, Tests oder CPU-Benchmarks parallel zu FPS-Fenstern.
Nur kurze native Messfenster, keine 10-/15-/30-Minuten-Laeufe.

Die Offline-Fixture bewegt Renderkamera und Chunk-Fokus entlang derselben
Hin-/Rueckflugstrecke. Die integrierte Standprobe verwendet echten Server,
HUD und Minimap. Werte verschiedener Modi sind keine direkten A/B-Paare.
Alle Welten sind separate temporaere Testverzeichnisse, keine Spielerwelten.

## Tatsaechliche Korrekturen

1. `meshing/upload.rs`: Statische authored Terrain-Normalen und -Tangenten
   verwenden Bevys native Snorm16x2-Kompression. Normalen benoetigen 4 statt
   12 Byte, Tangenten 4 statt 16 Byte. Typischer authored Vertex: 72 -> 52
   Byte; Position, UV, Farbe, AABB, Dreiecke und Detaildistanzen unveraendert.
   Ungueltige Frames, bereits gepackte Normalen und Custom-Wind-/Wasserpfade
   behalten ihre bisherigen Formate. Die Uploadbudgets werden nicht erhoeht.
2. `meshing/shadows.rs`: Bereits flache authored Fallbacks behalten einen
   gemeinsamen opaque Schattenproxy, aber ohne unnoetige versenkte Huellen:
   4 Vertices/2 Dreiecke statt 8 Vertices/10 Dreiecke pro Quad. Detaillierte
   Nahflaechen und normale Stone-Huellen bleiben unveraendert. Geteilte
   Materialslots werden vor der Zusammenfassung nach Geometrietyp getrennt.
3. `draw_calls.rs`: Normales Gameplay verwendet einen kleinen Draw-Feedback-
   Wrapper ohne optionale Counter-/Readback-Ressourcen. Diagnostik bleibt
   explizit einschaltbar; erfolgreiche Color-Draws liefern weiterhin die
   bestehenden Reveal-Bestaetigungen.
4. `preprocess_buffers.rs`: Engine-eigene GPU-Instanz-Ausgabepuffer erhalten
   amortisierte Kapazitaeten statt exakter Neuallokation bei jedem Wachstum.
   Logische Laengen, Offsets und Dispatchzahlen bleiben unveraendert.
   Power-of-two-Planung, 64-KiB-Grain, maximal 32 MiB Padding pro Buffer und
   strikte Adapterlimits. Kein neuer Handle-Cache; Bevy bleibt Eigentuemer.
5. `preprocess_cache.rs`: Der eigene Bindingscache ist kein Produktionspfad
   mehr. Er loeste im Flug trotz stabiler Puffer fast jedes Frame neu aus,
   weil echte WorkItem-Laengen und Dispatch-Metadaten wechselten. Seine
   Signaturarbeit, Marker-Commands und zusaetzlichen Schedule-Barrieren
   kosteten mehr als die Wiederverwendung einsparte. Normales Spielen nutzt
   wieder den unveraenderten oeffentlichen Bevy-Helper.

Der Cache bleibt nur fuer reproduzierbare Smoke-Vergleiche mit
`TERRA_PREPROCESS_CACHE=1` erhalten. Sein begrenzter Zwei-Frame-Cache wurde
zusaetzlich auf phasenspezifische Invalidierung und getrennte Build-/Unpack-
Abhaengigkeiten korrigiert. Keine laengenabhaengigen Bindings oder echten
Dispatch-Offsets werden fuer vermeintliche Cachehits ignoriert.
Keine Vendor-/Registry-Patches, Dependencywechsel oder Qualitaetssenkung.

## Native Vergleiche

Die ersten Nachher-Laeufe verwenden denselben spaeteren Stock-Produktionspfad
ueber den zuvor vorhandenen Testschalter `TERRA_PREPROCESS_CACHE=0`.

| Szene | Stand | AVG FPS | Min FPS | p95 ms | p99 ms | Frames unter 32 FPS |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Flug 42042, klar, 32 s | vorher | 47.25 | 22.23 | 34.45 | 39.72 | 166 / 1512 |
| Flug 42042, klar, 32 s | Kompression/Proxy/Reserve, Cache noch an | 46.35 | 21.21 | 34.45 | 38.10 | 168 / 1483 |
| Flug 42042, klar, 32 s | Stock-Bindings | 50.38 | 23.00 | 32.08 | 36.15 | 96 / 1612 |
| Flug 42042, klar, 32 s | Neuer Produktionsdefault, Wiederholung | 49.75 | 23.43 | 32.26 | 37.88 | 102 / 1592 |
| Flug 7, Wasser/Regen, 24 s | vorher | 55.42 | 26.48 | 29.92 | 33.33 | 38 / 1330 |
| Flug 7, Wasser/Regen, 24 s | Stock-Bindings | 59.75 | 30.05 | 27.25 | 30.14 | 4 / 1434 |
| Integrierter Stand 42042, 25 s | vorher | 81.35 | 32.43 | 19.70 | 21.78 | 0 / 2034 |
| Integrierter Stand 42042, 25 s | Neuer Produktionsdefault | 84.90 | 35.88 | 18.39 | 20.42 | 0 / 2123 |

CPU-Wallintervalle im gleichen klaren Flug, keine additiven Kostenanteile:

| Intervall | Vorher ms | Neue Geometrie + Cache ms | Stock-Bindings ms | Produktionswiederholung ms |
| --- | ---: | ---: | ---: | ---: |
| Mesh-/Asset-Phase | 5.20 | 5.07 | 4.79 | 5.01 |
| Exakter MeshAllocator-Body | 4.74 | 4.46 | 4.26 | 4.33 |
| Bindings-Phase | 3.88 | 4.33 | 2.57 | 2.78 |
| Render-Ausfuehrung | 5.63 | 5.60 | 5.86 | 5.80 |

Fuer Render-Ausfuehrung ist somit kein isolierter CPU-Zeitgewinn nachgewiesen.
Die Korrektur spart dort Geometrie/Diagnoseabfragen, aber eine kuerzere
Render-Ausfuehrung wird nicht behauptet. Die untersuchten Meshzahlen sind
keine Draw-Call-Zahlen: ein separater Counterlauf meldet zuletzt 349
indirekte API-Submissions, davon 310 GPU-counted. Deren tatsaechliche
GPU-Unterdrawzahlen wurden nicht aus Entityzahlen geraten.

## Verworfene Versuche

- Komplettes Weglassen des coarse Schattenproxies: 46.23 AVG FPS. Die Zahl
  sichtbarer MeshEntities stieg von ca. 28528 auf 29689, weil mehrere
  Farbslots wieder eigene Schatten-Caster wurden. Zurueckgenommen; der
  gemeinsame Proxy bleibt, nur seine redundante Huelle entfaellt.
- Mesh-Slab-Minimum 16 MiB und Wachstumsfaktor 2 bei unveraendertem Maximum:
  44.52 AVG, 22.12 Min FPS. Der neue Versuchscode wurde komplett entfernt.
- Exakte Vertex-Deduplication im Grasasset: nur 3 von 2374 Nah-Vertices
  doppelt (0.126 Prozent); kein sinnvoller grosser Uploadhebel, nicht eingebaut.

## Grenzen

Die Mindestgrenze 32 FPS ist im Flug weiterhin NICHT erreicht. Der erste klare
Stock-Lauf lud anfangs 40.71 statt 35.56 Sekunden; der Regenlauf 24.56 statt
24.77 Sekunden. Die Produktionswiederholung lud in 35.48 Sekunden. Der
40.71-s-Ausreisser bleibt dokumentiert, ist aber kein reproduzierbarer
genereller Rueckschritt. Es wird auch kein genereller Ladezeitgewinn behauptet.
Keine neue Langzeit-, 4K-, Mehrrechner- oder weitere GPU-Abnahme.

## Validierung

- `cargo fmt --all --check`: bestanden.
- `cargo check --locked --workspace --all-targets -j1`: bestanden.
- `cargo clippy --locked --workspace --all-targets -j1 -- -D warnings`: bestanden.
- `cargo build --locked --workspace --bins -j1`: Client und Dedicated Server gebaut.
- `cargo test --locked --workspace -j1 -- --test-threads=2`: 2766 bestanden,
  7 bestehende ignoriert, keine neuen Tests ignoriert.

Die erste Zwischenfassung bestand bereits die komplette Workspace-Suite mit
2765 Tests. Die erste Abschlussrunde mit voller Standardparallelitaet traf
jedoch das bestehende 15-Sekunden-Zeitlimit in
`shoreline_water_rebuilds_after_bank_edits_and_section_reload`.
Das betreffende Testfixture und seine Assertions wurden NICHT veraendert.
Die gesamte finale Suite wurde mit zwei Testthreads wiederholt und bestand
einschliesslich dieses Tests. Beide Abschlusslogs liegen unverkuerzt bei.

Der Produktionsdefault wurde ohne Cache-Testschalter erneut nativ geprueft:
klarer Hin-/Rueckflug, integrierter Stand mit HUD/Minimap und Grasgalerie.
Alle abschliessenden nativen Laeufe ohne Warnungen, Renderfehler oder Panics.
Die Galerie bestand GPU-Pixelpruefungen und wurde in Front-, Seiten- und
Draufsicht tatsaechlich angesehen. Keine fehlenden Texturen oder sichtbaren
neuen Normalen-/UV-Artefakte; dies ist keine neue 1:1-Art-Abnahme.

- [Front](native/fiverr-grass-front.png)
- [Seite](native/fiverr-grass-side.png)
- [Oben](native/fiverr-grass-top.png)

Beispiel fuer die klare Produktionsprobe (mit separatem Testwelt-Verzeichnis):

```bash
env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET WINIT_X11_SCALE_FACTOR=1 \
  TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/render-preparation-2026-10-28/configs" \
  TERRA_WORLD_DIR=/tmp/teramic-render-preparation-repro \
  TERRA_BENCH_GRAPHICS=1 TERRA_BENCH_SECONDS=32 TERRA_BENCH_LOAD_SECONDS=120 \
  TERRA_CPU_SCOPES=1 WGPU_BACKEND=vulkan \
  target/debug/terra-client --smoke-test-lod
```

Der explizite Cache-Vergleich ergaenzt nur `TERRA_PREPROCESS_CACHE=1`.
Fuer Seed 7 eine Kopie der Konfiguration mit `world-seed7.toml` als
`world.toml`, `TERRA_BENCH_SECONDS=24` und `TERRA_BENCH_WEATHER=rain` nutzen.
