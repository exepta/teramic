# Phase 101: Backlog-Struktur Und Erste Technische Abschluesse

## Umfang Und Erhalt Der Altaufgaben

TODO.md besitzt genau zwei Hauptgruppen: Features und Optimierungen.
Die neuen elf Featurebereiche sind nach Abhaengigkeiten geordnet. Vorhandene
Doppelungen bei Ruestung, Nachwuchs, Zaehmung, Pferden und Baumfaellen sind
zusammengefuehrt. Optimierungen enthalten einen getrennten Abnahmebereich
fuer echte Mehrrechnertests, andere GPUs/Plattformen und menschliche Art-/Audio-
Abnahme. Eine nicht ausgefuehrte Abnahme wird nicht als Code-Fix ausgegeben.

Die [Zuordnung](migration.md) deckt alle 100 alten offenen Checkboxen genau
einmal ab; Sammeleintraege koennen mehrere Nachfolgeaufgaben haben.
[previous-todo.md](previous-todo.md) bewahrt den vorherigen Inhalt unveraendert,
einschliesslich historischer Leistungsregressionen und Grenzen.
Die gesamte technische Altlast ist ausdruecklich noch nicht abgeschlossen.

## Codeaenderungen

### Kurze Sonnen- Und Mondschatten

`terrain_light_direction` begrenzt die projizierte Schattenlaenge auf hoechstens
die Hoehe des Schattenwerfers. Die minimale virtuelle Lichthoehe betraegt
45 statt 30 Grad. Ein glatter quadratischer Uebergang gleicht die erste Ableitung
an die unveraenderte Sonnenbahn an. Sichtbarer Himmel, Lux, entfernte Schatten
und leichte Prop-Kontaktschatten bleiben bestehen; kein Abschalten der Schatten.
Tests pruefen alle 1440 Tagesminuten und die Geschwindigkeit an den Uebergaengen.

### Audio-Kategorien

Die bestehende AudioConfig und die vorhandenen TiltUI-Audiooptionen erhalten
`animals_volume` und `footsteps_volume`, jeweils Default 1.0. Umgebung behaelt
`ambience_volume`; Master/Mute wirken weiterhin auf alle Busse. Alte TOMLs
werden kompatibel gelesen. Kategorieaenderungen wirken live und werden mit
dem bestehenden Apply/Cancel/Reset-/Speicherpfad behandelt.

Der vorhandene RecordedSoundscape-Mixer trennt Tieraktionen, Fusskontakte und
Umwelt einschliesslich ihrer Hallfahnen. Kategorie-Gains werden geglaettet;
ein neuer Decoder uebernimmt die bereits eingestellten Werte sofort, damit
ein vorab stummer Bus auch bei einem sofort gestarteten Cue still bleibt.
UI-Klicks und Wasserloops verwenden weiter ihre eigenen vorhandenen Regler.
Die Stimmenobergrenze 24 und vorhandene Aufnahmen/Credits bleiben erhalten.
Dies ist keine menschliche Hoerabnahme oder Remote-Audio-Replikation.

### Zusaetzliche Befunde Aus Nativen Gegenproben

Eine Item-Vorschau entfernte ihre Screenshot-Entity nach drei Sekunden
Wallclock, obwohl der GPU-Readback noch lief. Bevy lieferte anschliessend das
Ergebnis an eine bereits entfernte Entity; das Thumbnail ging verloren.
Der bestehende Queuepfad behaelt nun seinen einzigen laufenden Capture samt
Handles bis zum tatsaechlichen Callback. Spaeter Erfolg wird publiziert;
bereits bereitliegende Ergebnisse werden vor der Deadline verarbeitet.
Keine zusaetzliche Retired-Liste, kein hoeheres Inflight-Budget und kein
pauschales Unterdruecken von Rendererwarnungen.

Die native Menueprobe validiert ComputedNode-Geometrie jetzt nach PostUpdate
in Last und wartet nach Welterstellung auf das geschlossene Dialogmodell.
Die strengen Footer-/Viewport-Assertions bleiben erhalten. Die vorherige
450-ms-Wallclockpause garantierte bei langen Frames keine neue Layoutrevision.
Das aendert nur die Probe, nicht die produktive Menueanordnung.

Ein zweiter reproduzierter Fehler betrifft Wasserfall-Audio: Am unteren Becken
bei Seed 42042, `(-423.5, 29.883644, 614.5)` lag der Oberlauf auf 59.71662 m,
der Unterlauf auf 28.16364 m. Die alte Suche verwarf Oberlaufkandidaten oberhalb
von Spielerhoehe + 4 m, obwohl der hoerbare Fallfuss nur 2.54 m entfernt war.
Der [lesende Repro](water-audio-repro.log) belegt die verworfenen Kandidaten.
Der erste Diagnoseversuch ohne aktivierte Fluesse war nicht verwertbar;
der korrigierte Repro verwendet den tatsaechlichen Legacy-Terrainvertrag.
Die Erkennung hoher Fluss-Oberlaeufe erfolgt nun vor dem Hoehenfilter;
der Klang bleibt am wirklichen unteren Becken. Still-/Flussmix, Scanraster
17 x 17, Halbsekunden-Takt, Quellvalidierung und Wanddaempfung bleiben erhalten.
Der finale Test verwendet feste Legacy-TOML statt editierbarer Benutzerdateien.

### Datengetriebene Mob-Spawns

Die vorhandene MobDefinition erhaelt optionale, validierte Spawnfelder fuer
Biomfilter, Herdengroessen und marine Wasser-/Tiefenregeln. Zehn marine JSONs
enthalten die bisherigen Gewichte ausdruecklich. Ganzzahlige Standardwerte
erhalten die alten Seed-Residuen exakt; ein 10000-Seed-Vergleich prueft dies.
Bruchgewichte verwenden einen deterministischen Bruchzahlzug statt Modulo 0.
Die tatsaechliche Kandidatengenerierung liest die Einstellungen, auch bei
Landtieren/Bienen. IDs, Habitate und bereits gespeicherte Populationen werden
nicht ersetzt. Schema, Beispiele und Grenzen: [Mob-README](../../../../assets/terra/mobs/README.md).
Fehlende Sections am konfigurierten Tiefenziel fuehren zu einem spaeteren
Versuch, statt das Habitat voreilig dauerhaft als verarbeitet zu markieren.
Die Freigabe prueft die gesamte von der Kollisionspruefung besuchte Tier-AABB;
Regressionen laden Ziel-/Oberkoerper-Sections spaeter nach und erhalten alte IDs.
Nahrungslisten, Verbrauchsraten und artspezifische Populationslimits sind offen.

### Wasserfall-Verdeckung An Teilbloecken

Eine nicht fluessigkeitsdurchlaessige Nachbarzelle verdeckte bisher stets die
gesamte Wasserfallseite, auch bei Slabs, Treppen oder schmalen Objekten.
Der vorhandene Wasser-Mesher schneidet jetzt nur deren tatsaechliche ShapeBoxes
aus. Sichtbare Fragmente behalten UV-/Gischtkontinuitaet und werden im selben
Wasserpatch gebuendelt. Keine Entity/Materialinstanz pro Fragment.

Der Worker erhaelt begrenzte Shape-Snapshots benachbarter Teilbloecke,
einschliesslich gespeicherter Orientierungen ungeladener Nachbarn.
Vorhandene Epochen pruefen die Aktualitaet beim Veroeffentlichen.
Weil eine Nachbartreppe auch ihren eigenen Anschluss liest, invalidieren
Block-/Objektaenderungen Wassergeometrie mit einem Zwei-Zellen-Halo.
Gewoehnliche Fluidticks behalten den bisherigen Ein-Zellen-Halo. Ein eigener
Test prueft geaenderte Anschluesse zwei Zellen entfernt, bereits publizierte
Patches und die Ablehnung veralteter laufender Jobs, ohne Terrain-Rebuilds
ueber dessen bestehenden Halo hinaus anzufordern.
Regressionen pruefen alle 24 Treppen- und 58 Slabzustaende, die vier Seiten,
positive/negative Section-Grenzen, freien und verdeckten Flaecheninhalt,
gueltige Indizes/Winding sowie Entfernen einer Quelle mit Drainage/Reload.

**Grenze:** Fluessigkeit belegt weiterhin nur die bisher zugelassenen Zellen.
Das ist kein vollstaendiges Waterlogging und kein Wasserfilm in der freien
oberen Haelfte einer unteren Slab. Die uebrige Teilblock-Wasseraufgabe bleibt
offen; feste Holz-/Steingeometrie wird nicht unsichtbar mit Wasser gefuellt.

## Validierung

Alle fuenf zentralen Pruefungen enden mit Exitcode 0. Compilerlaeufe verwenden
`CARGO_BUILD_JOBS=1`; die Testsumme umfasst 1703 bestandene Tests, keinen
Fehlschlag und zwei bereits vorhandene ignorierte Tests.

| Pruefung | Rohlog |
| --- | --- |
| `cargo fmt --all --check` | [fmt.log](fmt.log) |
| `cargo check --locked --workspace` | [check.log](check.log) |
| `cargo test --locked --workspace` | [tests.log](tests.log) |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | [clippy.log](clippy.log) |
| `cargo build --locked --workspace` | [build.log](build.log) |

Neue Regressionen pruefen unter anderem Kategorie-Stille ab dem ersten
Audiosample, verzoegerten Screenshot-Erfolg ohne verlorene Entity, Spawn-Retry
bei fehlenden Koerper-Sections sowie Teilblock-Wasserflaechen und deren
Invalidierung ueber Section-Grenzen.

### Native Kurzpruefungen

Alle folgenden Fixtures enden mit Exitcode 0; konkrete Warnungen sind unten
getrennt aufgefuehrt. Es lief jeweils nur ein Client.

- Headless Dedicated: `--smoke-test --no-save`, ohne DISPLAY/WAYLAND_DISPLAY,
  27 Sections. [server-headless.log](server-headless.log).
- Tageslicht: 25 Aufnahmen einschliesslich Abend/Morgen. Die native
  17-Uhr-Ansicht wurde auf kurze Schatten kontrolliert.
  [daylight.log](daylight.log), [Bilder](native/daylight/).
- Vollstaendige Menueprobe nach den Queue-/Schedulekorrekturen: Seiten,
  Audio-Apply/Dateiinhalt/Mute/Reset, Minimap, Eingaben und Display-Rollback.
  Keine WARN-/ERROR-Zeile im finalen [menu.log](menu.log).
  Die [Audioansicht](native/menu-final/audio-applied.png) wurde visuell
  kontrolliert. Fruehere Fehlversuche bleiben als `menu-first.log`,
  `menu-retry.log` und `menu-gameplay.log` erhalten; sie belegen die
  Layout-Timing- und Screenshot-Lifecycle-Befunde, keine erfolgreiche Abnahme.
- Schritte: 27 eingespeiste Bodenkontakte ueber neun Materialien und echten
  AudioSink konsumiert. [steps.log](steps.log). Kein Beleg fuer menschlich
  gehoerte Klangqualitaet oder einen echten Lauf ueber alle Materialien.
- Fluessigkeit mit echter UDP-Autoritaet: Quelle setzen, 79 Flusszellen,
  sieben entfernte Props, Abbauverbot, Ersetzen und vollstaendige Drainage.
  [fluid-source.log](fluid-source.log), [Bilder](native/fluid/).
  Die Teilblock-Clipgeometrie selbst ist durch CPU-Mesh-Regressionen, nicht
  durch eine eigens aufgebaute native Slab-/Treppenszene geprueft.
- Wasser-Audio nach dem reproduzierten Hoehenfilterfehler: realer Sink,
  Spielerhoerposition unabhaengig von der Kameraposition, Unterwasserfilter,
  getrennte Busse/Mute, Wiederherstellung, Pause und Session-Cleanup.
  Der Fallfuss liefert trocken links/rechts Gain 0.21244/0.27390 statt null.
  [water-audio.log](water-audio.log). Der urspruengliche Timeout steht in
  [water-audio-first.log](water-audio-first.log).

### Performance-Kontrollen Auf Drei Seeds

Ryzen 9 5900X, RX 6950 XT, RADV/Mesa 26.2.4-arch3.1, Vulkan, optimiertes
Dev-Profil. Integrierter UDP-Server, Survival-HUD, Minimap und TiltUI aktiv.
Keine parallelen Compiler, VSync/FPS-Limit aus, High-Schatten/Wasser,
Ultra-Wetter, Rise-Chunkanimation, LOD 1.6, FOV 90, UI-Skalierung 1.
Physische Aufloesung wird im Test validiert; Wetter nutzt Serverbefehle.
Isolierte Konfigurationskopien, keine veraenderten Benutzerwelten.

| Szene | Seed | Messfenster | FPS | p95 / p99 (ms) | RSS am Ende (MiB) |
| --- | --- | ---: | ---: | ---: | ---: |
| Gehen/Rueckweg, 2880 x 1620, Sichtweite 12, klar | 42042 | 45 s | 110.90 | 12.139 / 13.948 | 1302.89 |
| Stand, 3840 x 2160, Sichtweite 20, Regen/Meer | 2262437629133911810 | 20 s | 89.52 | 14.918 / 22.625 | 1808.75 |
| Stand, 2880 x 1620, Sichtweite 12, Schnee 3 | 104729 | 20 s | 110.39 | 13.381 / 16.192 | 1388.04 |

CPU-Wall-Frames nach Szenenbereitschaft, Screenshot-Nachbarframes ausgeschlossen.
RSS ist weder Peak-Speicher noch VRAM. Die Gehroute bestaetigt einen echten
Hin-/Rueckweg ueber rund 64 m mit lokaler und autoritativer Position.
Im Regenlauf sind 11798 Sections geladen, 17520 Mesh-Assets, 94534 Props,
1982 Prop-Batches und 987 Wassertiles vorhanden; die Qualitaet wurde nicht
fuer diese Kontrollen reduziert.

Rohlogs: [Gehen](control-clear.log), [Regen](control-rain.log),
[Schnee](control-snow.log). Die [Schneeaufnahme](native/control-snow.png)
wurde visuell kontrolliert. Fuer Klar/Regen wurden in diesen Kontrolllaeufen
keine Screenshots aufgenommen.

Das sind aktuelle Kontrollen, kein isoliertes Vorher-/Nachherpaar fuer die
Phase-101-Aenderungen und kein Langzeitnachweis. Historische Phase-100-Paare
bleiben im [eigenen Bericht](../inventory-idle-regression-2026-10-22/README.md).
Keine neuen 10-/15-/30-Minuten-Laeufe und keine allgemeine FPS-/Speicherparitaet.

Reproduktion verwendet dieselben isolierten TOML-Grundlagen wie Phase 100:

```bash
TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/inventory-idle-regression-2026-10-22/configs/clear" \
TERRA_MENU_CONFIG_DIR="$PWD/tools/benchmarks/results/inventory-idle-regression-2026-10-22/configs/clear" \
TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 TERRA_BENCH_GRAPHICS=1 \
TERRA_BENCH_SECONDS=45 TERRA_SOAK_ROUTE=walk TERRA_SOAK_ROUTE_DELAY_SECONDS=5 \
TERRA_CPU_SCOPES=1 TERRA_SOAK_REPORT=1 \
target/debug/terra-client --smoke-test-benchmark
```

Regen: `configs/rain`, 20 s, ohne Gehroute,
`TERRA_BENCH_WEATHER=rain`. Schnee: `configs/snow`, 20 s, ohne Gehroute,
`TERRA_BENCH_WEATHER='snow 3'`. Kartencaches entstehen im gewaehlten
Testverzeichnis; die Benchmarkwelt wird nicht als Benutzerwelt gespeichert.

## Verbleibende Grenzen

- Der gesamte alte technische Backlog ist nicht abgeschlossen. Unter anderem
  Waterlogging, Nahrungs-/Populations-JSONs, Paging, GI und weitere
  Streaming-/Wetter-/Mehrspieleraufgaben bleiben offen in TODO.md.
- Menschliche Art-/Hoerabnahmen sowie andere GPUs, Windows/macOS und echte
  Mehrrechnersitzungen wurden hier nicht durchgefuehrt.
- Die fruehere Schrittprobe protokolliert beim AppExit noch
  `Failed to send screenshot: sending on a closed channel`. Das ist eine
  getrennte Shutdown-Grenze des Engine-Readbacks, nicht durch den Fix fuer
  verlorene laufende Item-Vorschauen als geloest ausgegeben.
- Die spezialisierte Fluid-Source-Fixture meldet einmal nicht behandelte
  CreatureStates, die Lightyear verwirft. Im 4K-Regenlauf gibt es eine
  initiale Winit-Monitorwahlwarnung; die spaetere physische Aufloesung ist
  bestaetigt 3840 x 2160. Keine dieser Meldungen wurde weggefiltert.
- Die Menuegegenprobe zeigte eine Pause von 4.469 s. Der Apply-Pfad speichert
  mehrere Konfigurationen synchron mit fsync; eine isolierte Zeitmessung
  dieses Pfads fehlt. Er wird deshalb unter O17 weitergefuehrt, nicht als
  bewiesene Ursache dieser gesamten Pause oder bereits korrigiert behauptet.
- Separate GPU-Framezeiten/VRAM und langfristige Ressourcenplateaus wurden
  durch die kurzen Kontrollen nicht nachgewiesen.

## Kompatibilitaet

Keine Dependency-Upgrades, kein zweites Registry-, Audio-, Prop- oder
Fluessigkeitssystem. Bestehende Block-/Mob-/Welt-IDs, Generatorversion 5,
Protokoll 55 und Speicherformat 2 bleiben erhalten. Dedicated bleibt headless.
Neue Spawnfelder sind optional und Audiofelder besitzen kompatible Defaults.
