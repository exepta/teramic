# Phase 100: Geschlossenes Inventar Verursachte FPS-Regression

## Ursache Und Korrektur

Die Phase-99-Aenderung hielt nicht nur die Inventarhuelle, sondern auch Slots,
Itembrowser, Ausruestung, Crafting und Tooltips geschlossen im TiltUI-Baum.
`display: none` verhindert das Zeichnen, suspendiert aber nicht die reaktiven
Bindings und Komponenten. Die fortlaufend aktualisierten HUD-/Minimap-Werte
lassen TiltUI auch diese unsichtbaren Bindings erneut auswerten. Mehr als
tausend unnoetige Entities blieben so waehrend des normalen Spiels aktiv.

Die Korrektur ist bewusst eng: `inventory-panel.component.html` materialisiert
den Inhalt nur bei `inventoryView.open`. Die leichte Backdrop-Entity und der
Welt-HUD bleiben bestehen. Gemeinsame Itembilder, Metadaten und Layoutschutz
werden weiterverwendet. Vermessung und Positionierung vor dem ersten sichtbaren
Frame bleiben erhalten. Keine Renderer-, Terrain-, Wasser-, Netzwerk- oder
Qualitaetsaenderung; keine neuen Abhaengigkeiten und keine Formatversionen.

Die native Inventarprobe prueft nun vor dem ersten Oeffnen und nach beiden
Schliessvorgaengen, dass Panel, Slot-Entities und Inventar-Tooltips wirklich
entfernt sind und keine verwaisten Tooltip-Portale verbleiben. Wiederoeffnen
prueft Bilder und Layout statt die Wiederverwendung unsichtbarer Slots zu
verlangen. Backdrop und Hotbar muessen weiterhin dieselben Entities bleiben.

## Messbedingungen

Ryzen 9 5900X, RX 6950 XT, RADV/Mesa 26.2.4-arch3.1, Vulkan, vorhandenes
optimiertes Dev-Profil. Ein Client mit echter integrierter UDP-Autoritaet,
Survival-HUD, Minimap und TiltUI. Keine parallelen Compiler waehrend der
FPS-Messungen. Isolierte Konfigurationskopien; keine Benutzerwelt veraendert.

VSync und FPS-Limit aus, High-Schatten/Wasser/Blockdetails, Ultra-Wetter,
volle Vegetationsdichte, Rise-Chunkanimation, LOD-Faktor 1,6, FOV 90.
Physische Aufloesung und effektive Sichtweite werden vom Test validiert.
Wetterbefehle laufen ueber die echte Serverautoritaet.

Die erste Nachher-Standmessung `dom-idle.log` verwendet absichtlich dasselbe
Vorherbinary wie die Baseline: Nur die zur Laufzeit geladene HTML-Vorlage wurde
um die Open-Bedingung erweitert. Damit ist die Inventar-Lebensdauer direkt
isoliert. Geh-/Regen-/Schnee-Nachherlaeufe verwenden den neu gebauten Client.

## Gemessene Werte

CPU-Wall-Frames nach Szenenbereitschaft, Screenshot-Nachbarframes ausgeschlossen.
RSS ist der Prozesswert am Ende des jeweiligen kurzen Laufs, kein Peak-VRAM.

| Szene | Seed | Dauer | FPS vorher | FPS nachher | p99 vorher/nachher (ms) | RSS vorher/nachher (MiB) |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Stand, 2880 x 1620, Sichtweite 12, klar | 42042 | 35 s | 63,27 | 111,09 | 20,513 / 13,652 | 1363 / 1267 |
| Gehen mit Chunkwechsel und Rueckweg, gleiche Grafik | 42042 | 45 s | 63,10 | 114,62 | 21,944 / 14,007 | 1422 / 1287 |
| Stand, 3840 x 2160, Sichtweite 20, Regen und Meer | 2262437629133911810 | 20 s | 34,53 | 68,58 | 42,239 / 28,680 | 1891 / 1854 |
| Stand, 2880 x 1620, Sichtweite 12, Schnee Stufe 3 | 104729 | 20 s | nicht gemessen | 115,19 | nicht gemessen / 14,847 | nicht gemessen / 1381 |

Die Gehroute wartet fuenf Sekunden, laeuft dann mit dem normalen Controller
64 m hin und zurueck und beginnt einen weiteren Hinweg. Ein vollstaendiger
Rueckweg ist in beiden Logs durch lokale und autoritative Positionen belegt.
Keine Flugbewegung, Teleports oder Terrain-Edits waehrend dieser Route.

Im identischen Standfenster nach 30 s sinken die lebenden Entities von 12059
auf 10824. Terrain bleibt bei 4375 geladenen und 2490 gerenderten Sections,
6363 Mesh-Assets, 44022 repraesentierten Props und 580 Prop-Batches. Im
Regenvergleich bleiben 11798 geladene / 7211 gerenderte Sections, 17520
Mesh-Assets, 94534 Props, 1982 Prop-Batches und 987 Wassertiles erhalten.
Die Verbesserung stammt nicht aus ausgeduennter Weltgeometrie.

Eine anschliessende Rueckkontrolle mit wiederhergestelltem alten HTML und
dem alten Binary erreicht trotz inzwischen warmer Kartencaches wieder nur
68,33 FPS / p99 18,103 ms (`control-retained-idle.log`). Es gibt Laufstreuung;
die Regression kehrt mit dem alten Slot-Lebenszyklus dennoch deutlich zurueck.
Die korrigierte Vorlage ist danach wiederhergestellt.

## Belege Und Reproduktion

Rohlogs: [Stand vorher](baseline-idle.log), [Stand nur HTML korrigiert](dom-idle.log),
[Rueckkontrolle](control-retained-idle.log), [Gehen vorher](baseline-walk.log),
[Gehen nachher](after-walk.log), [Regen vorher](baseline-rain-fullscreen.log),
[Regen nachher](after-rain.log), [Schnee nachher](after-snow.log).

Die isolierten TOML-Dateien stehen unter `configs/`. Beispiel fuer Clear/12:

```bash
TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/inventory-idle-regression-2026-10-22/configs/clear" \
TERRA_MENU_CONFIG_DIR="$PWD/tools/benchmarks/results/inventory-idle-regression-2026-10-22/configs/clear" \
TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 TERRA_BENCH_GRAPHICS=1 \
TERRA_BENCH_SECONDS=35 TERRA_CPU_SCOPES=1 TERRA_SOAK_REPORT=1 \
cargo run --locked -p terra-client -- --smoke-test-benchmark
```

Gehen: zusaetzlich `TERRA_SOAK_ROUTE=walk TERRA_SOAK_ROUTE_DELAY_SECONDS=5`,
Messdauer 45 s. Regen: Verzeichnis `configs/rain`, `TERRA_BENCH_WEATHER=rain`,
20 s. Schnee: `configs/snow`, `TERRA_BENCH_WEATHER='snow 3'`, 20 s.
Reproduktion erzeugt Kartencaches in der ausgewaehlten Konfigurationskopie;
die integrierte Benchmarkwelt wird nicht als Benutzerwelt gespeichert.
Die urspruengliche Vorlage ist unter `inventory-panel.before.html` archiviert.

## Validierung

Alle Befehle erfolgreich, mit `CARGO_BUILD_JOBS=1` fuer die Compilerlaeufe:

| Pruefung | Beleg |
| --- | --- |
| `cargo fmt --all --check` | [fmt.log](fmt.log), leer bei Erfolg |
| `cargo check --locked --workspace` | [check.log](check.log) |
| `cargo test --locked --workspace` | [tests.log](tests.log): 1678 bestanden, zwei bestehende ignoriert |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | [clippy.log](clippy.log) |
| `cargo build --locked --workspace` | [build-final.log](build-final.log), Client und Dedicated Server |
| Nativer Creative-Inventarlauf, echte OS-Maus | [inventory-creative-final.log](inventory-creative-final.log), elf Aufnahmen |
| Nativer Survival-Inventarlauf, echte OS-Maus | [inventory-survival-final.log](inventory-survival-final.log), sechs Aufnahmen |
| Dedicated Server ohne DISPLAY/WAYLAND_DISPLAY | [server-headless.log](server-headless.log), 27 Sections |

Inventarlaeufe pruefen Drag-/Split-Transfers, Crafting, Scrollen, Tooltipposition
in jedem sichtbaren Frame, Muelltonne, Freigabe geschlossener Inhalte,
Wiederoeffnen und den weiter montierten HUD unter Pause/Optionen.
Die ersten erweiterten Reopen-Checks verwendeten noch den absichtlich auf
960 x 360 verkleinerten Scroll-Stress-Viewport und scheiterten an der normalen
Mindestgroessenpruefung. Die Fixture stellt jetzt vor dieser normalen
Layoutpruefung wieder 1280 x 800 her; der Scroll-Stresstest bleibt unveraendert.
Beide ersten Logs bleiben als `inventory-creative.log` und
`inventory-survival.log` erhalten, beide finalen Laeufe bestehen.

Native Aufnahmen (Regen/Meer und Creative-Tooltip auch direkt visuell geprueft):
[Regen/Meer nachher](native/after-rain.png),
[Schnee Stufe 3](native/after-snow.png),
[Inventar](native/inventory-open.png),
[Creative-Tooltip](native/inventory-hotbar-tooltip.png),
[Survival-Tooltip](native/inventory-survival-browser-tooltip.png),
[HUD unter Optionen](native/hud-options-overlay.png).

## Grenzen

Keine neuen 10-/15-/30-Minuten-Tests, entsprechend dem ausdruecklichen Wunsch.
Keine gemessene komplette GPU-Framezeit und keine universelle FPS-Garantie.
Zeit, Wetter und Tiere laufen normal weiter; die Welten sind deterministisch,
aber nicht jedes animierte Einzelbild oder der Wetterbeginn bitidentisch.
Hohe Sichtweiten und Regen besitzen weiterhin messbare Frame-Spitzen.
Das aktive Inventar hat weiter echte Aufbau-/Bindingkosten; es ist nur nicht
mehr waehrend des gesamten Spiels unsichtbar aktiv. Dies ist keine pauschale
Behebung aller historischen Streaming- oder Speicherprobleme.

Ein erster 4K-Fensterlauf wurde korrekt verworfen: Der Fenstermanager begrenzte
die Hoehe auf 2124 statt 2160 Pixel. Die gueltigen 4K-Paare verwenden Borderless
und bestaetigen 3840 x 2160. Beide melden einmal die Winit-Warnung zur noch
nicht bekannten Monitorwahl bei Fenstererstellung; der spaetere Modus stimmt.
