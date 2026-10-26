# Cherry Forest, KI, Beleuchtung Und Dachanschluesse

## Code

- Bewohner verwerfen festgefahrene Erkundungsziele nach ausbleibendem Fortschritt.
  Die gemeinsame Wegpruefung kontrolliert jetzt auch die Koerperausrichtung am
  Start der Kante. Zuvor konnte eine ein Meter lange Kante als frei gelten,
  obwohl schon der erste reale Schritt eine Drehung in den Tuerrahmen verlangte.
  Bewohner koennen sich kollisionsgeprueft ohne Koerperdrehung seitlich befreien.
  Wege aus niedrigen Vorspruengen behalten waehrend des Heruntersteigens ihre
  vertikale Geschwindigkeit; die bestehende gesweepte Schwerkraft bleibt aktiv.
  Auswege sind begrenzt und verfallen; keine Befreiungs-Teleports oder groesseren
  A*-Budgets. Die Wegplanung behaelt ihre konservative Vierpunkt-Bodenprobe.
- Mob-Darstellung puffert hoechstens acht Serverzeit-Snapshots. Ankunftsjitter
  veraendert nicht mehr direkt die Abspielgeschwindigkeit. Extrapolation endet
  nach maximal 120 ms/0,5 m; groessere verspaetete Korrekturen werden geglaettet.
  Eine Gehbewegung braucht nun auch tatsaechliche dargestellte Ortsveraenderung.
  Mehrere verspaetete Voll-Snapshots eines Frames werden nach Validierung auf
  den neuesten zusammengefasst, anstatt die Wiedergabe mehrfach neu zu starten.
- Fackeln geben ab vier Metern 50 Prozent mehr Umgebungslicht als vorher ab.
  Der Kern bis einen Meter bleibt unveraendert, ebenso Reichweite, Lichtanzahl
  und Schattenbudget. Das gilt auch fuer die vorhandenen Dorffackeln.
  Die begrenzte Quellenauswahl gewichtet verdeckte Innenraumquellen niedriger
  als sichtbare Weglichter. Ecklichter in der Naehe bleiben als Reserve zugelassen.
  Strukturrevision 4 hatte keine Wegfackeln. Neue Welten erhalten deshalb
  Strukturrevision 5 mit hoechstens 20 unterstuetzten Fackelpfosten im Abstand
  von mindestens 18 m, neben den vorhandenen Wegen und nicht in ihren Laufspuren,
  Tueren oder Haeusern. Alte gespeicherte Strukturrevisionen bleiben unveraendert.
  Der globale dynamische Lichtpool behaelt dieselbe Groesse.
- Die sichtbare Oberflaechen-Biomkennung trennt trockenes Land von echten
  Meeres-Wassersaeulen. Klima-Gewichte fuer Materialuebergaenge bleiben erhalten.
- F34 verwendet ein kontinuierliches, seed-basiertes Sakura-Klimafeld mit
  pinken/weissen Blattbloecken (neue IDs 132/133), verzweigten breiten Kronen,
  pinken/weissen Blumen und dezent rosa Wasser-/Kartenfarben. Pinke Blumen
  lassen sich pfluecken, wieder pflanzen und im Crusher zu Farbstoff verarbeiten.
  Die Bluetentextur ist neutral statt gruen getoent; die Blattform nutzt weiterhin
  dieselbe Maskierung, Detailgeometrie und Windbewegung. Der gruene saisonale
  Farbfilter wird nur fuer Blueten umgangen, nicht deren saisonaler Blattverlust.
  Neue sparse Blatt-IDs verwenden dichte Provenienzslots statt Material-ID-Offsets.
- Schmale Baeche in Cherry Forest, Forest und Highlands nutzen denselben
  gecachten Drainagepfad, Gelaendeschnitt, Wasserstand und Fluss-Renderer wie
  vorhandene Fluesse. Variable Breiten, flache Becken und abwaerts gerichtete
  Gefaelle; keine zweite Fluessigkeitssimulation oder dekorative Wasser-Plates.
  Ecology 1 aktiviert die kleinen Baeche auch ohne grosse `rivers`-Pfade;
  `water=false` deaktiviert beide.
- `ecology_revision=1` gilt fuer neue Welten. Fehlende Felder in alten Saves
  bleiben Revision 0. Dedizierte Saves behalten ihre eigene Revision; vorhandene
  Chunks, Block-IDs und Spielerbauten werden nicht stillschweigend umgeneriert.
- Zwei gegenueberliegende Treppen-/Slab-Dachprofile erhalten jetzt einen
  geschlossenen Anschluss an senkrechte Rohre, inklusive der beiden anderen
  Dachseiten. Normale Wandanschluesse brauchen weiterhin drei Nachbarn.
  Materialmischung, Dachprofil, Kollisionsformen und freier Rohrquerschnitt bleiben.

## Naechste FPS Schritte

Diese Vorschlaege sind bewusst nicht als bereits implementierte FPS-Gewinne
ausgegeben. Ausgangspunkt sind die nativen Messungen aus
`../npc-detail-performance-2026-10-25/README.md` und der aktuelle Code.

1. **Unveraenderte Preprocessing-Bindings wiederverwenden.** Im Nachtblick
   kostet `prepare_preprocess_bind_groups.body` noch etwa 1,54 ms CPU.
   Cache je View und Buffer-Generation; bei Reallocation, View-Wechsel und
   Unloading sauber invalidieren. Aufwand mittel; mit Schatten und Streaming
   separat gegenpruefen. Keine Grafikreduktion.
2. **Einzelne sehr grosse Uploads staffeln.** `upload_budget.rs` hat bereits
   Vertex- und Batchlimits. Der erste uebergrosse Batch darf sie aber bewusst
   ueberschreiten, damit die Queue nicht stehen bleibt. Genau diese Ausnahme
   gezielt mit gestuften GPU-Uploads und atomarer Aktivierung behandeln;
   interaktive Aenderungen priorisiert lassen. Ziel sind p95/p99-Spitzen beim
   Fliegen, nicht ein erfundener pauschaler Durchschnitts-FPS-Zuwachs.
3. **Stabile entfernte Sections groesser buendeln.** Die 4K-Dorfszene hatte
   rund 30.000 Mesh-Assets. Kleine gleichartige Fern-Materialbatches raeumlich
   zusammenfassen, bei lokalen Aenderungen begrenzt invalidieren und beim
   Unloading freigeben. Nahbereich, Kollision, Dichte und Sichtweite erhalten.
   Aufwand groesser; Draw Calls und Culling-Verluste vorher/nachher messen.
4. **Leere Punktlicht-Schattenviews frueher behandeln.** Die gebuendelten
   Encoder behalten alle sechs Cubemap-Seiten. Views ohne Caster koennen
   ihre Preprocessing-Arbeit sparen, muessen aber korrekt geloescht werden,
   damit keine alten Schatten stehen bleiben. Zuerst im Nacht-Dorf messen;
   kein pauschales Abschalten oder selteneres Aktualisieren bewegter Schatten.

## Validierung

### Reale KI-Reproduktion

Seed 42042, bestehende Strukturrevision 4, Dorfregion [-2, 2], Zentrum
[-574, 32, 962], Ecology 0. Der Test generiert die echten Sections/Haeuser,
initialisiert deren Objektzustaende und fuehrt 17 Bewohner 400 Schritte zu
je 0,1 s aus. Tueren werden wie in der Autoritaet paarweise geoeffnet.

Zwei Bewohner blieben vorher nach 2,16 bzw. 2,28 m dauerhaft haengen. Im
korrigierten Lauf legen dieselben IDs 22,68 bzw. 25,77 m zurueck. Alle 17
erreichen 11,19 bis 43,45 m, ohne einen horizontalen Einzelschritt ueber
0,25 m, ungueltigen Zustand oder einen abschliessenden Stillstand >=12 s.
Das sind **40 simulierte Sekunden**, kein behaupteter 40-s-GPU-Benchmark.
Der Test steht in `creatures/villagers/settlement_tests.rs`.

Die Wiedergabetests pruefen 600 Frames mit einer knapp einsekundigen
Paketpause und eine gezielte verspaetete Acht-Meter-Korrektur. Der Puffer
bleibt <=8, fehlende Pakete enden in einer stehenden Pose, die Korrektur
springt nicht sofort acht Meter. Das ist ein synthetischer Netzwerktest,
keine Behauptung einer nativen Mehrspieler-Paketverlustmessung.
Ein weiterer Test sendet drei Acht-Meter-Korrekturen waehrend der noch
laufenden Glaettung. Auch diese bleiben kontinuierlich, ohne dass normale
Folgepakete die bereits laufende Glaettung staendig neu starten.

### Native Darstellung

RX 6950 XT / Vulkan, optimierter Dev-Build, 1280x800. Keine parallel
laufenden Cargo-Builds. Die folgenden Screenshots wurden geoeffnet und
betrachtet, nicht nur automatisch abgelegt:

- `cherry/cherry-overview.png` und `cherry/cherry-close.png`: wirklich
  generierte pinke/weisse Kronen, Seed 42042, Region bei [-2192, 44, -3200].
- `brook/cherry-brook-overview.png` und `brook/cherry-brook-close.png`:
  generierter Bach mit echten Wasser-/Luftproben, Ufer und kleinen Stufen.
  Seed 42042, bei [-2103, 20, -3247]. Kein fuer die Aufnahme manuell
  eingesetzter Bach.
- `pipe-roofs.png`: vier reale Dach-/Rohrfaelle, einschliesslich gedrehter
  Treppen, gemischter Slabs und zweier gegenueberliegender Dachseiten.
  Geladene Voxel, Objektzustaende, aktuelle Mesh-Epochen und freier Rauchkanal
  werden vor der Aufnahme automatisch geprueft.
- `village-night.png`: die neue Wegbeleuchtung in Strukturrevision 5.
  `village-revision4-night.png` zeigt denselben Platz ohne nachtraegliches
  Retrogen der alten Revision. Diese Bilder vergleichen Beleuchtung, nicht
  identische Schattenlast oder eine FPS-Optimierung.

### Kurze Nachtmessung

Integrierter Server mit echter HUD, Seed 42042, 22:00 pausiert, High-Schatten,
Sichtweite 12. Kamera angefordert [-574, 39, 962], nach normaler Controller-
Simulation `[-574.0, 38.70, 962.0]`, Blickziel [-574, 33, 970]. Acht Sekunden
Messfenster nach 26,12 s initialem Laden/Aufwaermen; die Ladefrist von 240 s
ist keine Messdauer. Screenshot-benachbarte Frames werden ausgeschlossen.

| Finale beleuchtete Szene | Gemessen |
| --- | ---: |
| Durchschnitt | 60,72 FPS |
| p95 / p99 Framezeit | 20,277 / 23,760 ms |
| Prozess-RSS | 1895 MiB |
| Geladene / dargestellte Sections | 5625 / 2878 |
| Mesh-Assets | 11367 |
| Repraesentierte Props | 80732 |
| Punktlichter / mit Schatten | 8 / 8 |

`render.preprocess_bind_groups.body` kostet in diesem Lauf weiterhin 1,523 ms
im Mittel. Das bestaetigt den ersten Vorschlag oben, ist aber noch kein
implementierter Bind-Group-Cache. Kontrolllaeufe der alten Strukturrevision
lagen in dieser Arbeitsphase bei 60,67 und 55,88 FPS. Unterschiedliche
Quellenpositionen, laufende Tiere, geringfuegig andere Controllerhoehen und
Desktoplast machen daraus **kein** kontrolliertes Vorher-/Nachher-Paar.
Kein allgemeiner FPS-Gewinn, GPU-Gesamtframezeit, Draw-Call- oder VRAM-Verlauf
wird aus diesen Kurzlaeufen abgeleitet. A05 bleibt offen.

Die Terrain-/Sakuraformensprache ist an den Referenzen orientiert, keine
Behauptung identischer Pixel, Landschaft oder Beleuchtung. Die neuen Baeche
behalten den bestehenden regionalen Drainageaufbau; ein weltweites verbundenes
Flussnetz und laengere Fluesse bleiben F41/F42.

### Workspace

Erfolgreich: `cargo fmt --all --check`, `cargo check --locked --workspace`,
`cargo clippy --locked --workspace --all-targets -- -D warnings` und
`cargo test --locked --workspace --no-fail-fast -- --test-threads=2`:
**2492 bestanden**, kein
Fehler, sieben bewusst ignoriert. Der neue Weglicht-Test prueft begrenzte
Anzahl, echte Stuetze, unveraenderte Bewohner und deterministische Plaene;
alte Strukturrevisionen werden nicht nachtraeglich beleuchtet.
Weitere Regressionen decken negative Koordinaten, Saat-Determinismus,
Kartenfarben, Meeresklassifikation, Save-Vertraege, Rohrquerschnitte,
Shader-Kennungen und die neuen Blatt-Provenienzslots ab.

Im vorherigen unbeschraenkt parallelen Lauf hatte der bestehende UDP-Test
`invalid_movement_is_disconnected_without_affecting_the_other_player` einmal
seinen Fuenf-Sekunden-Timeout erreicht. Sofort isoliert und anschliessend in
der gesamten Suite mit zwei Testthreads bestand er. Der Ausreisser ist kein
nachgewiesener neuer Produktionsfehler; seine Ursache wurde nicht abschliessend
nachgewiesen. Die erfolgreichen Wiederholungen sind keine Zusage, dass der
urspruengliche Paralleltest damit dauerhaft frei von Timing-Ausreissern ist.

Auch `cargo build --locked --workspace` besteht. Der finale Dedicated Server
startet mit entferntem `DISPLAY` und `WAYLAND_DISPLAY`, laedt 27 Sections und
beendet `--smoke-test --no-save` erfolgreich. Keine Dependency-Upgrades und
keine Renderinitialisierung im Headless-Server. Die finalen nativen
Darstellungslaeufe melden keinen Shader-/Renderfehler.

Rohdaten im selben Ordner: `terra128-tests-green.log`,
`terra128-check-final.log`, `terra128-clippy.log`, `terra128-build-final.log`,
`terra128-native-cherry.log`, `terra128-native-brook.log`, `terra128-pipes.log`,
`terra128-night-revision5.log`, `terra128-server-headless.log`,
`terra128-playback-final.log` und `terra128-udp-retry.log`.
Die gezielte KI-Reproduktion steht in `ai-before.log` / `ai-after.log`.
Keine 10-, 15- oder 30-Minuten-Tests.

### Reproduktion

Nach `cargo build --locked --workspace`, aus dem Repository:

```bash
env TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/cherry-ai-light-2026-10-25/configs-cherry" \
  TERRA_CAPTURE_DIR=/tmp/teramic-cherry TERRA_BIOME_SCENE=cherry \
  TERRA_BENCH_GRAPHICS=1 WGPU_BACKEND=vulkan \
  target/debug/terra-client --smoke-test-biomes
```

Fuer den Bach `TERRA_BIOME_SCENE=cherry-brook` verwenden. Die Galerie laeuft
mit derselben Konfiguration, `TERRA_SMOKE_PIPE_ROOFS=1`,
`TERRA_SCREENSHOT=/tmp/teramic-pipe-roofs.png` und `--smoke-test-custom`.

Die neue Nachtbeleuchtung:

```bash
env TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/cherry-ai-light-2026-10-25/configs-night-lights" \
  TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 \
  TERRA_BENCH_CAMERA=-574,39,962 TERRA_BENCH_LOOK_AT=-574,33,970 \
  TERRA_BENCH_WEATHER=clear TERRA_BENCH_SECONDS=8 TERRA_BENCH_LOAD_SECONDS=240 \
  TERRA_CPU_SCOPES=1 WGPU_BACKEND=vulkan \
  target/debug/terra-client --smoke-test-benchmark
```

Gezielte KI-Reproduktion:

```bash
cargo test --locked --workspace \
  residents_really_explore_generated_houses_and_paths_without_walking_in_place \
  -- --nocapture
```
