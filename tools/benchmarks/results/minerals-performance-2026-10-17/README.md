# Phase 53: Testprofile, Mineralien Und Renderkosten

## Umfang Und Kompatibilitaet

- `--profile tester-a` / `--profile tester-b` waehlen unterschiedliche persistente
  UUID-Dateien unter `configs/profiles/`. Standardprofil und serverseitige
  Duplicate-UUID-Pruefung bleiben erhalten. Das ist kein Online-Account-Verifier.
- Stein und alle sechs Erz-Felskerne teilen `stone-strata-hd.png`: matte, leicht
  warme Bruchflaechen, 1254 x 1254, elf asynchron erzeugte Mips. Keine neuen GLBs
  oder Materialien pro Voxel. [Bildgenerierungsmodus, Prompt und Herkunft](../../../../assets/terra/blocks/textures/stone-strata-hd.md).
- Fuenf Nicht-Salz-Erze besitzen verzweigte dunkle Bruchbetten und helle schmale
  Mineraleinlagen. Near/Middle/Far behalten 2/1/1 primaere Kristalle; zusaetzliche
  Flecken sinken auf 2/1/0. Maximal 64 Near-Dreiecke pro Face mit Adern statt der
  bisherigen 52 ohne Adern beziehungsweise historischen 104. Rendering und
  Picking teilen dieselben kanonischen Dreiecke. Der Hostwuerfel bleibt dicht;
  keine offenen Sichtloecher und keine neue Emissions-/Lichtsimulation.
- Neue Welten speichern Geologievertrag 2. Dripstonekammern erhalten echte
  mehrblockige Boden-/Deckenformationen; ihre deterministischen Zentren, Laengen,
  Radien und Supporthoehen werden begrenzt gecacht. Die vorhandene Cave-Dichte
  wird um diese Formen erweitert. Keine zweite Chunk-/Hoehlenpipeline.
- Salzformationen besitzen den hellen Koerperblock `teramic::salt_formation` (38).
  Abbau erzeugt die vorhandene Salzressource. `teramic::dripstone_formation` (39)
  teilt das Material der normalen Dripstonewand (37), nicht deren Geometrie:
  breite Spitzen wachsen an Formationen, die Waende behalten vereinzelte kleine
  Anbauten statt gleichfoermiger grosser Spitzen auf jedem Vorsprung.
  Alte IDs 0..37 bleiben unveraendert.
  Vertrag 0/1 wird beim Laden nicht hochgestuft. Kein Retrogen; fuer neue
  Formationen eine neue Welt anlegen. Generator 5, Weltformat 2 und Region-/
  Spielerformate bleiben bestehen. Client und Server verwenden Protokoll 27.

## Optimierung

Nahe Terrain-Cubes verwenden Schatten-only-Greedy-Schalen. Opaque Schalen teilen
einen positionsbasierten Section-Batch, unabhaengig von ihren Farbmaterialien.
Laub behaelt eigene Masken-/Windmaterialien. Der Farbpass behaelt seine bisherigen
Details und seine logischen Props. Glas, Wasser, Spezialmodelle und mineralische
Details mit eigenem Material bleiben aus dem Ersatzpfad ausgenommen. Proxies
erhalten weder normale Farbpass-Draw-Erwartungen noch temporaere Fade-Materialien.
Die Queue-/Schattenfreigabe und Abschnittstransformation gelten weiterhin.
`TERRA_BENCH_FULL_SHADOWS=1` waehlt ausschliesslich fuer Smoke-Fixtures den
Vollgeometrie-Vergleich. Kein regulaerer Schalter zum Entfernen von Vegetation.

Leere Uploadqueues vermeiden wiederholte Terrain-/Prop-Scans, solange die
geladenen Sections und Renderdaten unveraendert sind. Laufende Jobs und
Chunk-/Edit-/Reload-Aenderungen werden weiterhin verarbeitet.
Die erweiterte Hoehlenbeschreibung wird im Voxel-Hotpath ueber Referenzen gelesen,
statt sie fuer Dichte-/Kammerabfragen wiederholt zu kopieren. Geologie 0/1 ruft
die dort nicht vorhandene Formationsdichte nicht auf. Dichteformeln und
Blockauswahl bleiben dabei unveraendert.

Die groben Schatten bewahren die Voxel-/Kronensilhouette, aber nicht jeden
zentimetergrossen Rock-Chip oder jede kleine Blattfacette im Schattenbild.
Sichtbare Geometrie, Blatttransparenz und Wind werden nicht pauschal entfernt.
Zusatzmeshes und deren RAM-/Ladezeitkosten muessen in der Abnahme mitgemessen
werden; das ist keine kostenlose Optimierung.

Bei einem von Cube und Custom-Modell gemeinsam genutzten Material bleibt dessen
vollstaendige Schattengeometrie erhalten. Sonst koennte das Deaktivieren des
gemeinsamen Farb-Batch-Casters auch den Modellanteil aus dem Schatten entfernen.
Die Materialfreigabe wird auf dem Worker geprueft und ist regressionsgetestet.

## Messbedingungen

Ryzen 9 5900X, Radeon RX 6950 XT, Vulkan/RADV Mesa 26.2.4. Optimiertes Dev-Profil,
VSync aus. Hauptpaar: dieselben Phase-52-Welt-/Detailparameter, 32er-Sichtweite,
Terrain-LOD 192/320 m vor vorhandener 1.5-Skalierung, FOV 65, Seed 42042,
Geologievertrag 0 mit Erzrevision 1, feste Weltzeit 10 Uhr, identische Kamera
`[-622, 53.88851, 680]`, 60 s Messfenster nach Szenenbereitschaft, physisch
3828 x 2043 Pixel. `configs-fixed32` fordert 2552 x 1362 logische Pixel an, damit
das Fenster bei 1.5-facher Desktopskalierung ohne wechselnde Workarea-Begrenzung
in den Monitor passt. Das ist keine exakte 3840-x-2160-Aufnahme.

Der eingefrorene Vorherbuild verwendet seine eigene Assetkopie. Neue Geologie
wird nicht mit Rendergewinn vermischt. Keine gleichzeitig laufenden Cargo-,
CPU-Benchmark- oder weiteren nativen Testprozesse waehrend GPU-Messungen.
RSS und DRM-VRAM/GTT sind Peaks. Einzelne GPU-Passzeiten sind keine vollstaendige
oder summierte GPU-Framezeit.

Die Messpaare vergleichen das gesamte Aenderungspaket, nicht isoliert nur
Schatten oder nur das Auslassen leerer Uploadscans. Speicherunterschiede werden
daher nicht ausschliesslich einer einzelnen Massnahme zugeschrieben.

## Finale Ruhende Messpaare

| 32er-Sichtweite, 3828 x 2043 | Vorher | Nachher |
| --- | ---: | ---: |
| Durchschnittliche FPS | 55.302 | 63.330 |
| p95 Frame, ms | 20.393 | 17.233 |
| p99 Frame, ms | 26.635 | 19.005 |
| Initiale Szenenbereitschaft, s | 43.294 | 44.102 |
| Peak Prozess-RSS, MiB | 2373.64 | 2495.06 |
| Peak DRM-VRAM, GiB | 7.691 | 7.690 |
| Peak DRM-GTT, MiB | 68.14 | 68.14 |
| Terrain-Komposition, aufsummierte Workerzeit, s | 6.337 | 6.672 |
| Early-Prepass GPU-Mittel, ms | 2.226 | 2.269 |
| Main-Opaque GPU-Mittel, ms | 4.404 | 4.515 |
| Geladene / gerenderte Sections | 31 872 / 20 420 | 31 872 / 20 420 |
| Meshassets, einschliesslich Schattenproxies | 48 970 | 53 864 |
| Aktive / repraesentierte Props | 401 244 / 401 244 | 401 244 / 401 244 |
| Prop-Batches / Wassertiles | 4657 / 89 | 4657 / 89 |

In diesem Paar FPS +14.52 Prozent, p99 -28.65 Prozent. Nicht kostenlos: Peak RSS
+121.43 MiB, Bereitschaft +0.808 s und mehr Shadow-Meshassets. VRAM ist in diesem
Paar nahezu gleich; keine allgemeine Speicherparitaet. Die Haupt-Farbpasszeit
steigt leicht, die sichtbare Geometrie wurde nicht pauschal reduziert.
Schatten sind nicht als eigener vollstaendiger
GPU-Pass instrumentiert; keine GPU-interne Subdraw- oder Total-GPU-Behauptung.
[Vorher](before-4k32.log), [Nachher](after-4k32.log).

Zusaetzliches Paar mit denselben Tageszeit-/Welt-/Kamerabedingungen und
`configs-small`: 8er-Sichtweite, normale 96/160-m-LOD-Konfiguration vor bestehender
Skalierung, physisch 1920 x 1200, ebenfalls 60 s:

| 8er-Sichtweite | Vorher | Nachher |
| --- | ---: | ---: |
| Durchschnittliche FPS | 162.852 | 205.848 |
| p95 / p99 Frame, ms | 7.624 / 8.799 | 6.068 / 7.065 |
| Initiale Bereitschaft, s | 1.806 | 1.956 |
| Peak Prozess-RSS, KiB | 730 292 | 738 884 |
| Peak DRM-VRAM, KiB | 2 020 296 | 2 019 284 |
| Geladene / gerenderte Sections | 2041 / 875 | 2041 / 875 |
| Meshassets | 2561 | 3487 |
| Aktive / repraesentierte Props | 25 808 / 25 808 | 25 808 / 25 808 |

Auch hier zusaetzliche CPU-Meshdaten, kein Versprechen von Speicherparitaet.
[Vorher](before-small8.log), [Nachher](after-small8.log).

Reproduktion des grossen Nachherlaufs:

```bash
env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/minerals-performance-2026-10-17/configs-fixed32 \
  TERRA_BENCH_SECONDS=60 TERRA_RENDER_DIAGNOSTICS=1 TERRA_MEASURE_GPU_MEMORY=1 \
  python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-forest
```

Der Vorherlauf verwendet den lokal eingefrorenen Build `target/debug/terra53-before`
und `TERRA_ASSET_DIR` mit absolutem Pfad auf `target/terra53-before-assets`.
Diese grossen Buildartefakte gehoeren nicht zu den versionierten Spielassets.

## Bewegung Und Streaming

Zwei identische 120-s-Out-and-Back-Fluege mit der grossen Konfiguration, ohne
zusaetzliche Screenshots im Messfenster. Die feste Kurve bewegt die Kamera
zweimal hin und zurueck ueber jeweils 224 m und ueber beide LOD-Schwellen.
Der Flug wartet am Ende auf abgeschlossene Meshjobs. Das Messfenster enthaelt
Bewegung und Streaming, nicht diese abschliessende Wartezeit.

| 32er-Streaming | Vorher | Nachher |
| --- | ---: | ---: |
| Durchschnittliche FPS | 33.829 | 36.169 |
| p95 / p99 Frame, ms | 35.015 / 39.534 | 32.492 / 36.037 |
| Initiale Bereitschaft, s | 42.888 | 40.833 |
| Peak Prozess-RSS, KiB | 2 661 136 | 2 803 004 |
| Peak DRM-VRAM, KiB | 8 860 788 | 9 489 976 |
| Terrain-Komposition, aufsummierte Workerzeit, s | 24.238 | 25.597 |
| Abgeschlossene LOD-Wechsel | 29 525 | 31 122 |
| Aktive / repraesentierte Props | 401 244 / 401 244 | 401 244 / 401 244 |

Der Gewinn reicht nicht fuer ein behobenes 32er-Streamingproblem. p95/p99 sind
besser, Speicher und aufsummierte Terrain-Komposition aber hoeher.
Beide Fluege beenden alle Meshjobs; unterschiedliche Frameraten und asynchrone
Abschluesse erzeugen nicht dieselbe Anzahl transienter LOD-Auftraege. Kein
bitidentischer Jobablauf und keine Speicherparitaet behauptet. Die restlichen
Streamingkosten bleiben ausdruecklich in TODO.
[Vorher](before-stream32.log), [Nachher](after-stream32.log).
Reproduktion wie oben mit `TERRA_BENCH_SECONDS=120` und `--smoke-test-lod`.

## CPU-Generierung

Vier Worker, 4375 Sections, Radien 12/3, ein Warmup und fuenf Messungen je Modus.
Kein Renderer, keine GPU-Uploads und keine gleichzeitigen GPU-/Cargo-Prozesse.
Render-Vorbereitung umfasst gepaddete Voxel, Greedy-Meshing, Wasser und Props;
die Differenz zur Headless-Zeit ist keine isolierte Meshingmessung.

| Median, s, Oberflaechenzentrum (0,7,1) | Geologie 0 | Geologie 1 | Geologie 2 |
| --- | ---: | ---: | ---: |
| Headless-Generierung | 0.294842 | 0.264009 | 0.270684 |
| Render-Vorbereitung | 0.511689 | 0.464559 | 0.471241 |
| Greedy-Quads | 512 458 | 471 440 | 471 440 |
| Material-Checksumme | 25 753 415 | 23 756 359 | 23 756 359 |

Diese Oberflaechenprobe erzeugt zwischen 1/2 keine neuen Formationen; sie allein
waere deshalb kein Nachweis ihrer Kosten. Die alte Geologie behaelt denselben
Checksum-/Quad-/Propstand wie Phase 52; die historischen 0.291779/0.512423 s
liegen nahe am neuen Lauf, Headless ist historisch rund ein Prozent langsamer.
Das ist kein frisch gebautes bitidentisches Phase-52-CPU-
Vorherpaar und kein allgemeiner Generation-Speedup.
[Alt](worker-legacy.log), [Vertrag 1](worker-geology1.log), [Vertrag 2](worker-geology2.log).

Zusaetzliches Inhaltspaar im echten Salzgebiet, Sectionzentrum `(1050,-3,-1122)`:

| Median, s | Geologie 1, ohne grosse Formationen | Geologie 2, mit Formationen |
| --- | ---: | ---: |
| Headless-Generierung | 0.473196 | 0.474504 |
| Render-Vorbereitung | 0.828927 | 0.843964 |
| Greedy-Quads | 856 589 | 859 984 |
| Material-Checksumme | 54 704 941 | 54 827 165 |
| Props | 573 | 573 |

Leicht teurer bei absichtlich neuem Inhalt: rund +0.28 Prozent Generierung und
+1.81 Prozent Vorbereitung in dieser Probe. Die Caches enthalten nur begrenzte
immutable Deskriptoren. Das ist keine Messung des spaeteren Renderer-Komponierens
der Erzfacetten. [Ohne](worker-salt1.log), [Mit](worker-salt2.log).

Der abschliessende reine Borrow-Fix besitzt ein separates frisches Vergleichspaar
mit demselben Worker-Eingang und unveraendertem Inhalt:

| Median, s | Vor Borrow-Fix | Nach Borrow-Fix |
| --- | ---: | ---: |
| Geologie 0, Headless | 0.300347 | 0.294842 |
| Geologie 0, Vorbereitung | 0.522477 | 0.511689 |
| Salzgebiet Geologie 2, Headless | 0.475777 | 0.474504 |
| Salzgebiet Geologie 2, Vorbereitung | 0.847020 | 0.843964 |

Kleiner Gewinn in der Oberflaechenprobe, sehr kleine Unterschiede im Salzgebiet,
kein grosser universeller CPU-Speedup. Beide Paare behalten ihre jeweiligen
Material-Checksummen (25 753 415 beziehungsweise 54 827 165), Quads und Props.
[Vorher alt](worker-before-borrow0.log), [Nachher alt](worker-legacy.log),
[Vorher Salz](worker-before-borrow-salt2.log), [Nachher Salz](worker-salt2.log).

## Funktionsabnahme

Alle Befehle wurden mit `--locked` ausgefuehrt, wo Cargo dies unterstuetzt:

- [fmt](fmt.log): `cargo fmt --all --check` bestanden.
- [check](check.log): `cargo check --workspace` bestanden.
- [tests](tests.log): `cargo test --workspace`, 864 bestanden, ein vorhandener
  langer UDP-Soak ignoriert. Profile/UUIDs, Material-Sharing, kanonische Adern/
  Spitzen, Indexgueltigkeit, LOD-Seams, Proxy-LOD-Eviction, Cache-Determinismus,
  echte Boden-/Deckensupports und Erhalt gespeicherter Geologie 1 enthalten.
- [clippy](clippy.log): `cargo clippy --workspace --all-targets -- -D warnings`
  bestanden.
- [build](build.log): `cargo build --workspace` bestanden.
- [Sechs Erze](ores-native.log): echte exponierte Deposits in generiertem Terrain,
  geteilter Pickingpfad, Inspektionslicht und sechs GPU-Aufnahmen. Der Fixture
  sucht Salz in einer tatsaechlichen seltenen Salzkammer, nicht mehr am alten
  salzlosen Heimatpunkt. Keine kuenstlich eingesetzten Erz-Testbloecke.
- [Geologie](geology-native.log): fuenf GPU-Ansichten von Cheese, Dripstone,
  Spaghetti, Salz und Cliffted Desert. Helle mehrblockige Salzkoerper und Spitzen
  sind in [Salz](captures/geology-salt.png) sichtbar; [Tropfstein](captures/geology-dripstone.png)
  nutzt die warme eigene Mineralflaeche. [Gold](captures/ore-raw_gold.png) zeigt
  Stein und feine Bruchadern. Diese wichtigen Nah-/Gesamtansichten wurden
  tatsaechlich gesichtet, nicht nur aus erfolgreichem Shaderbau abgeleitet.
- [Animationen](animations-native.log): nativer GPU-/Pixelfixture fuer erste
  Uploads, FadeSlow, Bounce, Rise und Slide sowie fertig vorbereitetes Terrain
  in acht Richtungen. Keine auf unsichtbare Shadow-Proxies wartenden Farb-Draws.
- [Server](network-server.log), [Beobachter](network-observer.log),
  [zweiter Spieler](network-companion.log): zwei echte Clients mit
  `--profile tester-a` beziehungsweise `--profile tester-b`, verschiedenen
  persistenten UUIDs und autoritativen ACKs. Der
  Beobachter prueft den replizierten Avatar des anderen Spielers; dieser bewegt
  sich rund 6.7 m. [Gesichtetes Remote-Modell](captures/network-observer.png).
  Die Inspektionskamera ist im Fixture erhoeht, damit nahes Gras den Avatar nicht
  verdeckt; normale Gameplay-Kameras werden dadurch nicht veraendert.
- [Headless](headless.log): Server mit entfernter `DISPLAY`-/`WAYLAND_DISPLAY`-
  Umgebung gestartet, 27 Sections und sauberer Exit. Der isolierte normale
  [Server-Dependencytree](server-dependencies.log) enthaelt kein `terra-render`,
  `terra-ui`, `bevy_render` oder `winit`.

## Zwischenversuche

Die Rohlogs der Versuche bleiben erhalten, werden aber nicht als finale
Messung ausgegeben:

- `leaf-only-shadow-experiment.log`: 57.916 FPS, p99 28.847 ms, 2 557 368 KiB
  Peak RSS. Der kleine Gewinn gegenueber 55.882 FPS bei zusaetzlichem RAM war
  Anlass fuer zusammengefasste opake Schalen.
- `alternate-camera-shadow-experiment.log` hat eine abweichende Kamerahoehe
  (57.233925). Seine 63.525 FPS sind kein strenges Vergleichspaar.
- `alternate-workarea-core-borrow.log` wurde vom Desktop auf 3828 x 2112 statt
  3828 x 2043 Pixel begrenzt. Er dient nicht als strenger Vorher-/Nachhervergleich.
  Deshalb wurden beide grossen ruhenden Laeufe mit kleiner angeforderter, stabiler
  Fensterflaeche frisch wiederholt. Fruehere Werte stehen unter `before-body-split/`.
- `unmerged-shadow-experiment.log` vor weiterer Zusammenfassung hatte 64.131 FPS,
  p95 17.085 ms und p99 24.578 ms, aber 57 383 Meshassets statt 48 970. Das ist
  ebenfalls ein Zwischenstand, kein Nachweis des endgueltigen Batching.
- `cpu-stacks*.log`: zwei Debugger-Stichproben zeigen Render-/Queue-Wartepfade,
  kein statistisches CPU-Profil und keine bewiesene Hotspot-Zuordnung. Anhaengen
  an einen fremden Prozess war per ptrace nicht erlaubt. Der als Debuggerkind
  gestartete Lauf enthaelt Pausen und darf nicht als FPS-Benchmark dienen.

## Verbleibende Grenzen

Keine allgemeine 100-FPS-, Speicherparitaets- oder Mehr-GPU-/Langzeitgarantie.
Grosse Sichtweiten und Streaming bleiben ein eigenes Performanceziel.
Schattenproxies sind aktuell Teil der Near-Komposition. Das Abschalten der
Schattenmaps beendet ihre Draws, gibt diese Meshdaten aber noch nicht gezielt
frei; eine qualitaetsabhaengige Proxy-Erzeugung/-Freigabe steht in TODO.
Kaltes Umgebungslicht kann Stein in Schatten weiterhin blau faerben; die neue
Albedo ersetzt keine globale Hoehlenbeleuchtung. Bitmapwiederholung ist visuell
zu beurteilen, nicht mathematisch an identische Randtexel gebunden.
Die grossen Formationen besitzen normale Voxel-Kollision und -Abbau. Es gibt
keine herabfallenden Tropfsteinprojektile, Schaden oder Kettenbruch beim Entfernen
eines Supportblocks. Die nativen Inspektionslichter sind nur Testhilfen, keine
automatisch generierten Hoehlenlampen. Keine 1:1-Referenzqualitaet behauptet.
