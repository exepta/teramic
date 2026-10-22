# Phase 103: Inventar-FPS und reaktive UI-Kosten

Stand: 2026-10-22. Dieser Bericht dokumentiert tatsaechlich ausgefuehrte kurze
native Vergleichslaeufe, zusaetzliche Regen-/Schnee-Nachherproben und die
abgeschlossene Cargo-Abnahme. Zwei strenge stationaere Schnee-Inventarversuche
sind ungueltig geblieben. Unten stehen keine erfundenen Testresultate oder
Langzeitversprechen.

## Reproduktion und Eingrenzung

Das gemeldete Inventarproblem wurde auf der vorhandenen Maschine reproduziert:
Bei 3840 x 2160 und Sichtweite 20 fiel der gleiche unveraenderte Kamerablick von
91.78 FPS mit geschlossenem Inventar auf 32.79 FPS mit offenem Inventar. Im
offenen Inventar existierten 2015 UI-Nodes und 571 Property-Binding-Nodes. Es
war weiterhin genau eine aktive Kamera vorhanden, die eigentliche Weltkamera.

Der separat instrumentierte GPU-Pass `render/ui/elapsed_gpu` benoetigte im
offenen 4K-Vorherlauf im Mittel 0.349 ms, p99 0.381 ms. Das ist kein gesamter
GPU-Frame und keine komplette CPU-UI-Messung. Der Einbruch auf rund 30 ms
CPU-Wall-Framezeit wurde nicht durch einen zusaetzlichen permanenten
Inventar-3D-Renderpass erklaert. Item- und Avatar-Vorschaukameras rendern einmal
und werden danach entfernt.

Die untersuchten UI-Codepfade enthielten konkrete unnoetige Kopien:

- Einfache Bindings wie `inventoryView.title` klonten bisher den ganzen
  Root-JSON-Wert inklusive Itemkatalog, bevor sie einen einzelnen Wert lasen.
- Auch zusammengesetzte Ausdruecke traversierten geklonte Container.
- Kleine Updates eines anderen HUD-Stores serialisierten erneut unveraenderte
  Inventar-/Rezept-/Menue-/Karten-Stores und kopierten deren Snapshots.
- Die Flow-Sortierung ermittelte dieselbe Elternkettentiefe mehrfach waehrend
  der Sortiervergleiche.
- Teramics Bildbruecke suchte pro Icon und Frame linear durch Itemvorschauen
  und erzeugte dabei fuer Kandidaten immer wieder Identifier-Strings.

## Codekorrekturen

`tilt-ui-runtime` bleibt bei Version **0.1.0** und wird gezielt ueber
`[patch.crates-io]` aus `vendor/tilt-ui-runtime` eingebunden. Die MIT-/Apache-
Lizenztexte liegen bei. Im Lockfile entfallen fuer dieses Paket lediglich
Registry-Quelle und Checksumme; es gibt keinen Versionssprung. Details stehen
in [PATCHNOTES.md](../../../../vendor/tilt-ui-runtime/PATCHNOTES.md).

- `component/binding_runtime.rs`: Einfache Pfade traversieren geliehene
  JSON-Teilbaeume. Nur das benoetigte Endergebnis wird gegebenenfalls kopiert.
- `component/expression.rs`: Zusammengesetzte Ausdruecke verwenden intern
  geliehene Roots/Members bzw. `Cow<Value>`. Grammatik, lokale Scopes,
  Alias-Prioritaet, Indizes, Methodenaufrufe und Short-Circuiting bleiben erhalten.
- `component/binding.rs`: Store-Snapshots werden nur bei Ressourcenveraenderung,
  Erscheinen oder Entfernen aktualisiert. Aliasse teilen eine `Arc`-Allokation;
  gleiche serialisierte Werte behalten Allokation und Revision.
- `component/flow.rs`: `sort_by_cached_key` berechnet die Hierarchietiefe einmal
  je wartendem Flow-Node. Die stabile Eltern-vor-Kind-Reihenfolge bleibt erhalten.
- `crates/terra-ui/src/inventory.rs`: Ein lokaler Alt-Text-zu-Handle-Index wird
  nur bei Aenderung der Vorschauquellen aufgebaut. Neue oder veraenderte
  Metadata/ImageNodes werden gezielt gebunden. Blinzelportraits invalidieren
  nicht den Itemkatalog. Unveraenderte Handles schreiben keine Change-Ticks;
  TiltUIs geerbte Opacity wird nicht auf Weiss oder volle Deckkraft gesetzt.

Das originale veroeffentlichte Runtime-Crate enthielt vier in seinen Tests
referenzierte Showcase-Fixtures nicht. Diese vier HTML-/CSS-Dateien wurden
unveraendert aus dem urspruenglichen lokalen Repository-Commit
`20995eb12cb7591996a0b60af4d7fe4c8193f507` per `git archive` uebernommen und unter
`vendor/tilt-ui-runtime/tests/fixtures/basic` abgelegt. Die zugehoerigen
`include_str!`-Pfade in den Flow-Tests wurden angepasst. Dies aendert keine
Produktivfunktion, Paketversion oder weitere Lockfile-Aufloesung.

Es wurden weder Aufloesung, Sichtweite, Vegetationsdichte, Wasserqualitaet,
Schatten noch UI-FPS fuer die Nachmessung pauschal reduziert. Kein FPS-Limit
und keine neue Frame-Pacing-Sperre wurden als Loesung eingefuehrt.

## Messrahmen

- CPU: Ryzen 9 5900X; GPU: RX 6950 XT, 16 GiB; RAM: 32 GiB.
- Vulkan/RADV, Mesa `26.2.4-arch3.1`; identischer nativer Rechner.
- Optimierter Dev-Build: eigene Pakete `opt-level=1`, Abhaengigkeiten
  `opt-level=3`, Debug-Informationen aktiv. Kein Release-FPS-Versprechen.
- Echter integrierter UDP-Server, Survival-Client, echte TiltUI, Minimap und
  Status-HUD aktiv. Kein Dummy-Inventar und keine Offline-Ersatzsimulation.
- `clear`: Seed 42042, 2880 x 1620, Sichtweite 12, UI-Scale 1.0, UI-FPS 60.
- `user`: Seed 2262437629133911810, 3840 x 2160, Sichtweite 20,
  UI-Scale 1.1, UI-FPS 90. Grafikdatei und Einstellungen sind archiviert.
- Beide: vertikale Sichtweite 3, VSync aus, FPS-Limit 0, Schatten/Wasser/Block-
  Details High, Cave Medium, Wetter Ultra, Vegetationsdichte 1.0, FOV 90,
  Chunkanimation Rise und LOD-Faktor 1.6. Die Minimap bleibt bei 60 FPS.
- Keine nebenher laufende Kompilierung und kein zweiter Spielclient waehrend
  der Messfenster. Sonstige Desktoplast wurde nicht exklusiv kontrolliert.

Der neue opt-in Test `apps/client/src/inventory_benchmark.rs` wartet auf
`benchmark_ready_time`, echte UI-Readiness und integrierte Autoritaet. Er misst
je 12 Sekunden geschlossen, offen und erneut geschlossen. Zwischen den Phasen
sendet er echte `InventoryAction::Toggle`, wartet auf `InventoryView` und den
berechneten Inventarpanel-Node und laesst danach jeweils 2 Sekunden Settle-Zeit.
Eine bewegte oder fehlende Weltkamera, unpassender UI-/Servermodus, eine
gleichzeitig aktivierte Laufroute oder eine ueberschrittene Deadline fuehren
zu einem Fehler statt zu einem gueltigen Ergebnis.

Samples sind begrenzt; in allen hier berichteten Inventarphasen wurden null
Samples verworfen. FPS sind CPU-Wall-Frames pro Messfenster, Perzentile folgen
Nearest Rank. GPU-Readbacks werden separat je instrumentiertem Pass gesammelt
und nicht zu einer angeblichen Gesamt-GPU-Zeit addiert. CPU-Scope-Berichte
verwenden Fensterdeltas; deren `cumulative_max_ms` schliesst hingegen fruehere
Phasen und Startup ein und ist kein Fenstermaximum.

## Ergebnisse: unveraenderte Kameraposition

Framezeiten in ms, RSS als Prozess-RAM am jeweiligen Phasenende in MiB.
`Zu` = geschlossen; `Auf` = offen; `Wieder zu` = erneut geschlossen.

| Szene | Build | Phase | FPS | Median | p95 | p99 | Maximum | RSS MiB |
|---|---|---|---:|---:|---:|---:|---:|---:|
| clear | Vorher | Zu | 102.77 | 9.51 | 12.39 | 13.91 | 32.95 | 1224.16 |
| clear | Vorher | Auf | 43.22 | 22.48 | 27.75 | 33.62 | 70.74 | 1347.57 |
| clear | Vorher | Wieder zu | 103.22 | 9.52 | 11.86 | 14.23 | 24.16 | 1354.20 |
| clear | Nachher | Zu | 111.22 | 9.02 | 11.65 | 12.96 | 16.88 | 1228.68 |
| clear | Nachher | Auf | 90.99 | 10.86 | 12.67 | 15.68 | 42.52 | 1372.07 |
| clear | Nachher | Wieder zu | 112.88 | 8.83 | 11.60 | 13.80 | 17.98 | 1378.71 |
| user / 4K | Vorher | Zu | 91.78 | 10.76 | 13.74 | 15.48 | 22.42 | 1732.88 |
| user / 4K | Vorher | Auf | 32.79 | 30.34 | 33.05 | 35.16 | 36.89 | 1808.23 |
| user / 4K | Vorher | Wieder zu | 81.34 | 12.21 | 14.10 | 16.20 | 17.94 | 1810.23 |
| user / 4K | Runtime-Zwischenstand | Zu | 92.23 | 10.80 | 13.78 | 15.09 | 17.09 | 1720.17 |
| user / 4K | Runtime-Zwischenstand | Auf | 50.32 | 19.74 | 21.46 | 23.80 | 29.51 | 1792.16 |
| user / 4K | Runtime-Zwischenstand | Wieder zu | 92.97 | 10.75 | 13.37 | 15.28 | 19.35 | 1794.59 |
| user / 4K | Nachher | Zu | 98.41 | 10.15 | 12.72 | 14.05 | 15.55 | 1749.25 |
| user / 4K | Nachher | Auf | 68.38 | 14.45 | 16.68 | 18.15 | 19.29 | 1820.22 |
| user / 4K | Nachher | Wieder zu | 98.63 | 10.08 | 12.71 | 14.27 | 17.89 | 1822.32 |

Der Runtime-Zwischenstand enthielt bereits geliehene einfache Root-Pfade und
inkrementelle Shared-Snapshots, aber noch nicht Compound-Auswertung, gecachte
Flow-Tiefen oder Teramics neue Bildbruecke. Dieser Lauf ist **kein isolierter
Einzelpatch-Benchmark**. Der endgueltige Patchverbund verbessert in diesen
Messungen das offene Inventar; ein allgemeiner prozentualer FPS-Gewinn fuer
beliebige Szenen wird daraus nicht abgeleitet.

## Ressourcen und GPU-Grenzen

Im 4K-Lauf bleibt die UI-Groesse vor/nach identisch:

| Phase | UI-Nodes | Tilt-Nodes | Property-Binding-Nodes | Bild-Nodes | Aktive Kameras |
|---|---:|---:|---:|---:|---:|
| Zu | 115 | 80 | 31 | 14 | 1 |
| Auf | 2015 | 1328 | 571 | 180 | 1 |
| Wieder zu | 115 | 80 | 31 | 14 | 1 |

Die 4K-Mesh-Assetzahl bleibt in allen Phasen bei 17519, vorher und nachher.
Die Gesamt-Entityzahl geht nach dem Schliessen von rund 30750 wieder auf rund
27750 zurueck. Kleine Differenzen durch lebende Weltobjekte bleiben vorhanden.
Dies belegt keinen mehrminuetigen Leak-Stabilitaetstest.

GPU-UI-Pass im offenen Inventar: vorher 0.349 ms, Runtime-Zwischenstand
0.361 ms, nachher 0.286 ms im Mittel. Die deutliche Framezeitveraenderung ist
damit nicht als ebenso grosse GPU-UI-Einsparung zu interpretieren.

Ein RAM-Gewinn ist **nicht** nachgewiesen: Das offene 4K-Inventar liegt im
Nachherlauf rund 12 MiB ueber Vorher, `clear` rund 24.5 MiB. Nach dem Schliessen
bleiben unter anderem Font-/Bild-Caches und Allocator-Reserven erhalten. Hier
liegen nur RSS-Endpunkte vor, keine Peak-/VRAM-Messungen und keine bewiesene
allgemeine Speicherparitaet.

## Kurzer Lauf-/Streamingvergleich

Separater 45-Sekunden-Lauf mit echter Eingabe und integrierter Autoritaet,
Seed 42042, 2880 x 1620 und Sichtweite 12. Die 64-m-Hin-/Rueckroute wurde
wirklich gelaufen; beide Logs enthalten mindestens eine bestaetigte Rueckkehr.
Kleine Unterschiede der Endposition und der Streamingjobs entstehen im
laufenden Netzwerk-/Frame-Timing und werden nicht als exakt gleicher Frame
ausgegeben.

| Build | FPS | p95 ms | p99 ms | RSS KiB | Geladene / gerenderte Sections | Aktive Props |
|---|---:|---:|---:|---:|---|---:|
| Vorher | 101.43 | 13.34 | 15.21 | 1318520 | 4375 / 2556 | 37266 |
| Nachher | 116.08 | 11.43 | 12.72 | 1319128 | 4375 / 2556 | 37266 |

Dies ist ein positiver kurzer Vergleich mit gleicher Route und Qualitaet,
aber kein Beleg fuer die Behebung saemtlicher Streaming-, GPU- oder
zeitabhaengigen FPS-Probleme. Lange 10-/15-/30-Minuten-Laeufe wurden gemaess
der frueheren Nutzeranweisung nicht angesetzt.

## Wetter: Nachherlaeufe und ungueltige Versuche

Ein zusaetzlicher echter Regenlauf auf Seed 2262437629133911810 bei 4K und
Sichtweite 20 beendete alle drei Phasen. Die offene UI hatte wiederum 2015
Nodes und 571 Property-Binding-Nodes. Es existiert hierzu **kein passendes
Vorher-Regenpaar**; dieser Lauf belegt keine prozentuale Regenverbesserung.

| Phase | FPS | Median ms | p95 ms | p99 ms | Maximum ms | RSS KiB |
|---|---:|---:|---:|---:|---:|---:|
| Zu | 91.82 | 10.61 | 14.76 | 22.34 | 42.87 | 1811160 |
| Auf | 67.00 | 14.46 | 18.58 | 24.41 | 28.88 | 1889676 |
| Wieder zu | 91.46 | 10.88 | 14.32 | 17.02 | 33.32 | 1896392 |

Zwei Versuche mit Seed 104729 und `snow 3` sind **ungueltig**, auch wenn schon
einzelne Phasenwerte im Log stehen. Diese Werte werden nicht als bestandene
Schnee-Abnahme oder belastbares Vergleichsergebnis uebernommen:

- [Erster Schneeversuch](logs/phase103-after-snow.log): Der Windowmanager
  aenderte waehrend des offenen Inventars die physische Aufloesung von
  2880 x 1620 auf 2892 x 1626. Die strenge Aufloesungspruefung brach ab.
  Zusaetzlich wurde ein Rodio-Audio-Buffer-Underrun/Overrun protokolliert.
- [4K-Schneeversuch](logs/phase103-after-snow-4k.log): Die reale Weltkamera
  bewegte sich in `ClosedAgain`; die stationaere Vergleichspruefung brach ab.
  Der Test wurde nicht durch Abschalten der Kamera-Stabilitaetspruefung
  nachtraeglich als bestanden ausgegeben.

Ein separater allgemeiner Schnee-Nachherlauf ohne die strenge stationaere
Inventarvergleichssequenz beendete anschliessend seinen 45-Sekunden-Test:
Seed 104729, 3840 x 2160, Sichtweite 12, UI-Scale 1.0/UI-FPS 60,
Schneestufe 3, Survival-HUD, Minimap, 59 Wassertiles/23436 Wasserdreiecke.
Physische Aufloesung und echter integrierter Server wurden geprueft. Es wurden
keine Bewegungsroute und keine Inventar-Toggles aktiviert. Eine eventuell
durch die lebende Szene veraenderte Spielerhoehe ist hier kein garantiert
konstanter Kamerablick.

| FPS | p95 ms | p99 ms | RSS KiB | Sampleframes |
|---:|---:|---:|---:|---:|
| 99.29 | 14.13 | 15.34 | 1504280 | 4469 |

[Schnee-Nachherlog](logs/phase103-after-snow-general.log) und
[isolierte Grafikdatei](configs/snow/graphics.toml).
Dies ist eine gueltige weitere Seed-/Wetterprobe, **kein** Schnee-Vorherpaar
und **kein** bestandener stationaerer Schnee-Inventarvergleich. Die zwei
frueheren Abbrueche bleiben unveraendert oben dokumentiert. Der gueltige Lauf
enthaelt keine ERROR-Zeile und dieselbe einmalige Monitor-Auswahlwarnung.

## Native Funktionsabnahme

Neben den FPS-Laeufen bestanden die echten nativen Inventar-Fixtures fuer
beide Spielmodi, mit OS-Eingaben ueber `uinput` und Serverbestaetigungen:

- [Creative-Log](logs/phase103-inventory-creative-native.log): Erfolg mit
  11 Captures. Unter anderem echte Drag-/Pickup-Gesten im Inventar und
  Craftinggrid, Count-Erhaltung aller 128 Items, Cursor-/Slot-Muelltonne,
  Outside-Drop, Tooltip, Reopen und weiter sichtbarer Welt-HUD.
- [Survival-Log](logs/phase103-inventory-survival-native.log): Erfolg mit
  6 Captures, unter anderem Survival-Katalog/Tooltip, kompaktes Layout,
  Scroll-Erreichbarkeit, Reopen und erhaltenem Hotbar-/Minimap-HUD.

Die [breite Creative-Ansicht](native/creative/inventory-creative-wide.png)
und die [Survival-Ansicht](native/survival/inventory-survival-browser-tooltip.png)
wurden zusaetzlich visuell geprueft. Weitere gespeicherte Captures liegen unter
`native/creative` und `native/survival`; ihr blosses Vorhandensein wird nicht
als vollstaendige zusaetzliche visuelle Einzelabnahme ausgegeben. Die
Funktionsfixtures pruefen auch bewusst kleine Test-Viewports. Diese
Layout-Sonderfaelle sind keine FPS-Nachmessungen mit verringerter Qualitaet.

## Nachweise und Wiederholung

- [2880p vorher](logs/phase103-before-clear.log),
  [2880p nachher](logs/phase103-after-clear.log).
- [4K vorher](logs/phase103-before-user.log),
  [Runtime-Zwischenstand](logs/phase103-runtime-user.log),
  [4K nachher](logs/phase103-after-user.log).
- [Laufen vorher](logs/phase103-before-walk.log),
  [Laufen nachher](logs/phase103-after-walk.log).
- [Zusaetzlicher Regen-Nachherlauf](logs/phase103-after-rain.log).
- Archivierte reine Benchmark-TOMLs: [clear](configs/clear/graphics.toml),
  [user](configs/user/graphics.toml), jeweils auch Client, Welt, Gameplay und
  Server. Keine privaten Player-Profile, Weltstaende oder Kartencaches kopiert.

Beispiel fuer den aktuellen Inventartest aus dem Repository-Root mit bereits
gebautem Client und einer isolierten Benchmark-Konfigurationskopie:

```bash
env TERRA_CONFIG_DIR="$PWD/target/phase103/user" \
  TERRA_MENU_CONFIG_DIR="$PWD/target/phase103/user" \
  TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 TERRA_BENCH_GRAPHICS=1 \
  TERRA_BENCH_INVENTORY=1 TERRA_BENCH_SECONDS=90 \
  TERRA_RENDER_DIAGNOSTICS=1 TERRA_CPU_SCOPES=1 \
  target/debug/terra-client --smoke-test-benchmark
```

Fuer die Laufroute `TERRA_BENCH_INVENTORY` weglassen, `TERRA_BENCH_SECONDS=45`,
`TERRA_SOAK_ROUTE=walk` und `TERRA_SOAK_ROUTE_DELAY_SECONDS=5` setzen. Nicht
gleichzeitig mit Compiler oder weiterem Spielclient messen. Die abgelegten
TOMLs sind eine Reproduktionshilfe; keine Benutzerkonfiguration ueberschreiben.

## Validierung und offene Abnahme

- Alle sieben Hauptlaeufe sowie der zusaetzliche Regen-Nachherlauf erreichten
  ihre Erfolgsmeldung. Die beiden Schneeversuche sind separat als ungueltig
  dokumentiert und zaehlen nicht dazu.
- Keine ERROR-/Panic-/Use-after-free-Zeile in den sieben Hauptlogs oder dem
  gueltigen Regenlog gefunden. Fehler der ungueltigen Schneeversuche bleiben
  im Nachweis erhalten.
- Die Borderless-4K-Laeufe enthalten jeweils eine Winit-Warnung zur Auswahl
  des aktuellen Monitors beim Erzeugen des Fensters; sie wird nicht verborgen.
- Alle fuenf Workspace-Pruefungen erfolgreich: `cargo fmt --all --check`,
  `cargo check --locked --workspace`, `cargo test --locked --workspace`,
  `cargo clippy --locked --workspace --all-targets -- -D warnings`,
  `cargo build --locked --workspace`. 1742 Tests bestanden, 2 bestehende
  Tests ignoriert. Cargo-Jobs auf 2 begrenzt, keine nativen FPS-Messungen
  gleichzeitig mit Compilerprozessen.
- Der direkte Runtime-Test `cargo test --locked -p tilt-ui-runtime --lib`
  besteht alle 309 Tests, einschliesslich der 10 neuen Regressionen. Auch
  `cargo clippy --locked -p tilt-ui-runtime --all-targets -- -D warnings`
  und die gesonderte Formatpruefung des vendorten Pakets bestehen. Die vier
  Originalfixtures und ihre Lizenz-/Quellzuordnung sind enthalten; kein
  separater Lockfile oder Dependency-Upgrade fuer diese Tests.
- [Headless-Server-Smoke](logs/phase103-headless-server.log) ohne DISPLAY und
  WAYLAND_DISPLAY erfolgreich: 27 Sections, realer UDP-Socket, kein Renderer.
- Regen-Nachherlauf bestanden, aber kein Regen-Vorherpaar. Separater
  Schnee-/Wasser-Nachherlauf auf drittem Seed bestanden; beide stationaeren
  Schnee-Inventarversuche bleiben ungueltig und dieser Vergleich offen.
- [Workspace-Tests](logs/phase103-tests.log),
  [Runtime-Tests](logs/phase103-tests-runtime.log),
  [Check](logs/phase103-check.log), [Clippy](logs/phase103-clippy.log),
  [Runtime-Clippy](logs/phase103-clippy-runtime.log),
  [Build](logs/phase103-build.log), [Fmt](logs/phase103-fmt.log),
  [Runtime-Fmt](logs/phase103-fmt-runtime.log).
- Andere GPUs, Gesamt-GPU-Framezeit, VRAM/Buffer-Ownership und lange
  zeitabhaengige Stabilitaet: hier nicht vollstaendig vermessen.
- Das Inventar verursacht weiterhin messbare zusaetzliche CPU-/Layout-Arbeit;
  68.38 FPS bei offenem 4K-Inventar sind nicht die 98.41 FPS der geschlossenen
  Ansicht. Die verbleibende Differenz wird nicht als beseitigt ausgegeben.
- Der Runtime-Patch besitzt weiterhin die bestehende globale Binding-
  Revision, keinen neuen per-Root-Abhaengigkeitsgraphen. Bewusst als ganzer
  Ausdruck zurueckgegebene Container werden kopiert; auch eine geklammerte
  Container-Zwischenexpression kann noch eine Kopie benoetigen. Es wird
  keine generell kopierfreie Expression-Engine behauptet.
