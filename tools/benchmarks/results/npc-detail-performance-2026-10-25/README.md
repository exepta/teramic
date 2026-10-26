# Detaillierte Bewohner Und Schatten Performance

## Umsetzung

Die zehn bestehenden Villager-Rollen besitzen jetzt deutlich kleinteiligere,
prozedural modellierte Kleidung, Gesichter, Haare und Berufsausruestung nach
der gelieferten Referenz. Rollen-IDs, Verhalten, Netzwerkdaten, Inventare und
gespeicherte Bewohner bleiben unveraendert.

- Arbeitsloser: offene Weste, Hemdfalten, Knopfreihe, Taschen und Guertel.
- Farmer: gestufter Strohhut, Schuerze, geflochtener Korb, Weizen und Mistgabel.
- Soldier: segmentierter Helm, roter Kamm/Schal, Schulter- und Hueftplatten,
  Handschuhe und detailliertes Schwert.
- Archer: orange Haarstraehnen, Dutt, gruener Schal, Bogen, Sehne und Koecher.
- Guard: blauer Helmkamm/Umhang, Bart, verzierter Schild und Speer.
- Cleric: geschichtete Kapuze, blau-cremefarbene Robe, Goldsaeume und Kristallstab.
- Fletcher: Lederschuerze, Pfeile, Koecher, Werkzeuge und Taschen.
- Medic: Blumenkopfband, Zopf, Mantel, Faltenrock, Blumen und Medizintasche.
- Worker: Haarbueschel, Bart, Hosentraeger, Arbeitstaschen und Spitzhacke.
- Lumberjack: rot-schwarzes Karomuster, Bart, Kragen und Axt.

Alle Varianten besitzen ausserdem abgestufte Aermel, Haende/Finger,
Stiefelsohlen, Manschetten, Schnallen und Naehte. Die Proportionen haben einen
groesseren Kopf und kuerzere, kraeftigere Gliedmassen. Die Fusskorrektur der
Gehanimation verwendet die neuen Hueft-/Sohlenmasse.

`creatures/model/villager/sculpt.rs` fuehrt ueberlappende Detailkoerper in einem
lokalen 2,5-cm-Raster zusammen. Nur aussenliegende Flaechen werden ausgegeben;
gleichfarbige Nachbarflaechen werden greedy verbunden. Das passiert einmal
beim Aufbau der gemeinsamen Modellbibliothek, nicht je Bewohner oder Frame.
Es bleiben sechs geteilte, starre Mesh-Teile je Rolle, ein gemeinsames
Material und weniger als 16.000 Vertices je kompletter Variante. Keine
Entity, Materialinstanz oder Draw Call je Knopf, Haarbueschel oder Korbstrebe.

Native Aufnahmen aller zehn Rollen, Nahansicht, Geh- und Bettpose sind in
`villagers/` gespeichert und wurden betrachtet. `villager-models.log`
bestaetigt den erfolgreichen Lauf. Das ist eine Rendering-/Animationsprobe,
kein neuer KI- oder Mehrspieler-Langzeittest. Die Modelle sind eine
codebasierte Umsetzung der Referenzrichtung, keine identische Texturkopie
oder Behauptung identischer Beleuchtung.

## Nachgewiesener FPS Engpass Und Fix

Die seit Phase 125 gruppierten Punktlichtschatten berechneten die globalen
Indirect-Daten bereits nur einmal. Die anschliessenden gecachten Systeme
flushten aber weiterhin ihren `RenderContext` nach jeder einzelnen
Cubemap-Seite. Bei acht Lichtern entstanden fuer Unpacking, Preprocessing und
Tiefenzeichnung bis zu 144 getrennte per-face Encoder-Abschluesse pro Frame.

`grouped_point_shadows.rs` buendelt jetzt auch diese Befehlsaufzeichnung:

1. Globale Indirect-Metadaten einmal zuruecksetzen.
2. Alle Seiten entpacken, erst danach den gemeinsamen Encoder abschliessen.
3. Alle Seiten vorverarbeiten, erst danach den gemeinsamen Encoder abschliessen.
4. Globale Indirect-Draw-Parameter einmal erstellen.
5. Alle Tiefenansichten im bestehenden Bevy-`per_view_shadow_pass` zeichnen.

Die originalen Bevy-Systeme und Shader bleiben erhalten. Ein einzelner
Render-World-Helfer traegt die Liste der Schattenviews; er ist keine Kamera
und erzeugt keine weitere Szene. Die Reihenfolge der GPU-Arbeit bleibt
erhalten, jede der sechs Seiten wird jeden Frame behandelt. Unvollstaendige
Gruppen, Direct Drawing, Occlusion Culling und nicht unterstuetzte GPUs
behalten den bestehenden Core3d-Pfad. Die vorhandene Fehler-Rueckfallbehandlung
bleibt bestehen.

Es wurden weder Schatten abgeschaltet noch Sichtweite, Geometrie,
Vegetationsdichte, Wasser- oder Wetterqualitaet reduziert. Keine neue
Dependency, kein neuer Renderer und kein Patch an Cargo-Registry-Dateien.

## Native Kurzvergleiche

Ryzen 9 5900X, RX 6950 XT / Vulkan, Linux. Derselbe optimierte Dev-Client
wie beim regulaeren `cargo run`, ohne gleichzeitig laufenden Cargo-Build.
Beide Seiten der Hauptvergleiche verwenden die **neuen NPC-Modelle**.
`TERRA_SHADOW_ENCODERS=per_face` stellt nur im Smoke-Test den bisherigen
Encoderpfad wieder her; normale Spielstarts verwenden automatisch den Fix.

| Szene | FPS vorher | FPS nachher | p95 vorher/nachher | p99 vorher/nachher |
| --- | ---: | ---: | --- | --- |
| Dorf nachts, 1280x800, Sichtweite 12, High, integriert mit HUD, 8 s | 50,12 | 60,86 | 23,504 / 19,847 ms | 24,674 / 21,200 ms |
| Dorf bei Regen, 3840x2160, Sichtweite 20, Ultra-Wetter, integriert mit HUD, 8 s | 46,22 | 52,47 | 33,163 / 28,519 ms | 43,210 / 41,430 ms |

Rohdaten: `reference-night.log`, `final-night.log`,
`reference-4k-rain.log`, `final-4k-rain.log`. Die zugehoerigen Screenshots
liegen daneben. Gewinn in diesen Szenen: etwa 21,4 bzw. 13,5 Prozent.

| CPU-Wallintervall | Nacht vorher/nachher | 4K-Regen vorher/nachher |
| --- | --- | --- |
| Gruppierte Punktlichtschatten | 4,643 / 3,276 ms | 3,427 / 2,497 ms |
| Gesamte Render-Ausfuehrungsphase | 12,870 / 9,345 ms | 11,618 / 9,083 ms |

Diese Intervalle sind ineinander enthalten und duerfen nicht addiert werden.
Der Fix spart auch nachgelagerte Encoder-/Submit-Arbeit, nicht nur die direkt
gemessene Gruppensystemzeit.

| Szenenlast | Nacht vorher/nachher | 4K-Regen vorher/nachher |
| --- | --- | --- |
| Geladene Sections | 5625 / 5625 | 16137 / 16137 |
| Gerenderte Sections | 2877 / 2877 | 7486 / 7486 |
| Mesh-Assets | 11332 / 11327 | 30243 / 30243 |
| Schattenwerfende Punktlichter | 8 / 8 | 5 / 5 |
| Repraesentierte Props | 80732 / 80732 | 128345 / 128346 |
| Prozess-RSS | 1799 / 1944 MiB | 2444 / 2484 MiB |

Die lebende Szene ist kein bitidentischer Frame-Replay: Tiere und Wetter
laufen, asynchrone Jobs enden in anderer Reihenfolge. Im Nachtvergleich weicht
die physikalisch erreichte Kamerahoehe um sechs Zentimeter ab; im 4K-Vergleich
sind Position und Blickrotation identisch. Die RSS-Einzelwerte belegen keine
Speicherreduktion und keinen Langzeitverlauf.

Zusaetzlich: Seed 7, Wasserregion/Regen, 1280x800/Sichtweite 12,
24-s-Hin-/Rueckflug: **101,39 FPS**, p95 **14,697 ms**, p99 **17,238 ms**,
625 Wassertiles. `flight-seed7.log` verwendet keine Punktlichtschatten;
entsprechend kein behaupteter Gewinn durch den Schattenfix. Der historische
Phase-125-Lauf derselben Route lag bei 101,71 FPS.

Die GPU-Readback-Kontrolle verwendet separate Laeufe, nicht die FPS-Tabelle:

| Schattenkontrolle | Bisheriger Encoderpfad | Gebuendelter Encoderpfad |
| --- | ---: | ---: |
| Schatten-Draw-API-Aufrufe | 452 | 452 |
| Tatsaechlich nichtleere GPU-Schatten-Draws | 10804 | 10814 |
| Nicht gelesene Submissions | 0 | 0 |
| Schattenwerfende Punktlichter | 8 | 8 |

`shadows-reference-draws.log` und `shadows-final-draws.log` bestaetigen,
dass der Gewinn nicht aus ausgelassenen Schatten stammt. Die kleine
Differenz liegt in der laufenden Szene bei identischen Section-/Meshzahlen.

## Reproduktion

Aus dem Repository-Verzeichnis, nach `cargo build --locked -p terra-client`:

```bash
env TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/villagers-pipes-physics-2026-10-25/configs-night" \
  TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 \
  TERRA_BENCH_CAMERA=-574,39,962 TERRA_BENCH_LOOK_AT=-574,33,970 \
  TERRA_BENCH_WEATHER=clear TERRA_BENCH_SECONDS=8 \
  TERRA_CPU_SCOPES=1 TERRA_BENCH_LOAD_SECONDS=240 WGPU_BACKEND=vulkan \
  target/debug/terra-client --smoke-test-benchmark
```

Fuer den Vorherpfad zusaetzlich `TERRA_SHADOW_ENCODERS=per_face` setzen.
Fuer 4K: `configs-user` im Phase-125-Verzeichnis, Kamera `974,74.8,-557`,
Ziel `974,74.1,-562`, Wetter `rain`. Fuer die Schattenkontrolle zusaetzlich
`TERRA_GPU_DRAW_DIAGNOSTICS=1`, ohne diese Werte als regulaere FPS auszugeben.
Die 240 Sekunden sind ausschliesslich eine Ladefrist; gemessen werden acht
Sekunden nach Laden/Aufwaermen. Keine neuen 10-/15-/30-Minuten-Tests.

Die NPC-Galerie ist mit `TERRA_SMOKE_VILLAGERS=1`,
`TERRA_CAPTURE_DIR=<Zielordner>` und `--smoke-test-creatures` reproduzierbar.
Die Regenflugroute verwendet `configs-seed7`, `TERRA_BENCH_GRAPHICS=1`,
`TERRA_BENCH_SECONDS=24`, `TERRA_BENCH_WEATHER=rain` und `--smoke-test-lod`.

## Diagnostik Und Grenzen

Die opt-in CPU-Diagnostik misst jetzt zusaetzlich die wirklichen Systemkoerper
von Mesh-Allokation, Mesh-Asset-Vorbereitung und den zwei Bind-Group-Systemen.
Die Wrapper behalten Bevy-Zugriffsdeklarationen, Funktionssets, Reihenfolge
und Bedingungen. Ohne `TERRA_CPU_SCOPES` werden sie nicht installiert.
Die aelteren `.body`-losen Zwischenintervalle koennen sich ueberlappen und
sind keine isolierten Funktionskosten.

Im Nachtblick kostet `prepare_preprocess_bind_groups` weiterhin etwa
1,54 ms, `prepare_mesh_bind_groups` etwa 0,13 ms. Im Seed-7-Flug kostet die
Mesh-Allokation/der Upload im Mittel 0,48 ms, reine Mesh-Asset-Vorbereitung
0,031 ms. Grosse Uploads beim Laden bleiben ein eigener Engpass; der neue
Code beansprucht nicht, alle Streaming- oder 4K-Spitzen zu beseitigen. A05
bleibt offen. GPU-Gesamtframezeit und VRAM-Verlauf wurden nicht gemessen.

`before-night.log` ist die erste Messung mit den alten NPCs (50,72 FPS),
`encoder-night.log` ein Zwischenschritt (58,66 FPS). Die Haupttabelle oben
verwendet stattdessen die sauber getrennten Referenz-/Finalpaare mit gleicher
Binaerdatei. `encoder-flight.log` verwendete versehentlich 1920x1200 statt
der Grafikdatei und ist kein Vergleich zur 1280x800-Route.
`aborted-build-overlap.log` wurde wegen Compilerueberlappung abgebrochen und
liefert keinen gueltigen FPS-Wert. GPU-Passdiagnostik aus `before-gpu.log`
ist ebenfalls kein regulaerer FPS-Vergleich und keine GPU-Gesamtframezeit.

## Validierung

Erfolgreich: Formatpruefung, Workspace-Check, strenges Workspace-Clippy fuer
alle Targets, Workspace-Build, native NPC-Galerie, beide nativen FPS-Vergleichspaare,
Schatten-Readback-Vergleich und Seed-7-Regenflug. Kein Shader-/Renderfehler
in diesen finalen nativen Laeufen.

`cargo test --locked --workspace --no-fail-fast` besteht mit **2477 Tests**,
0 Fehlern und 7 absichtlich ignorierten Tests (`tests-workspace.log`).
Auch `git diff --check` ist sauber.

Der finale Dedicated Server startet ohne `DISPLAY` und `WAYLAND_DISPLAY`,
laedt 27 Sections und beendet den Headless-Smoke-Test erfolgreich
(`server-headless.log`). Er verwendet weiterhin keinen Renderer.

Gezielte Regressionen pruefen sichtbare statt innenliegende Sculpt-Flaechen,
zehn unterschiedliche begrenzte Modelle, gueltige Indizes/Normalen,
geteilte sechs Koerperteile, Bodenkontakt der Stiefel, Geh-/Schlafpose und
die erst nach allen 48 Schattenseiten ausgefuehrten Deferred-Befehle.
Der letzte isolierte Render-Testlauf besteht mit 599 Tests und einem
absichtlich ignorierten Test. Die erste Testfassung fand die inzwischen
korrigierte Sohlenkorrektur und einen Diagnosetest, der interne Bevy-Entities
mitzaehlte; diese fehlgeschlagenen Logs sind keine Erfolgsnachweise.
