# Kamera, Cursor, Drops Und Detailgras - 16.10.2026

Phase 28 erweitert die bestehenden Systeme, ohne neue Dependencies, neue
UI-Frameworks oder parallele Speicherpfade. Weltformat 1, Generatorversion 5,
Seeds und Block-/Welt-IDs bleiben erhalten. Netzwerkprotokoll **13** benoetigt
zusammen aktualisierte Clients und Server. Alte Inventare und Drops bekommen
fehlende neue Felder durch Serde-Defaults.

## Darstellung Und Eingaben

Der lokale Avatar las bisher direkt den Physiktransform, die Kamera dagegen
bereits eine geglaettete Darstellung. Fixed-Ticks und Serverkorrekturen wurden
dadurch als gegeneinander versetzte Bewegung sichtbar. Jetzt verwenden beide
`CharacterPresentation::visual_position` mit derselben Interpolationsphase.
`reconcile` bewahrt bei kleinen Korrekturen die Renderposition fuer jede Phase,
waehrend Teleports direkt einrasten. Die Serverautoritaet bleibt bestehen;
dies ist keine Behauptung korrekturfreier Bewegung bei beliebiger Latenz.

F5 schaltet erste Person, Rueckansicht, Vorderansicht und wieder erste Person.
`CameraLook` trennt Spielerblick und angezeigte Kamerarotation. Bewegung,
Schwimmrichtung, Zielstrahl und gesendeter Blick werden in der Vorderansicht
nicht umgekehrt. Der kugelgepruefte Kameraboom arbeitet auf beiden Seiten.

Die Augen sind kleiner und besitzen ein gemeinsames Lidmesh. Ein etwa
0.16 Sekunden langer Blinkimpuls alle drei bis sieben Sekunden, gelegentlich
doppelt, bewegt die Lider, nicht den gesamten Kopf. Lokaler und entfernter
Avatar behalten sechs Koerperteilmeshes. Fuer das Inventar entstehen einmalig
zwei GPU-gelesene Portraits, offen und geschlossen; die Vorschaukameras werden
anschliessend entfernt. Blinzeln schaltet nur das Image um.

Die neuen frei belegbaren Controls sind Middle Pick, Drop und Drop Stack:
mittlere Maus kopiert nur in Creative den wirklich anvisierten Block. Ein
passender Hotbarstapel wird bevorzugt; sonst fordert der Client serverseitig
128 Einheiten im aktuellen Slot an. Q wirft eine Einheit, Shift+Q den gesamten
Stack. Linke und rechte Modifikatortasten sind fuer Chords gleichwertig;
Shift+Q loest nicht gleichzeitig das einfache Q aus. Vorhandene individuelle
Belegungen werden beim Ergaenzen neuer Defaults nicht ueberschrieben.

## Inventar Und Autoritaet

Alle Elemente bleiben TiltUI-Komponenten. Feste quadratische Slots mit leichter
Rundung verwenden je Layoutstufe 54/48/42 Pixel. Survival zeigt 18 Rucksack-
und neun verschiedene Hotbarplaetze, keine doppelten Speicherzellen.

Der Mausstapel ist `Inventory.cursor` im UUID-gebundenen Serverzustand und im
bestehenden `EditJournal::gameplay` gespeichert. Auch ein voller Rucksack beim
Schliessen des Inventars verliert den Cursor nicht. Alle Manipulationen laufen
ueber denselben zuverlaessigen Kontrollkanal und Inventarrevisionen.

- LMB: aufnehmen, ablegen, vereinigen, tauschen; mit gehaltenem Stapel ziehen
  verteilt gleichmaessig auf besuchte passende Slots.
- RMB: halben Stapel aufnehmen bzw. eine Einheit ablegen; bei erneutem Betreten
  nach einem anderen Slot darf wieder eine Einheit abgelegt werden.
- Mit leerem Cursor begonnener LMB-Zug sammelt passende besuchte Stapel;
  Shift-LMB-Ziehen verschiebt besuchte Stapel zwischen Rucksack und Hotbar.
- Doppelklick sammelt passende Items, das Mausrad transferiert Einheiten.
- Creative-Katalog: LMB 128/RMB eine Einheit auf den Cursor; Shift-Klick
  befuellt den echten Bestand ohne einen vorhandenen Stapel zu ersetzen.
- Ausserhalb des Inventarpanels wirft LMB den ganzen Cursorstapel ab,
  RMB nur eine Einheit. UI-Klicks bauen nicht zugleich Weltbloecke ab.

Die Gesten orientieren sich am primaeren
[MouseTweaks-Code](https://github.com/YaLTeR/MouseTweaks/blob/master/src/main/java/yalter/mousetweaks/Main.java),
sind aber kein Versprechen vollstaendiger Modkompatibilitaet mit jedem Container.
Die Client-FIFO ist auf 64 Aktionen begrenzt, sendet genau einen Befehl bis zur
Antwort und wiederholt nur ausdrueckliche Revisionskonflikte, hoechstens zweimal.
`CursorGuard` bewahrt die urspruengliche Aktion auch ueber Wartezeiten: Leeren
oder Typwechsel des Cursors darf einen Ablagezug nicht in Aufnehmen oder einen
Mausrad-Cursortransfer in einen anderen Transfer umdeuten. Der Server validiert
zusaetzlich Spielmodus, Slotindizes, Mengen und Dropbudget atomar.

Beschreibungen stehen in `description` jedes vorhandenen Block-JSONs;
Prop-Itemtexte in `assets/terra/props/items.json`. Die bestehende BlockPalette
traegt sie bis zum Tooltip, ohne zweiten Itemkatalog. Name, `teramic::`-ID und
Beschreibung bleiben getrennt. Das Standardstapellimit bleibt 128.

## Physikalische 3D-Drops

Der vorhandene `WorldDrop` erhaelt Quaternion, Winkelgeschwindigkeit,
Bodenkontakt und eine kurze Einsammelsperre fuer geworfene Items. Server-Fixed-
Ticks simulieren Wurf, Schwerkraft, Rotation, kleines Abprallen, Reibung und
Ausruhen auf einer Flaeche. Entfernt man die Unterstuetzung, faellt der Drop
weiter. Rotationsabhaengige AABB-Terrainkollisionen begrenzen den Aufwand;
Drop-gegen-Drop-Kollisionen und beliebige Rigidbody-Stapel sind nicht enthalten.

Der Renderer verwendet echte BlockTemplate-/Propmodelle mit geteilten
Mesh-/Material-Handles statt 2D-Sprites oder generischen Cube-Markern.
Nur fliegende Snapshots werden interpoliert; ruhende Drops liegen exakt an
ihrer bestaetigten Pose, ohne Hover-/Dauerrotation. Laubdrops verwenden ein
statisches Maskenmaterial, damit Wind den Ruhekontakt nicht verschiebt.
2048 weltweite, 256 nahe replizierte Drops und 300 Sekunden Lebensdauer bleiben
erhalten. Volle Inventare lassen den Drop liegen; volle Dropbudgets vernichten
keine Ressourcen. Keine Avian-Entity pro Drop und keine GPU-Abhaengigkeit im
Dedicated Server.

## Abbau Und Gras

Normale Cube-Ziele erhalten eine begrenzte asynchrone Renderersetzung genau
einer betroffenen Section. Der Zielblock wackelt, acht Teilstuecke verschwinden
fortschreitend und werden von kurzlebigen Splittern begleitet. Ein geschlossener
Kern, statischer Anschlussrand und zuvor verdeckte feste Nachbarflaechen
verhindern Durchblicke, auch an Sectiongrenzen. Bei Abbruch wird das Original
sofort wiederhergestellt. Bei bestaetigtem Abbau verschwindet nur die Zielgruppe;
die Ersatzsection bleibt bis zum fertigen regulaeren Upload, ohne alten Block
erneut kurz einzublenden. Epochen und Abbruchpruefung verwerfen alte Workerjobs.
Maximal ein Worker und eine Ersatzsection, keine Welt-Rebuilds im Hauptthread.
Sections mit importierten Custom-Mesh-Szenen behalten als konkrete
Einschraenkung das Riss-Overlay, auch beim normalen Cubeziel in derselben Section.
Creative bleibt sofortiger Abbau ohne Haltezeit.

Gras hat drei Silhouetten aus breiten, stumpfen, geknickten Segmenthalmen und
bodennahen Blattpixeln. Einzelne Varianten tragen kleine weisse Blueten.
Die Referenz wird als Pflanzensilhouette aufgegriffen, nicht als schwebender
Dirtblock unter jedem Bueschel. Near: 312/348/384 Vertices, mit Blueten maximal
476; Middle/Far unveraendert 160/80. Bestehende Section-Batches, stabile Prop-IDs,
Picking, Biom-/Zustandsfarben, Wind und 1.3-fache Darstellung bleiben erhalten.

## Pruefnachweise

Alle fuenf verlangten Workspace-Pruefungen bestanden am eingefrorenen Stand
mit `CARGO_BUILD_JOBS=2`:

| Pruefung | Ergebnis |
| --- | --- |
| `cargo fmt --all --check` | Exit 0, [Log](logs/terra-interaction-v2-fmt.log) |
| `cargo check --workspace` | Exit 0, [Log](logs/terra-interaction-v2-check.log) |
| `cargo test --workspace` | 516 bestanden, ein vorhandener Langzeitsoak ignoriert, [Log](logs/terra-interaction-v2-tests.log) |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0, [Log](logs/terra-interaction-v2-clippy.log) |
| `cargo build --workspace` | Exit 0, [Log](logs/terra-interaction-v2-build.log) |

Abgedeckt sind unter anderem Cursor-/Slotkonservation, Split/Drag/Sammeln,
Creative-Berechtigungen, atomare Fehler bei vollem Dropbudget, zwei echte
UDP-Sitzungen mit Inventaraktionen und Snapshotweitergabe, Legacy-Speicherung,
Modifikatorbelegung, Frontkamera, korrigierte Darstellung sowie Meshgueltigkeit,
Miningabbruch, Anschlussflaechen und Epochen-Handoff. Die 69 UI-Tests umfassen
ausdruecklich einen Cursor mit nur einer Einheit und mehrere volle Ziele;
nachfolgende wartende Aktionen duerfen keine fremden Items aufnehmen.

### Native Eingabe, Kamera Und Drops

Die nachstehenden Funktionslaeufe verwenden Wayland/Vulkan auf RX 6950 XT.
Screenshots sind nach `ScreenshotCaptured` bzw. echtem GPU-Readback geprueft,
nicht nur angeforderte Dateinamen. Aus gleichzeitig mit Cargo ausgefuehrten
Funktionstests werden keine Performancewerte abgeleitet.

`--smoke-test-camera` bestand mit echter Integrated-Server-Verbindung.
Bevy-Eingaben durchlaufen produktive Controls, Zielauswahl und Inventar-RPC;
Middle Pick lieferte 128 Grass. F5 hinten/vorne/erste Person und Bewegung
bestanden mit 726 verglichenen Darstellungssamples, maximalem Positionsfehler
0.0 und vier fertigen Captures. Dies prueft die gemeinsame Darstellungsposition,
nicht beliebige Internetlatenz oder eine menschliche Bewegungsabnahme.
Das Testprofil verwendet FOV 45, weshalb die Figur auf den Bildern gross ist.
[Kameralog](logs/terra-interaction-v2-camera.log),
[Rueckansicht](captures/camera-rear.png),
[Vorderansicht](captures/camera-front.png),
[Bewegung](captures/camera-walking.png).

`--smoke-test-drops` bestand ebenfalls am echten Integrated Server mit
produktiven Bevy-Tastaturereignissen: Q aenderte Bestand 4 -> 3, Shift+Q 3 -> 0.
Die replizierten Drops enthielten genau eine und drei Einheiten, ohne Verlust.
Der Test pruefte echte nichtleere 3D-Meshes, geaenderte Rotation waehrend des
Flugs und eine flache, mindestens 800 ms stabile Ruhelage. Der Mittelpunkt lag
0.12228/0.12225 m ueber dem Terrain bei 0.12 m halber Modellgroesse.
[Droplog](logs/terra-interaction-v2-drops.log),
[Flug](captures/drop-air.png), [Ruhe](captures/drop-rest.png),
[ganzer Stack](captures/drop-stack-rest.png).

Der Creative-Inventarsmoke verwendete echte OS-Mausereignisse ueber Wayland/
`uinput`, nicht direkte Aenderungen des erwarteten Inventarzustands. Katalog128,
Aufnehmen, Halbieren, einzelne Ablage, gleichmaessiges Verteilen, Ziehsammeln,
Suche, Tooltip und beide Layoutgroessen bestanden. Ein echter Klick auf den
Backdrop warf den gehaltenen 63er-Stapel ab: Cursor leer, 65 im Inventar plus
63 im replizierten WorldDrop = 128. Der nahe Drop wurde nach der regulaeren
Sperrzeit spaeter wieder eingesammelt; die kompakte Aufnahme zeigt deshalb
wieder Bestand, keinen verschwundenen Drop.
[OS-Eingabelog](logs/terra-inventory-v2-outside.log),
[breites Inventar](captures/inventory-creative-wide.png),
[kompaktes Inventar](captures/inventory-creative-compact.png).
Der gesonderte Survival-Inventarlauf ist eine echte Mode-/Layoutabnahme;
Survival-Ressourcenverbrauch wird im separaten Bau-Smoke geprueft.
[Survival-Layoutlog](logs/terra-inventory-v2-survival.log),
[breit](captures/inventory-survival-wide.png),
[kompakt](captures/inventory-survival-compact.png).

Ein frueher Creative-Bau-Smoke erreichte den 0.7-s-Inventarcheck nicht; dieser
urspruengliche Fehler ist nicht reproduziert und wird nicht als belegter
Produktions-RPC-Bug ausgegeben. Unitregression, Middle Pick und der explizite
Legacy-Nativelauf bestaetigten den alten CreativeSet-Pfad erfolgreich:
Grass128 wurde erhalten, nach Platzierung blieben128 und Creative erzeugte
keine Drops. `TERRA_BUILD_LEGACY_SELECT=1` bleibt als Fixturediagnose vorhanden.
[Legacy-Baulog](logs/terra-interaction-v2-building-legacy.log).

Anfaengliche Kamera-/Botanikfehler waren Fixtureannahmen: Das neue Testflag
benoetigt die Menuinstallation, und ein fester Bodenstrahl traf korrekt ein
Grass-Prop. Die Fixture sucht jetzt mit dem produktiven Picker eine tatsaechlich
freie Blickrichtung, statt echte Props bei der Zielauswahl zu ignorieren.
Botanik aktiviert fuer seine Zielnamensassertion explizit den sonst abgeschalteten
Debugmodus. Diese Fehler werden nicht als behobene Produktregressionen verkauft.

### Native Renderabnahme

Botanical, Avatar sowie Creative- und Survival-Bau bestanden am finalen Stand,
insgesamt 31 abgeschlossene PNGs. Alle vier Logs sind ohne Warnungen,
Shader-/Validationfehler oder Panics. Renderer-Agent betrachtete Gras, Busch,
beide Portraits und mehrere Abbaustufen. Root betrachtete zusaetzlich Gras,
die Vorderkamera, einen ruhenden 3D-Drop und die spaete Abbaustufe.

Botanical hat sieben Aufnahmen einschliesslich identischer Kameras zu zwei
Windzeitpunkten. Die zentralen Gras-/Buschausschnitte enthalten unterschiedliche
Pixel, ohne Debug-Overlay als Ursache. Avatar hat fuenf Aufnahmen, zwei
unterschiedliche GPU-Portraitreadbacks, eine beobachtete Welt-Blinkanimation und
vier animierte Gliedmassen. Beide Portraitkameras werden danach entfernt.
[Botaniklog](logs/terra-interaction-v2-botanical-final.log),
[Gras](captures/terra-interaction-v2-botanical-final-grass.png),
[Windpose](captures/terra-interaction-v2-botanical-final-grass-wind.png),
[Avatarlog](logs/terra-interaction-v2-avatar-final.log),
[Augen offen](captures/terra-interaction-v2-avatar-final-portrait.png),
[Augen geschlossen](captures/terra-interaction-v2-avatar-final-portrait-closed.png).

Creative-Bau nutzte den normalen Pointer -> Cursor -> Hotbar-Pfad, bestaetigte
128 Einheiten, etwa 30 ms sofortigen Abbau, keine Drops und weiterhin 128 nach
Platzierung. Survival pruefte isoliertes Ziel und Abbruch, drei zunehmende
Zerfallsaufnahmen, etwa 619 ms Abbau, Ernte 1 und Verbrauch auf 0. Nur der
Zielblock veraendert sich; Nachbarn bleiben stehen und die Innenflaechen sind
geschlossen. Die Zeitwerte sind einzelne Funktionslaeufe, keine Latenzbenchmarks.
[Creative-Baulog](logs/terra-interaction-v2-building-final-creative.log),
[Survival-Baulog](logs/terra-interaction-v2-building-final-survival.log),
[fruehe Zerlegung](captures/building-fracture-0.png),
[spaete Zerlegung](captures/building-fracture-2.png),
[nach dem Abbau](captures/building-broken.png).

### Headless Und Zwei Clients

Separater `cargo build -p terra-server` und anschliessender Start ohne DISPLAY
oder WAYLAND_DISPLAY bestanden. Der Smoke beendete nach 27 Sections mit Exit0.
`cargo tree -p terra-server -e normal` enthaelt weder bevy_render/bevy_ui noch
TiltUI, wgpu oder bevy_winit.
[Build](logs/terra-interaction-v2-headless-build.log),
[Start](logs/terra-interaction-v2-headless.log),
[Features](logs/terra-interaction-v2-headless-features.txt).

Ein echter Dedicated Server und zwei GPU-Clientprozesse mit getrennten UUIDs
bestanden den Netzwerk-Smoke jeweils mit Exit0. Der Observer bestaetigte einen
replizierten Spieler mit passendem Avatar und sechs gueltigen Koerperteilen;
der Companion bewegte sich um 6.47998 m. Beide hatten beim Capture vier offene
Inputs und laufende ACKs. Der Observer beendete frueher, deshalb enthaelt der
spaetere Companion-Capture keinen Fremdspieler mehr. Keine Behauptung zweier
gleichzeitiger Sichtaufnahmen, einer Internetabnahme oder eines Langzeitsoaks.
Der Server protokollierte das Shutdownsignal und verliess seinen ECS-Loop;
die umgebende PTY-Ausfuehrung meldete dabei Exit1. Der separate Headless-Smoke
oben beendete regulaer mit Exit0.
[Server](logs/terra-interaction-v2-network-server.log),
[Observer](logs/terra-interaction-v2-network-observer.log),
[Companion](logs/terra-interaction-v2-network-companion.log),
[Observeraufnahme](captures/terra-interaction-v2-network-observer.png).

## Isolierter Leistungsvergleich

Verglichen wird ausschliesslich die **gueltige stationaere Nachherbaseline aus
Phase 27**, nicht deren verworfener alter Binary-/UI-Mix. Gleiche Dateien unter
`configs/`, Seed 42042, Sichtweite 16/3, LOD 96/160, Water High, VSync aus,
RX 6950 XT, RADV Mesa 26.2.4 Vulkan, Wayland 150%, 1920x1200 physische Pixel, Kamera
(-622,53.88851,680). Historischer Flagname `--smoke-test-forest`, heute eine
bewachsene Pilzparklandschaft. Beide sind Offline-Renderfixtures ohne laufende
Netzwerkphysik, ohne Bewegung und ohne weitere von uns gestartete Cargo-/GPU-
Prozesse waehrend der Messung. Jeweils ein 18-s-Messfenster, kein statistisch
abgesicherter Mehrfachvergleich.

| Kennwert | Phase 27 | Phase 28 |
| --- | ---: | ---: |
| Frames / mittlere FPS | 2808 / 155.969 | 2778 / 154.289 |
| p95 / p99 CPU-Wall-Framezeit, ms | 8.087 / 9.584 | 7.846 / 9.090 |
| Erster Readyzeitpunkt inklusive Rendering, s | 3.154 | 3.014 |
| Geladene / gerenderte Sections | 7807 / 4115 | 7807 / 4115 |
| Meshassets / sichtbare Meshentities | 9635 / 4546 | 9636 / 4546 |
| Repraesentierte Props / Batches | 92789 / 1306 | 92789 / 1306 |
| Terrain-Meshing, Worker-Spannensumme, s | 0.590 | 0.630 |
| Prop-Meshing, Worker-Spannensumme, s | 0.308 | 0.311 |
| Prozess-RSS am Ende, KiB | 929876 | 934600 |
| Peak-RSS ueber getrusage, KiB | 925112 | 931384 |
| GPU-Opaque-Pass Mittel / p95 / p99, ms | 1.188 / 1.253 / 1.739 | 1.205 / 1.307 / 1.759 |

Mittlere FPS sind in diesem Paar etwa 1.1% niedriger, nicht verbessert; die
geringeren p95/p99 sind ebenfalls keine Garantie fuer Bewegung oder Streaming.
Die extra Grasdetails haben hier keine Entity-/Batchvermehrung verursacht.
Worker-Spannen sind keine serielle Ladezeit und kein neuer reiner CPU-
Generierungsbenchmark. RSS-Zaehler stammen aus verschiedenen Linux-Messwegen
und werden unverrechnet angegeben. GPU-Passzeit ist keine gesamte GPU-Framezeit,
exakte Draw Calls bleiben uninstrumentiert. Die Propzahl zaehlt repraesentierte
Objekte in Batches, nicht ausschliesslich einzeln gerade sichtbare Pflanzen.
[Vorherlog](../inventory-ranger-2026-10-16/logs/terra-inventory-forest-after.log),
[Nachherlog](logs/terra-interaction-v2-forest-after.log),
[Nachheraufnahme](captures/terra-interaction-v2-forest-after.png).

Reproduktionsaufruf nach beendetem Build und allen anderen Testprozessen:

```bash
env -u DISPLAY TERRA_CONFIG_DIR=tools/benchmarks/results/interaction-camera-drops-2026-10-16/configs \
  TERRA_BENCH_SECONDS=18 TERRA_RENDER_DIAGNOSTICS=1 \
  TERRA_SCREENSHOT=/tmp/terra-interaction-v2-forest-after.png \
  python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-forest
```

Offen bleiben der isolierte Worst Case mit 2048 bewegten Drops, mehrstuendige
Netzwerklast, Internet-/Mehrrechnertests und gemeinsame menschliche Artabnahme.
Diese Runde aendert weder logische Blockaufloesung noch prozedurales Terrain;
die CPU-Generierungsbaseline 2.6 s wird deshalb nicht als neu gemessen ausgegeben.
