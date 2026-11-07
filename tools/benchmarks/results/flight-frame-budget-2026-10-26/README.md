# Flug-Framebudget (Phase 138)

## Ziel Und Stand

32 FPS entsprechen maximal 31,25 ms pro Frame. Der erste Ausgangslauf reproduziert
einen 94,33-ms-Frame (10,60 FPS), seine Wiederholung ohne Aenderungen nicht.
Der abschliessende Stand verbessert den Durchschnitt im integrierten Flug um
4,8 % und im Wassertest um 6,6 %, aber **nicht die schlechtesten Frames**.
Die geforderte 32-FPS-Untergrenze ist nicht erreicht. Kein Abschluss von A05,
keine zugesicherte Mindestframerate und keine neuen Langzeittests.

## Tatsaechliche Aenderungen

- `meshing/upload.rs`: unveraenderliche Section-Meshes erhalten ihre berechneten
  AABBs beim Upload und `NoAutoAabb`. Wind-Padding bleibt erhalten. Die Engine
  muss diese Entities nicht bei jeder Mesh-Asset-Aenderung erneut pruefen.
- `props/jobs.rs`, `water/upload.rs`: vorhandene gepaddete Bounds werden ebenfalls
  als explizit verwaltet markiert. Kein `NoFrustumCulling`: Sichtbarkeitspruefungen
  und Schatten bleiben aktiv; bewegte/dynamische Meshes behalten automatische Bounds.
- `streaming/resources.rs`: bestehende Section-/Column-Tabellen und Mengen nutzen
  Bevys vorhandene Hash-Collections statt des Standard-Hashers. Keine neue Registry,
  kein zweiter Cache, keine Aenderung von Seeds, Generation oder Speicherformaten.
- `streaming/interest.rs`: benoetigte Sections werden direkt in die gemeinsame
  Interest-Menge eingefuegt, statt fuer jeden Spieler zuerst eine temporaere Menge
  aufzubauen und deren Eintraege anschliessend noch einmal zu hashen.
- `Cargo.toml`: die CPU-intensiven Workspace-Pakete `terra-render` und `terra-voxel`
  verwenden im normalen Dev-Build Optimierungsstufe 3. Die vorhandene Wildcard
  galt nur fuer externe Abhaengigkeiten. Debug-Informationen und Checks bleiben
  erhalten. Nachteil: laengere Kompilierung dieser beiden Pakete.
- `smoke_test.rs`: zusaetzlicher Zaehler `frames_below_32_fps`; kein Glaetten,
  Verwerfen langsamer Frames oder kuenstliches FPS-Limit.

Grafikoptionen, Vegetationsdichte, Sichtweite, Wasser, Schatten und Eintritts-
Animationen wurden nicht reduziert. Kein Vendor-/Registry-Patch, keine
Dependency-Upgrades und keine TiltUI-Aenderung in dieser Phase.

## Profiling Und Verworfene Versuche

Ein separater Linux-perf-Lauf mit User-Space-Callchains identifizierte unter
anderem `AssetChangeCheck<Mesh3d>::has_changed` (zusammen ca. 4,06 % Self-CPU in
zwei Instanziierungen), Section-Hashing, Frustum-Tests und Buffer-/Binding-
Verwaltung. Der Profilerlauf verlor 11,92 % seiner Samples und ist keine
FPS-Vergleichsmessung. Rohprofil liegt lokal unter `/tmp/teramic138-cpu.data`.
Nicht der gesamte AssetChangeCheck-Aufwand ist die Bounds-Berechnung: Material-
Spezialisierung braucht weiterhin Asset-Aenderungserkennung.

Der vorherige instrumentierte Lauf aus Phase 137 erfasste `streaming.interest`
mit maximal 8,41 ms inklusive Startphase. Das ist kein Beweis, dass gerade dieser
Pfad den schlechtesten Frame verursacht hat. Die doppelte Mengenbildung ist
direkt im Code nachvollziehbar und wird gezielt entfernt.

CPU-Affinitaet auf sechs Kerne ergab 66,91 FPS. Ein Experiment mit vier Compute-
Workern und seriellen Main-/Render-Schedules ergab 76,82 FPS. Beide wurden
verworfen; die regulaere Thread-/Schedule-Konfiguration bleibt unveraendert.
Der vorzeitig waehrend einer Kompilierung gestartete Lauf `teramic138-bounds.log`
wurde abgebrochen und wird nicht als Messwert verwendet.

## Native Vergleichsmessungen

Ryzen 9 5900X, RX 6950 XT, Vulkan/RADV, VSync aus. Keine parallelen Compiler,
Tests oder CPU-Benchmarks waehrend der Messlaeufe. Die normale Desktop-Umgebung
bleibt aktiv; einzelne Minima sind deshalb keine harte Echtzeitgarantie.
Screenshots werden vor dem Messfenster aufgenommen bzw. angrenzende Frames
wie schon in der unveraenderten Ausgangsfixture ausgeschlossen.

Hauptlauf: Seed 5866727212944175842, unveraenderte Landschaftsrevision 0,
1920x1080, Sichtweite 20, hohe Schatten/Wasser/Wetterqualitaet, Regen,
integrierter Server, echtes HUD/Minimap und 60 Sekunden reale Flug-Eingaben.
Diese Einstellungen gelten fuer alle Zeilen der folgenden Tabelle.

| Stand | AVG FPS | Min FPS | p95 ms | p99 ms | Max Frame ms | Laden s |
|---|---:|---:|---:|---:|---:|---:|
| Ausgang | 78,26 | 10,60 | 22,84 | 28,05 | 94,33 | 37,10 |
| Explizite Bounds | 79,54 | 27,07 | 20,74 | 27,03 | 36,95 | 36,39 |
| Zusaetzlich schnelle Maps | 82,17 | 26,65 | 19,91 | 25,25 | 37,53 | 36,40 |
| Zusaetzlich optimierte Pakete | 86,09 | 28,81 | 18,65 | 23,68 | 34,71 | 36,25 |

Die Bounds-Zwischenmessung hatte zusaetzlich sparsames `TERRA_BINDING_DIFF=1`-
Logging. Die uebrigen Vergleichsmessungen verwenden keine CPU-/GPU-Probes.
Im optimierten Paketlauf liegen 2 von 5165 Frames unter 32 FPS. Der groesste
einzelne Ausreisser im Ausgangslauf darf nicht als typischer Frame oder als
belastbarer Faktor fuer eine generelle Minimum-Verbesserung interpretiert werden.

Zweiter Seed 7: separate Offline-Renderer-Hin-/Rueckflugfixture, 1280x800,
Sichtweite 20, hohe Qualitaet, Regen, 24 Sekunden. Nicht mit den absoluten
Werten des integrierten Szenarios vergleichen.

| Stand | AVG FPS | Min FPS | p95 ms | p99 ms | Max Frame ms |
|---|---:|---:|---:|---:|---:|
| Ausgang | 76,84 | 25,77 | 22,34 | 24,83 | 38,81 |
| Bounds/Maps/optimierte Pakete | 75,40 | 25,45 | 21,74 | 24,22 | 39,30 |

Diese Wasserwerte belegen keinen AVG-/Min-FPS-Gewinn. Der Nachher-Lauf hat
4 Frames unter 32 FPS. Die letzten Interest-Aenderungen sind in diesen
Zwischenmessungen noch nicht enthalten.

## Abschliessender Vergleich

Nach dem endgueltigen Build wurde der integrierte Test erneut ausgefuehrt und
danach auch das unveraenderte Ausgangsbinary wiederholt. Der abschliessende
Wassertest verwendet ebenfalls den endgueltigen Build. Keine parallele
Compiler-/Testlast oder Profiler in diesen Laeufen.

| Szene / Stand | AVG FPS | Min FPS | p95 ms | p99 ms | Max Frame ms | Frames unter 32 FPS |
|---|---:|---:|---:|---:|---:|---:|
| Integrated Ausgang, Wiederholung | 79,63 | 24,72 | 20,53 | 26,96 | 40,45 | nicht erfasst |
| Integrated endgueltig | 83,44 | 24,34 | 19,68 | 24,89 | 41,09 | 6 / 5006 |
| Wasser Ausgang | 76,84 | 25,77 | 22,34 | 24,83 | 38,81 | nicht erfasst |
| Wasser endgueltig | 81,94 | 25,19 | 20,12 | 22,43 | 39,70 | 1 / 1957 |

Der einmalige 94-ms-Ausreisser ist damit ausdruecklich **kein** belastbarer
Nachweis einer durch den Fix verbesserten Mindest-FPS. Die wiederholte Basis
und der Endstand haben aehnliche schlechteste Frames. Die normalen/laengeren
Frames (p95/p99) und der Durchschnitt verbessern sich, das wichtigste
Minimum-Ziel bleibt offen. Auch die Schwankung zwischen Zwischenmessungen
zeigt, warum ein einzelner kurzer Lauf keine feste Framerate garantiert.

Der integrierte Endstand laedt initial in 36,20 s (wiederholte Basis 36,31 s).
Der Wasser-Endstand laedt in 23,26 s (Basis 23,55 s). Keine verzoegerte oder
deaktivierte Welt als FPS-Gewinn. Es fehlen insbesondere eine eindeutige
CPU-/GPU-Warteursache pro schlechtestem Frame und ein ueber weitere Szenen
stabiler Nachweis der 31,25-ms-Grenze. Das Sampling-Profil allein liefert
diese Kausalitaet nicht; weitere spekulative Renderer-Aenderungen wurden
nicht als Fix ausgegeben.

## Reproduktion

Die Konfigurationen liegen hier unter `integrated-config/` bzw. `water-config/`;
die Laeufe verwendeten deren identische Kopien `/tmp/teramic137-perf-config`
und `/tmp/teramic137-water-config`. Ausgangsbinary: `/tmp/teramic138-before-client`.
Fuer jeden integrierten Vergleich neue separate Menu-/World-Verzeichnisse
verwenden; keine Benutzerwelt veraendern.

```bash
env TERRA_CONFIG_DIR=tools/benchmarks/results/flight-frame-budget-2026-10-26/integrated-config \
 TERRA_MENU_CONFIG_DIR=/tmp/teramic138-repeat-menu \
 TERRA_WORLD_DIR=/tmp/teramic138-repeat-world \
 TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 TERRA_BENCH_GRAPHICS=1 \
 TERRA_BENCH_SECONDS=60 TERRA_BENCH_WEATHER=rain TERRA_BENCH_LOAD_SECONDS=180 \
 TERRA_SOAK_ROUTE=explore WGPU_BACKEND=vulkan \
 python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-benchmark
```

## Validierung

Neue Regressionen pruefen statische/Wind-Bounds, Erhalt des Displacement-Paddings
bei Asset-Aenderungen, weiterhin aktualisierte Bounds dynamischer Meshes sowie
die deduplizierte Interest-Union mehrerer Spieler bei negativen Koordinaten.
Die bestehenden Upload-Tests pruefen zusaetzlich explizite Bounds an tatsaechlich
publizierten Section-Entities ohne Abschalten des Frustum-Cullings.

Bestanden: `cargo fmt --all --check`, `cargo check --locked --workspace --all-targets -j2`,
`cargo clippy --locked --workspace --all-targets -j2 -- -D warnings` und
`cargo test --locked --workspace -j2 -- --test-threads=2` (2584 bestanden,
7 bestehende Spezialtests ignoriert). Der zuletzt hinzugefuegte Interest-Test
wurde danach separat mit `cargo test --locked --workspace -j2 direct_interest_union
-- --test-threads=2` erfolgreich ausgefuehrt. Insgesamt 2585 bestandene Tests.

`cargo build --locked --workspace -j2` ist ebenfalls bestanden (Client und
Dedicated Server). Native Abschlusslaeufe und die oben aufgefuehrten Zwischen-
laeufe endeten erfolgreich, ohne ERROR/WARN/Panic in den Abschlusslogs.
Das bedeutet Laufzeit-/Rendering-Erfolg, nicht Bestehen des FPS-Minimum-Ziels.
Initiale Vorher-/Nachher-Aufnahmen wurden visuell auf fehlende Geometrie
geprueft; sie ersetzen keine umfassende Art- oder Langzeitabnahme.
