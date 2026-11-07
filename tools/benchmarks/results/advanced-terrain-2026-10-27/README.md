# Landschaftsrevision 5: Messstand vom 2026-10-27

Vertrag und Grenzen: [TERRAIN.md](../../../../TERRAIN.md).
Die folgenden Messungen stammen aus optimierten Dev-Builds mit Debuginfo,
nicht aus Release-Builds. Linux x86_64, Ryzen 9 5900X; native Grafiklaeufe auf
Radeon RX 6950 XT / RADV / Vulkan / Mesa 26.2.5-arch3.1 bei echten 1280x800 Pixeln.
Alle Zeiten sind Wall-Clock-Werte einschliesslich Schleifen, Timer und Scheduling.

Eager-Tile-Baseline und finale Lazy-Cache-Messung stehen getrennt nebeneinander.
Zwischen den Laeufen wurde auch erheblicher Speicher-/Tempdateidruck beseitigt;
der Vergleich isoliert deshalb nicht ausschliesslich die Codeaenderung.
Ein schneller warmer Lookup belegt weder schnellere kalte Weltgenerierung
noch bessere Gameplay-FPS.

## CPU: Warme Und Kalte Hoehenabfragen

`terrain_profiles --benchmark`: Renderer/GPU aus; Wasser, Fluesse, Hoehlen,
Baeume und Strukturen aus. Pro Revision 4096 ungemessene Warmup-Abfragen,
danach 7 Runden mit je 16 x 4096 Abfragen im selben lokalen 128-m-Tile.
Kalt: frischer Generator, einmal 128 unterschiedliche, 128 m entfernte Tiles.
Revisionen 4/5 nutzen gleiche Koordinaten und Einstellungen, aber andere Hoehen.

Eager-Baseline:

| Seed | Warm v4, 458752 Abfragen | Warm v5, 458752 Abfragen | Kalt v4, 128 Abfragen | Kalt v5, 128 Abfragen |
| --- | ---: | ---: | ---: | ---: |
| 42042 | 307.647 ms | 47.540 ms | 0.116 ms | 77.320 ms |
| 7 | 280.730 ms | 45.655 ms | 0.112 ms | 76.520 ms |
| 99173 | 286.975 ms | 46.083 ms | 0.116 ms | 79.841 ms |

Die Eager-Baseline verbessert hier warme Abfragen, erkauft sie aber durch viel
teurere erste Tile-Zugriffe. Dies ist kein Cold-Streaming-Leistungsnachweis.
Rohdaten: `/tmp/teramic141-final-benchmark/seed-*-benchmark.txt`.

Finaler Lazy-Cache, identisches Abfrageverfahren:

| Seed | Warm v4, 458752 Abfragen | Warm v5, 458752 Abfragen | Kalt v4, 128 Abfragen | Kalt v5, 128 Abfragen |
| --- | ---: | ---: | ---: | ---: |
| 42042 | 308.331 ms | 52.790 ms | 0.118 ms | 11.712 ms |
| 7 | 284.361 ms | 52.703 ms | 0.116 ms | 11.419 ms |
| 99173 | 286.014 ms | 52.112 ms | 0.113 ms | 11.867 ms |

Die warmen v5-Lookups sind gegenueber Eager etwas langsamer, kalte v5-Zugriffe
in diesem Lauf deutlich schneller; gegenueber v4 bleiben kalte Abfragen teurer.
Alle warmen und kalten Pruefsummen stimmen pro Seed/Revision mit Eager ueberein.
Rohdaten: `/tmp/teramic141-lazy-benchmark/seed-*-benchmark.txt`.

## CPU: Section-Population

`terrain_sections`: 36 Sections pro Pass, drei X/Z-Anker `[256,256]`,
`[-2048,3072]`, `[5376,-4864]`, jeweils 2x2 Spalten und drei vertikale Sections
um die lokale Oberflaeche. Wasser/Fluesse/Hoehlen/Baeume an, Strukturen aus.
`with_mesh=false` misst Voxel und Props, nicht Meshing, Rendering oder FPS;
Hoehenabfragen sind enthalten. Warm verwendet denselben Generator erneut.

Eager-Baseline vor der Cache-Umstellung:

| Seed | Kalt v4 | Kalt v5 | Warm v4 | Warm v5 | Gefuellte Voxel v4/v5 | Props v4/v5 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 42042 | 99.290 ms | 418.437 ms | 66.121 ms | 92.500 ms | 85255 / 67283 | 536 / 351 |
| 7 | 91.995 ms | 403.747 ms | 72.119 ms | 90.570 ms | 73259 / 61069 | 795 / 110 |
| 99173 | 100.328 ms | 474.323 ms | 76.846 ms | 97.259 ms | 76418 / 67429 | 169 / 625 |

Die kalte Eager-Generierung ist in diesem Versuch deutlich langsamer; auch die
warmen Section-Passes sind langsamer. Identische X/Z sind keine identische
Geometrie: Hoehe, Voxelmenge und Bewuchs unterscheiden sich absichtlich.
Rohdaten: `/tmp/teramic141-sections.log`.

Finaler Lazy-Cache:

| Seed | Kalt v4 | Kalt v5 | Warm v4 | Warm v5 |
| --- | ---: | ---: | ---: | ---: |
| 42042 | 94.453 ms | 227.567 ms | 67.684 ms | 77.170 ms |
| 7 | 84.558 ms | 216.017 ms | 63.711 ms | 79.119 ms |
| 99173 | 90.093 ms | 257.164 ms | 73.084 ms | 84.091 ms |

Lazy misst hier guenstigere v5-Section-Zeiten als Eager. Trotzdem bleibt v5
in allen drei Seeds kalt und warm teurer als v4. Gefuellte Voxel und Props
entsprechen in jedem Seed/Revision/Pass exakt den Zaehlern der Eager-Tabelle.
Rohdaten: `/tmp/teramic141-sections-lazy.log`.

## Einfluss Der Speicherbereinigung

Zwischen den Messstaenden wurden drei eigene ELF-Probes mit 2,651,536,328 Bytes
und sieben alte Benchmark-Client-ELFs mit 11,857,149,008 Bytes entfernt,
zusammen 14,508,685,336 Bytes (13.512 GiB). Quellen, Rohlogs und Bilder blieben
erhalten. Das 16-GiB-`/tmp`-tmpfs war zuvor zu 97% belegt, danach rund 1.7 GiB
beziehungsweise 11%; verfuegbarer RAM stieg von rund 5.7 auf 16 GB.
Die historischen Before-/Baseline-ELFs sind damit nicht mehr vorhanden.

Cache-Aenderung und veraenderte Speicher-/I/O-Bedingungen sind in diesem
Vorher-/Nachher-Vergleich nicht sauber getrennt. Weder der gesamte Zeitgewinn
noch der fruehere Cherry-Ausreisser kann dadurch allein dem Code, der
Bereinigung oder einer konkreten GPU-/Scheduling-Ursache zugeschrieben werden.

## Profil-Survey

Je Seed 32x32 Basishoehen-Samples, Radius 8192 m und Abstand 512 m; keine
Fluesse, Seen, Hoehlendichte oder finale erodierte Oberflaeche abgefragt.
Alle zehn Profile waren pro Seed mindestens einmal dominant:

| Dominantes Profil | 42042 | 7 | 99173 |
| --- | ---: | ---: | ---: |
| Plains | 60 | 49 | 59 |
| Rolling Plains | 40 | 44 | 40 |
| Forest | 91 | 102 | 91 |
| Mountain Forest | 15 | 25 | 21 |
| Fantasy Mountains | 100 | 100 | 101 |
| Snowy Highlands | 61 | 61 | 74 |
| Cherry Grove | 17 | 11 | 12 |
| Spirit Forest | 20 | 13 | 14 |
| Desert | 57 | 49 | 48 |
| Ocean | 563 | 570 | 564 |

Gemessene Basishoehenbereiche: Seed 42042 -95.915..324.348 m,
Seed 7 -95.708..320.695 m, Seed 99173 -97.960..315.624 m.
Dominant bedeutet groesstes Mischgewicht, nicht ein reines Profil oder die
Einhaltung seines einzelnen Y-Bands durch die gesamte Mischung.
Rohdaten mit Ankern: `/tmp/teramic141-final-survey/seed-*-profiles.txt`.

## Native Kurzfenster

Fuenf abgeschlossene Offline-Laeufe ueber drei Seeds, je 6 Sekunden gemessen;
statische Kamera, UI/Hotbar/Minimap/Survival-HUD aus, VSync/FPS-Limit aus,
FOV 45 Grad, vertikale Sichtweite 4, High-Schatten/-Wasser/-Wetter,
Vegetationsdichte 1.0 und High-Blattgeometrie.
Screenshot-nahe Frames sind ausgeschlossen. Es wurden CPU-Wall-Frames erfasst,
keine gesamte GPU-Framezeit; `controller_calls=0`, kein Autoritaets-/Flugtest.

| Szene / Seed | Sichtweite | Frames | AVG FPS | Min FPS | P95 / P99 ms | Schlechtester Frame | Frames <32 FPS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Peak / 42042 | 6 | 1232 | 205.332 | 110.220 | 5.946 / 6.820 | 9.073 ms | 0 |
| Wasser, Regen / 42042 | 6 | 1079 | 179.755 | 109.508 | 6.769 / 7.550 | 9.132 ms | 0 |
| Plains / 7 | 12 | 860 | 143.270 | 71.855 | 9.417 / 10.967 | 13.917 ms | 0 |
| Cherry Overview / 99173 | 12 | 548 | 91.248 | **4.541** | 17.462 / 28.999 | **220.212 ms** | **5** |
| Cherry Wiederholung / 99173 | 12 | 965 | 160.772 | 82.972 | 7.963 / 9.235 | 12.052 ms | 0 |

Der erste Cherry-Lauf bleibt ein echter Ausreisser. Die bessere Wiederholung
ersetzt ihn nicht; aus diesen Logs wird keine Ursache oder GPU-Stall-Zuordnung
abgeleitet. Initiale Ladezeiten: 3.423 / 4.661 / 7.189 / 9.429 / 7.390 Sekunden
in Tabellenreihenfolge. Keine FPS-Garantie, kein 30-Minuten-Test, kein
Langzeit-Flythrough und keine gleichwertige Vergleichsszene ueber alle Seeds.
Die spaetere Heatmap-Pruefung ist unten separat dokumentiert und ersetzt diese
Offline-Messwerte nicht.

Rohlogs: `/tmp/teramic141-peak-42042.log`, `/tmp/teramic141-water-42042.log`,
`/tmp/teramic141-plains-7.log`, `/tmp/teramic141-cherry-99173-overview.log`,
`/tmp/teramic141-cherry-99173-repeat.log`. Lokale `/tmp`-Artefakte sind nicht versioniert.

## Native Heatmap Mit Integrated Server

Separater finaler 20-s-Lauf: echter Integrated Server mit UI, Seed 42042,
Sichtweite 6, High-Einstellungen, 1280x800, klares Wetter und Flusszusammenfluss.
Die angeforderte Kamera war `[-1166.5,62,854.5]`, Blickziel `[-1158,46,864]`;
die physisch simulierte Kamera lag bei der Zusammenfassung auf Y61.717.
Hotbar und Minimap waren sichtbar. Das ist nicht derselbe Modus wie die
6-s-Offline-Tabelle und deshalb kein direkter FPS-Vergleich.

| Messfenster | Frames | AVG FPS | Min FPS | P95 / P99 ms | Schlechtester Frame | Frames <32 FPS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 20 s | 2950 | 147.494 | 70.502 | 8.005 / 9.372 | 14.184 ms | 0 |

Gemeldet: 2608 Controller-Aufrufe, 1,472,384 KiB residenter Speicher und
12.278 s initiale Ladezeit. Der Lauf wurde abgeschlossen; das Log enthaelt
keine WARN-/ERROR-Meldungen. Der aufgenommene Screenshot wurde kontrolliert:
farbige Height-Heatmap und alle zehn Messwerte sichtbar, keine weisse Flaeche.
Auch dieser kurze Lauf ist weder ein Soak-Test noch eine allgemeine FPS-Garantie.
[Log](/tmp/teramic141-terrain-height-fixed.log),
[Screenshot](/tmp/teramic141-terrain-height-fixed.png).

## Reproduktion

Vom Repository-Root mit zuvor gebauten Dev-Binaries, ohne parallelen Build/GPU-Lauf:

```bash
target/debug/examples/terrain_profiles --seed 42042,7,99173 --benchmark --output /tmp/terrain-benchmark
target/debug/examples/terrain_profiles --seed 42042,7,99173 --radius 8192 --step 512 --output /tmp/terrain-survey
target/debug/examples/terrain_sections
env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET WINIT_X11_SCALE_FACTOR=1 \
TERRA_CONFIG_DIR=/tmp/teramic141-config TERRA_WORLD_DIR=/tmp/teramic141-native-worlds \
TERRA_BENCH_MODE=offline \
TERRA_BENCH_GRAPHICS=1 TERRA_BENCH_SECONDS=6 \
TERRA_BENCH_CAMERA='3812,285,-5338' TERRA_BENCH_LOOK_AT='3840,251,-5375' \
TERRA_BENCH_WEATHER=clear target/debug/terra-client --smoke-test-benchmark

env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET WINIT_X11_SCALE_FACTOR=1 \
TERRA_CONFIG_DIR=/tmp/teramic141-config TERRA_WORLD_DIR=/tmp/teramic141-native-worlds \
TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 \
TERRA_BENCH_GRAPHICS=1 TERRA_BENCH_SECONDS=20 TERRA_TERRAIN_DEBUG=height \
TERRA_BENCH_CAMERA='-1166.5,62,854.5' TERRA_BENCH_LOOK_AT='-1158,46,864' \
TERRA_BENCH_WEATHER=clear target/debug/terra-client --smoke-test-benchmark
```

Die native Config muss Seed, Sichtweite und Wetter der jeweiligen Zeile verwenden:
1280x800, Revision 5, Meeresspiegel 22, Strukturen aus, Uhrzeit 12.
Die temporaere Config wurde zwischen Laeufen geaendert, ist also kein unveraenderliches Archiv.

| Szene | Kamera X,Y,Z | Blickziel X,Y,Z | Wetter |
| --- | --- | --- | --- |
| Peak / 42042 | 3812,285,-5338 | 3840,251,-5375 | clear |
| Wasser / 42042 | 576.5,73,-2847.5 | 581,55,-2832 | rain |
| Plains / 7 | 3816,73,2841 | 3840,48,2816 | clear |
| Cherry / 99173, beide Laeufe | 5377,123,-4864 | 5406,83,-4892 | clear |

## Validierungsstand

`/tmp/teramic141-workspace-final.log`: finaler Workspace-Lauf nach Lazy-Cache,
2703 Tests bestanden, 7 ignoriert, 0 Fehler. Darin enthalten: 759 World-Tests
bestanden, 4 ignoriert, 0 Fehler. Der fruehere World-Zwischenstand 755+4 wird
nicht zusaetzlich gezaehlt.

- Check erfolgreich: `/tmp/teramic141-check-lazy.log`.
- Clippy erfolgreich: `/tmp/teramic141-clippy-lazy.log`.
- Doc-Test-Lauf erfolgreich, derzeit 0 ausfuehrbare Doc-Tests:
  `/tmp/teramic141-doctests-final.log`.
- Finaler Dev-Build fuer Client, Server und World erfolgreich:
  `/tmp/teramic141-build-lazy-final.log`.

Diese Ergebnisse bestaetigen die dokumentierten Prueflaeufe, nicht eine
universelle Laufzeit-/FPS-Grenze oder einen 30-minuetigen Stabilitaetstest.
