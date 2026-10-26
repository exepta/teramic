# Zaehmung Effekte Und Interaktionslatenz

Historischer Stand von Phase 113: Seit Phase 114 sind automatisches Folgen und
Leerhand-Folgen/Warten vorerst deaktiviert. Vertrauen/Besitzer bleiben erhalten;
eine spaetere Befehlsoberflaeche ist F33. Die folgenden Folge-Aufnahmen und
Messungen dokumentieren den damaligen Stand, nicht eine aktuell bedienbare Funktion.

Phase 113 erweitert die vorhandenen Tier-, Status-, Inventar- und Streaming-Systeme.
Welt-/Block-/Tier-IDs und Generatorversion 5 bleiben erhalten. Client und Server
verwenden gemeinsam Protokoll 65; Spielerdateien bleiben PLY7. Neue Regionen nutzen
RGA3 als Erweiterung des bisherigen RGA2-Inhalts. Alte Regionen werden weiter gelesen.

## Zaehmung

Mit der konfigurierten Platzieren-Taste wird das anvisierte Tier mit dem aktiv gehaltenen
Futter gefuettert. Vier erfolgreiche Fuetterungen zaehmen Schaf/Kuh/Schwein,
fuenf Pferd/Spinne. Bei ununterbrochener Fuetterung durch denselben Spieler liegen
dazwischen mindestens 1,25 Sekunden simulierte Tierzeit. Schlaege oder wechselnde
Fuetterer unterbrechen diese Folge; der allgemeine 450-ms-Interaktionsschutz bleibt.
Nahrung wird auch in Creative aus dem ausgewaehlten Slot verbraucht.
Erfolgreiches Fuettern beruhigt noch nicht vollstaendig gezaehmte Tiere fuer acht
simulierte Sekunden. Ein erfolgreicher Angriff auf das Tier beendet diese Beruhigung.

| Tier | Futter | Vertrauen je Fuetterung |
|---|---|---:|
| Schaf, Kuh | Weizen | 25 |
| Pferd | Weizen, Mais, Salat | 20 |
| Schwein | Mais, Salat, Pilze | 25 |
| Alle drei Spinnenarten | Rohes Rind-/Schweine-/Schaffleisch oder roher Fisch/Clownfisch | 20 |

Ab Vertrauen 100 verwendet das Tier die vorhandene persistente Besitzer-UUID und
folgt seinem Besitzer. Benutzen mit leerer Hand schaltet fuer den Besitzer zwischen
Folgen und Warten um. Nichtbesitzer koennen das Tier nicht uebernehmen oder umschalten.
Vertrauen, Besitzer und Modus stehen im Ziel-HUD. Die UUID-Kurzkennung fremder Besitzer
ist kein erfundener Spielername; die vorhandene Namensliste stellt keine UUID-Zuordnung bereit.

Folgen verwendet die bestehenden geladenen Kollisions-/Navigationsdaten und
Suchbudgets, ohne Teleportieren durch Hindernisse. Abwesende oder zu weit entfernte
Besitzer lassen das Tier warten. Tiere laden keine eigene unbegrenzte Chunk-Umgebung.
Gezaehmte Tiere bleiben aus dem natuerlichen Spawnlimit ausgeschlossen. Bereits vor
dieser Erweiterung gespeicherte Besitzerzuordnungen bleiben gueltig und starten wartend.

## Spinnen

Hoehlenspinnen priorisieren ein gueltiges verankertes Netz und speichern danach
eine eigene Hoehlenzuordnung mit der echten Netz-ID. Vorhandene geeignete Netze
koennen wiederverwendet werden. Dies verhindert unbegrenztes wiederholtes Spinnen
derselben Umgebung. Entfernte Netze erlauben spaeter einen Neubau.
Die Ersatz-ID wird aus der gespeicherten vorigen Netz-ID deterministisch
abgeleitet, ohne die gesamte Entfernungshistorie zu durchsuchen. Unpassende
Anheftstellen fuehren zu begrenzter Standortsuche statt endlosem Stillstand.
Fuettern und Besitzerbefehle erhalten echte Wand-/Decken- und Seilbefestigungen.

Spinnen besitzen Nahrung 50/50 und keinen Durst. Ab hoechstens 40/50 Nahrung suchen
sie nach dem Netzbau innerhalb von 20 Metern um sich und ihr Netz nach geeigneter Nahrung.
Kleine Spinnen jagen Insekten; groessere auch geeignete ungezahmte Tiere. Passende
echte Fleisch-/Fischdrops werden verzehrt. Angriffe, Reichweite, Sicht und existierende
Nahrung werden beim Verzehr erneut geprueft; keine Nahrung aus dem Nichts.
Der gemeinsame Suchindex wird nur bei aktiven hungrigen wilden Spinnen mit Netz
aufgebaut, aus aktiven Tieren und geladenen Drops, nicht allen historischen Regionen.

Beim Lesen alter Regionen mit bisherigen Spinnen ohne Nahrung wird genau einmal
auf 50 Nahrung migriert. Bereits im neuen Format gespeicherte hungrige Spinnen
werden beim Neuladen nicht wieder aufgefuellt. Auch gezaehmte Spinnen erleiden
keinen Durstschaden.

## Effekte

```text
/effect teramic::nightvision 2 100
/effect @Name teramic::nightvision 2 200
/effect @"Name Mit Leerzeichen" teramic::regeneration 1 5
/effect clear
```

Die neue Direktform erwartet Effekt, Stufe, Sekunden. Ohne Ziel gilt der lokale
Spieler; andere Spieler benoetigen die bestehende Environment-/Adminberechtigung.
Die Serverkonsole verwendet dieselbe Grammatik ohne Slash und mit Zielspieler.
Alte `effect give`-/`effect clear`-Formen, Hilfe und Vervollstaendigung bleiben erhalten.
Stufen werden gegen die jeweilige Definition, Laufzeiten auf endliche Werte von
0,1 bis maximal 3600 Sekunden geprueft.

Night Vision ist der angehaengte 14. Effekt; die bisherigen Enum-Indizes bleiben
unveraendert. Er arbeitet mit lokalem Kamera-Ambientlicht und HUD-Augenicon,
auch in Creative/Spectator. Sonnenlicht und fremde Kameras werden nicht veraendert;
Ablauf/Entfernen setzt den Zuschlag auch bei pausierter Weltzeit zurueck.

## Interaktion Und Geraeusche

- Creative-Pick-Block erzeugt ein Exemplar statt 128. Ein vorhandener passender
  Hotbarstapel wird wie bisher ausgewaehlt, nicht auf ein Exemplar reduziert.
- Place auf ein naeheres Tier oeffnet keine dahinterliegende Station und treibt
  keine dahinterliegende Handkurbel an. Explizites Use behaelt die bestehende
  Stationsbedienung. Gehaltenes Place startet dabei auch kein Essen.
- Truhensounds reagieren auf bestaetigte Betrachter-Zustandswechsel, auch fuer
  nahe Beobachter. Doppeltruhen haben einen gemeinsamen Anker und spielen nur
  einen Cue. Laden/Entladen oder Ankommen bei einer offenen Truhe erzeugt keinen
  falschen Oeffnungs-/Schliesssound. Maximal 256 nahe Truhen werden verfolgt.
- Hinge-/Latch-Foley verwendet bestehende CC0-Aufnahmen im begrenzten gemeinsamen
  24-Stimmen-Mixer, ohne neue Audio-Entity je Truhe. Herkunft und Ableitung stehen
  in [CREDITS](../../../../assets/audio/CREDITS.md).

## Korrigierte Engpaesse

`world::streaming::dispatch_jobs` liess Aenderungen geladener Sections hinter
saemtlichen normalen Generierungsjobs warten und verarbeitete bis zu 16 Sections
seriell in einer Aufgabe. Geladene bearbeitete Sections verwenden nun eine eigene,
bevorzugte Einzel-Remesh-Aufgabe mit maximal einem zusaetzlichen Jobslot. Sie
verwendet vorhandene Blockdaten, die vorhandene vorbereitete Spalte und einen
geladenen Nachbar-Halo statt vollstaendiger Weltgenerierung. Veraltete Aufgaben
werden invalidiert. Neuere Wasser-/Prop-Zustaende bleiben bei der Publikation erhalten.
Natuerliche Wasserflaechen werden aus derselben edit-bewussten Halo-Geometrie
erneuert; neuere Live-Fluessigkeitsstaende und explizite Overrides haben Vorrang.
Normale Generierungsjobs geben zwischen Sections kooperativ ihren Worker frei,
damit der bevorzugte Editjob nicht hinter vier ganzen Spaltenberechnungen festhaengt.
Eine einzelne synchrone Spaltenvorbereitung kann weiterhin kurz Rechenzeit benoetigen.

Der Renderer priorisiert bearbeitete Sections bei Meshing und Upload, verwirft
veraltete Ergebnisse und erkennt Edits gezielt statt auf einen kompletten
zeitbudgetierten Welt-Sweep zu warten. Bestehende Uploadbudgets und Grafikqualitaet
bleiben erhalten.

TiltUI finalisierte beim Aufbau verschachtelter Slot-Branches wiederholt globale
Tooltip-/ID-Ziele. Diese Arbeit wird nun bis zum fertigen Flow-Batch aufgeschoben
und einmal je betroffener Komponente ausgefuehrt. Inventarkatalog-Metadaten werden
wiederverwendet; nur die sichtbare Browserseite wird aufgebaut. Die gemessene
Browserkapazitaet bleibt beim erneuten Oeffnen erhalten; beim ersten Oeffnen
wird sie weiterhin aus dem tatsaechlichen Layout ermittelt.

Eine einzelne Inventaroberflaeche wird waehrend des Ladens unsichtbar vorbereitet
und pro aktiver Welt wiederverwendet. Dabei werden echte Slotabmessungen,
Browserkapazitaet und Kategorie-Dropdown abgewartet, auch bei niedriger UI-FPS.
Geschlossen ist sie `display: none`; ihre Flow-/Property-Bindings werden geparkt
und Textauswertung ausgesetzt. Beim Oeffnen werden dieselben Entities wieder
aktiviert und auf den aktuellen Zustand gebracht. Portallierte Tooltips werden
mitgeparkt, beim Weltwechsel wird der Cache verworfen. Es gibt keinen wachsenden
Cache fuer jede besuchte Truhe und keinen zusaetzlichen Runtime-Hintergrundjob.

Neue CSS-Selektoransichten vieler verschachtelter Slot-Komponenten erfassen die
Element-/ControlPart-Daten nun gemeinsam je Style-Pass. Bereits warme partielle
Aktualisierungen bleiben erhalten; neu erzeugte Scrollbar-Teile erhalten weiterhin
eine aktuelle Ansicht. Regressionstests vergleichen gruppierte und isolierte
Selektoren einschliesslich verschachtelter Scopes, Pseudoteile und Mutationen.
Der direkte Workspace-Verweis auf `tilt-ui-runtime` stellt nur die Park-API des
bereits gepatchten Pakets bereit; es wurde keine Dependency-Version aktualisiert.

Diese konkreten Ursachen und ihre Regressionstests sind kein pauschaler Nachweis
fuer beliebige FPS-/Hardware- oder mehrminuetige Streaminglast. Lange 10-/15-/30-Minuten-
Laeufe sind entsprechend der Nutzeranweisung nicht Bestandteil dieser Phase.

## Offene Erweiterungen

Neue TODO-Eintraege F30/F31/F32 beschreiben Traenke mit Mischwirkungen,
Flaschen/Kessel/Loeffel, Lagerfeuer und Kaelte/Hitze. Diese Systeme sind hier
ausdruecklich nicht implementiert. Reiten und Zucht bleiben F07 und F06.
Menschliche Hoerabnahme und Tests auf weiteren Rechnern bleiben offen.

## Funktionspruefungen

Auf der echten RX 6950 XT (RADV/Mesa 26.2.4, Vulkan), ohne parallelen Cargo-Build:

- `--smoke-test-taming`: echter integrierter UDP-Server, vier verbrauchte Weizen,
  Vertrauen 25/50/75/100, richtige Besitzer-UUID, leere Hand, Warten und echte
  Folgebewegung von 0,963 m. Fuenf Aufnahmen unter [native/taming](native/taming).
  Die Fixture wartet auf tatsaechlich gebundene HUD-Texte und verifiziert lokale
  sowie zum Server uebertragene Blickrichtung. Balken animieren weiterhin weich.
- `--smoke-test-effects`: zehn Aufnahmen, alle 14 registrierten Effekte,
  begrenzte Karten/Overflow, drei Fenstergroessen und echte Direktbefehle fuer
  Nachtsicht um Mitternacht einschliesslich Entfernen. Kamera-Ambientlicht und
  Augenicon werden geprueft. [Nachtsicht](native/effects/status-nightvision-1280.png)
  und [danach](native/effects/status-nightvision-clear-1280.png) wurden visuell verglichen.
- `--smoke-test-building` mit Truhenfixture: reale Silhouette, bestaetigtes
  Oeffnen/Schliessen, genau ein Cue je Wechsel und vier Modell-Abbaustufen.
  [Offener Deckel](native/chest/chest-lid-open.png) wurde visuell geprueft.
- Normale Creative-Baufixture: echtes Abbauen, Inventar, kleine/grosse Ansicht,
  Vorschau, Platzieren und sichtbares aktuelles Mesh. Zwei kurze Einzel-Edit-Proben:
  37,491 beziehungsweise 44,196 ms vom Request zur Mesh-Publikation;
  13,142 beziehungsweise 15,875 ms von bestaetigter Blockeigenschaft zum
  aktuellen sichtbaren Mesh. Dies ist kein GPU-First-Draw-
  Zeitstempel und keine allgemeine Streaming-p99. Aufnahmen unter
  [native/placement](native/placement).
- `--smoke-test-inventory` mit `TERRA_NATIVE_OS_POINTER=1`: echte Wayland-/uinput-
  Mausgesten, elf Aufnahmen, Kategorie-Dropdown mit paginierter Werkzeugliste,
  Einzelitem-Grant, Pickup/Split/Drag/Sweep, Count-Erhaltung, Tooltips, Scrollen,
  Muelltonne, erneutes Oeffnen derselben Shell und weiter sichtbarer Welt-HUD.
  Die Fixture prueft alle zehn kanonischen Ausruestungspositionen und entfernt
  ihre eigene kuenstliche Scrollhoehenbegrenzung vor dem Reopen. Breite Ansicht
  unter [native/inventory](native/inventory) wurde visuell kontrolliert.
- `terra-server --smoke-test --no-save`: 27 Sections, erfolgreich ohne Fenster.

Die Funktionsfixtures verwenden das isolierte vorhandene Phase-109-Profil,
nicht private Welten. Die Effektfixture setzt mehrere logische Fenstergroessen;
die realen Pixelabmessungen stehen in ihrem Log. Inventar-Performance wird
getrennt mit explizit geprueften physischen Aufloesungen und groesseren
Sichtweiten gemessen.

## Inventar-Messvergleich

Gemessen mit dem optimierten Dev-Build (`opt-level = 1`, Dependencies 3), echter
RX 6950 XT/Vulkan und integriertem UDP-Server. Keine parallelen Cargo-Prozesse.
Vorher/Nachher bezeichnet hier ausschliesslich die letzte Inventar-Vorwaermungs-,
Park- und CSS-Batch-Korrektur, nicht einen erfundenen Ausgangswert fuer alle
anderen Aenderungen dieser Phase.

Gleicher Seed 42042, physisch 2880x1620, Sichtweite 12, hohe Schatten/Wasser,
Ultra-Wetterqualitaet bei klarem Wetter, UI-FPS 60 und identische festgehaltene
Kamera `(8.5, 114.63007, 24.5)`. Das vorhandene Benchmark-System prueft die
physische Aufloesung; jede Phase misst 12 Sekunden nach abgeschlossener
Szenenladung. Zwischen UI-Umschaltungen liegen zwei Sekunden Beruhigungszeit.
Oeffnungs-/Schliessframes werden separat gemessen, nicht in die steady-state-FPS
eingerechnet. An dieser Position waren keine Wasser-Tiles sichtbar.

| Phase | FPS vorher | FPS nachher | p95 ms vorher / nachher | p99 ms vorher / nachher | RSS MiB vorher / nachher |
|---|---:|---:|---:|---:|---:|
| Geschlossen | 112,10 | 112,56 | 11,32 / 11,23 | 12,94 / 12,58 | 1395,4 / 1471,1 |
| Offen | 81,42 | 82,46 | 13,90 / 14,45 | 17,28 / 16,34 | 1550,0 / 1512,6 |
| Wieder geschlossen | 114,30 | 114,99 | 11,75 / 11,23 | 13,30 / 12,71 | 1558,4 / 1519,4 |

| Echter UI-Uebergang | Vorher ms | Nachher ms |
|---|---:|---:|
| Erste Oeffnung bis sichtbares, gesetztes Slotlayout | 324,65 | 65,34 |
| Laengster Oeffnungsframe | 166,07 | 24,92 |
| Schliessen | 32,18 | 22,69 |

Die erste Oeffnung wird damit rund 80 Prozent schneller; der laengste zugehoerige
Frame wird rund 85 Prozent kuerzer. Die dauerhaft offene UI wird dadurch nicht
pauschal schneller: ihre FPS liegen innerhalb der normalen Schwankung beider
Laeufe. Der begrenzte vorgewarmte Baum kostet vor der ersten Oeffnung rund 76 MiB
zusaetzlichen RSS in diesem Lauf. Geschlossen bleiben nur 31 Property-Binding-
Nodes aktiv, wie zuvor; der Rest ist geparkt. Assets/Entities werden nicht je
Oeffnung erneut dauerhaft gecacht. Dies ersetzt keine mehrminuetige Leak-Messung.

Rohdaten: [vorher](logs/benchmark-inventory-clear12.log) und
[nachher](logs/benchmark-inventory-clear12-after.log).

### Zweiter Seed Mit Regen Und Wasser

Zusaetzlicher kurzer Nachtest, **ohne** dafuer einen Vorher-Wert zu behaupten:
Seed 2262437629133911810, physisch 3840x2160, Sichtweite 20, UI-FPS 90,
hohe Schatten/Wasser, Ultra-Wetterqualitaet und autoritatives `/weather set rain`.
987 Wasser-Tiles mit 463876 Dreiecken waren beim Szenenstart geladen.

| Phase | FPS | Median ms | p95 ms | p99 ms | RSS MiB |
|---|---:|---:|---:|---:|---:|
| Geschlossen | 59,42 | 16,44 | 25,10 | 30,09 | 2044,2 |
| Offen | 58,62 | 16,12 | 22,97 | 26,56 | 2053,0 |
| Wieder geschlossen | 59,98 | 16,41 | 25,17 | 29,48 | 2065,8 |

Hier dauerte die erste Oeffnung 163,91 ms ueber vier Frames; ihr laengster Frame
war 81,41 ms. Bei der hoeheren Last bleibt also eine messbare Oeffnungsspitze;
vollstaendig hitchfreies Verhalten in jeder Aufloesung wird nicht behauptet.
Die permanente UI-Last und die Grafikqualitaet wurden nicht abgesenkt.
Ein Winit-Hinweis beim Ermitteln des aktuellen Monitors erschien vor dem Laden;
die physische Aufloesungspruefung bestand und der komplette Lauf war erfolgreich.
Keine Slab-Allocator-/Use-after-free- oder Shaderfehler in diesen beiden Laeufen.
[Regen-Log](logs/benchmark-inventory-rain20.log).

Die Messwerte sind CPU-Wall-Framezeiten mit realem Rendering. Die ebenfalls
geloggten einzelnen GPU-Passzeiten sind keine additive GPU-Gesamtframezeit.

### Benchmark Wiederholen

Die vorhandenen isolierten Profile verwenden; fuer einen eigenen Vergleich
entsprechend getrennte Konfigurationsverzeichnisse mit identischen Einstellungen
anlegen. Keine privaten Welten fuer die automatisierte Fixture verwenden.
Die fuenf TOML-Dateien je verwendetem Profil sind unveraendert gegenueber den
bereits archivierten [Clear-Einstellungen](../inventory-fps-2026-10-22/configs/clear)
und [4K-Einstellungen](../inventory-fps-2026-10-22/configs/user).

```bash
TERRA_CONFIG_DIR="$PWD/target/phase103/clear" \
TERRA_MENU_CONFIG_DIR="$PWD/target/phase103/clear" \
TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 TERRA_BENCH_GRAPHICS=1 \
TERRA_BENCH_INVENTORY=1 TERRA_BENCH_SECONDS=90 \
TERRA_RENDER_DIAGNOSTICS=1 TERRA_CPU_SCOPES=1 \
target/debug/terra-client --smoke-test-benchmark
```

Der Inventartest beendet selbst nach seinen drei kurzen Phasen. Die 90 Sekunden
sind die Laufzeiteinstellung des normalen Benchmark-Szenarios, das Inventarmodul
beendet den Lauf vorher; dies ist kein angeforderter Langzeittest.
Fuer den Regenlauf beide Verzeichnisse auf `target/phase103/user` aendern und
`TERRA_BENCH_WEATHER=rain` hinzufuegen.

## Abschliessende Automatisierte Pruefung

Auf dem abschliessenden Quellstand erfolgreich:

```text
cargo fmt --all --check
cargo check --locked --workspace
cargo test --locked --workspace --no-fail-fast
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace
```

2.054 Workspace-Tests bestanden, drei vorhandene Tests bleiben ignoriert.
Zusaetzlich bestanden alle 319 TiltUI-Runtime-Tests. Cargo-Builds liefen einzeln
mit `CARGO_BUILD_JOBS=1`. Ein veraltetes Runtime-Artefakt wurde gezielt mit
`cargo clean -p tilt-ui-runtime` entfernt (7,5 GiB), ohne Quellen oder Welten
zu loeschen; es gibt kein neues Paket oder Dependency-Upgrade.

Die neuen Regressionen decken unter anderem Arten/Futter, Verbrauch und
Fremdbesitzer, Folgen/Warten und sichere Bewegung, RGA2-Migration/RGA3-Roundtrip,
Netzverlust/-neubau, verankerte Spinnen, 20-m-Jagd, Durstfreiheit,
Effektberechtigungen/Parser, Nachtsicht-Ruecksetzung, Creative-Pick,
Truhen-Zustandswechsel/Doppeltruhen, bearbeitete Remesh-Jobs auf einem Worker,
Wasser-/Prop-Zustandserhalt, begrenzten UI-Cache und kalte CSS-Scope-Batches ab.

Rohlogs: [Tests](logs/test-final.log), [TiltUI](logs/test-tilt-ui-all.log),
[Check](logs/check-final.log), [Clippy](logs/clippy-final.log),
[Build](logs/build-final.log). Native Endlaeufe:
[Zaehmung](logs/native-taming-final.log),
[Inventar](logs/native-inventory-final.log),
[Effekte](logs/native-effects-final.log),
[Truhe](logs/native-chest-final.log),
[Platzieren](logs/native-placement-final.log),
[Headless](logs/native-headless-final.log).
Diese nativen Endlaeufe enthielten keine ERROR- oder WARN-Zeilen.
