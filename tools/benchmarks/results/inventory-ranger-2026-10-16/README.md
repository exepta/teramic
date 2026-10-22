# Inventar, Waldlaeufer Und Bedienung - 16.10.2026

Technischer Pruefbericht zur aktuellen Inventar-/Avatar-/Bedienungsrunde.
Seed, Welt-IDs, Block-IDs, Weltformat 1 und Generatorversion 5 bleiben erhalten.
Keine Dependency-Upgrades, kein weiteres UI-Framework und kein separates
Inventar-, Prop- oder Speicherbackend. Netzwerkprotokoll jetzt **12**;
Client und Server muessen gemeinsam aktualisiert werden.

## Implementierter Backendpfad

Das bestehende `terra_core::Inventory` besitzt 27 stabile Slotindizes und
standardmaessig maximal 128 Einheiten pro Stapel. Leere Slots werden beim
Verbrauch nicht mehr zusammengezogen. Alte kompakte Slotlisten bleiben lesbar;
fehlende neue Revisionsfelder erhalten Serde-Defaults. Namen und Beschreibungen
kommen aus der bestehenden Blockdefinition/Materialregistry. Namespaced-IDs
verwenden beispielsweise `teramic::stone`; Props haben entsprechende stabile
Itemkennungen.

`InventoryRequest` verwendet den vorhandenen zuverlaessigen Lightyear-
Kontrollkanal. `InventoryCommand` enthaelt die erwartete Inventarrevision und
eine Aktion: `Move`, `Craft` oder `CreativeSet`. Der Server validiert Revision,
Slotgrenzen, Mengen und Spielmodus, bevor er den UUID-gebundenen Bestand im
bestehenden `EditJournal::gameplay` aendert. Die Antwort enthaelt den bestaetigten
Bestand und einen expliziten Ergebnisstatus. Das gilt fuer Integrated Server
und Dedicated Server gleichermassen.

- Verschieben kann gleiche Items bis zum Stapellimit vereinigen und vollstaendige
  unterschiedliche Stapel tauschen. Teilmengen werden vom Backend unterstuetzt.
- Die persoenlichen 2x2-Craftingzellen referenzieren echte Inventarslots. Ein
  Eichen-/Birkenholzblock ergibt vier passende Planks; vier Sand ergeben einen
  Sandstein. Doppelte Quellreferenzen benoetigen entsprechend viele Items.
  Verbrauch und Ergebnisablage sind atomar; Platzmangel veraendert nichts.
- `CreativeSet` befuellt einen echten Slot. Survival-Clients koennen damit keine
  Ressourcen erzeugen.
- `SelectedInventorySlot` bindet den aktiven Stapel an den vorhandenen
  Platzierungsbefehl. `InventorySource { slot, revision }` verhindert, dass
  beim Platzieren ein anderer identischer Stapel verbraucht wird. Veraltete
  Revisionen und falsche/leere Quellen werden ohne Welt- oder Inventaraenderung
  abgewiesen. Erst ein erfolgreicher Weltcommit verbraucht genau ein Item.
- Der kompatible interne Aufruf ohne explizite Slotquelle kann weiterhin einen
  passenden Stapel verwenden. Die slotbasierte Inventaroberflaeche liefert eine
  konkrete Quelle; die freie Materialpalette verwirft ihre alte Slotquelle.

## Gepflanzte Props Und Lose Ressourcen

Geerntete Pflanzen werden ueber denselben autoritativen Editpfad platziert.
Der Server prueft Reichweite, Sichtlinie, obere Blockflaeche, natuerlichen
Untergrund, freie Zelle, bestehenden Bewuchs und Wasser. Die Wasserpruefung nutzt
`fluid_cell_at`, also auch dynamische Fluessigkeiten aus dem Journal und nicht
nur die prozedurale Wasserhoehe.

Spielerplatzierungen liegen in `SectionDelta.placed_props`; entfernte IDs und
`prop_epoch` verwenden die vorhandene Propverwaltung. Keine Entity pro Pflanze
oder Microvoxel wird dafuer benoetigt. Die bestehenden Section-Batches reagieren
auf die Prop-Epoche, ohne Terrain und Wasser grundlos neu zu bauen. Worker-
Ergebnisse erhalten vor dem Einfuegen den neuesten Journalzustand. Wiederholtes
Anwenden derselben bestaetigten Delta erzeugt keine zweite Pflanze. Das Entfernen
einer platzierten Pflanze entfernt auch ihren Placement-Eintrag und bewahrt den
Entfernungszustand. Pro Section sind maximal 256 aktive Spielerplatzierungen
zugelassen; weitere Begrenzungen reservieren Platz fuer Entfernungseintraege.

Lose Ressourcen bleiben Teil von `GameplayState`, jetzt inklusive Geschwindigkeit
und Alter. Schwerkraft und kleine AABB-Kollisionen laufen autoritativ im
FixedUpdate; maximal 16 Substeps begrenzen einen Bewegungsschritt. Unbekannte
Terrainzellen werden nicht durchquert. In ungeladenen Sections ruht die Bewegung,
die Lebensdauer laeuft weiter. Nach 300 Sekunden laufender Serverzeit verfaellt
ein Drop. Offline-Zeit ist keine ablaufende Serverzeit. Gespeicherte Drops ohne
neue Felder starten kompatibel ohne Geschwindigkeit und mit Alter null.

Bestehende Grenzen bleiben wirksam: maximal 2048 lose Ressourcen weltweit,
maximal 256 replizierte nahe Drops je Client innerhalb von 48 Metern und
Snapshots hoechstens bei 10 Hz. Ruhende Dropalter muessen nicht in jedem
Physikschritt eine neue Replikationsrevision erzeugen. Es werden keine Avian-
Rigidbodies oder GPU-Abhaengigkeiten pro Drop auf dem Server erzeugt. Die kleine
visuelle Dreh-/Schwebeanimation bleibt kosmetisch relativ zur simulierten
Position. Volle Inventare lassen Ressourcen bis zum Ablauf in der Welt.

## Automatische Pruefungen

Die folgenden Workspace-Laeufe wurden ausgefuehrt und ihre Protokolle archiviert.
Builds verwendeten `CARGO_BUILD_JOBS=2`. Nach den letzten nativen UI-Korrekturen
wurden alle fuenf Cargo-Pruefungen am eingefrorenen Stand erfolgreich wiederholt.
Die nativen Pruefungen sind
weiter unten einzeln eingeordnet.

| Pruefung | Ergebnis dieses Arbeitsstands |
| --- | --- |
| `cargo fmt --all --check` | Exitcode 0; [Protokoll](logs/terra-inventory-fmt.log), erwartungsgemaess ohne Ausgabe |
| `cargo check --workspace` | Exitcode 0; [Protokoll](logs/terra-inventory-check.log) |
| `cargo test --workspace` | Exitcode 0; 470 bestanden, ein vorhandener 180-s-Netzwerksoak ignoriert; [Protokoll](logs/terra-inventory-tests.log) |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exitcode 0; [Protokoll](logs/terra-inventory-clippy.log) |
| `cargo build --workspace` | Abschliessender Freeze-Build Exitcode 0; [Protokoll](logs/terra-inventory-build.log) |

Die 470 erfolgreichen Tests enthalten Client 13, Core 23, Network 41, Physics 6,
Player 39, Render 125, Server 1 sowie 2 Headless-Integrationstests, Storage 7,
UI 47, Voxel 20 und World 146. Damit sind auch die zuletzt ergaenzte echte
Hotbar-Mausradnavigation, der Player-Regressionsfall und die zwei abschliessenden
Tooltip-/Creative-Regressionen im aktuellen Lauf enthalten. Die zuvor
beanstandete Edit-Systemsignatur wurde durch einen
`SystemParam` gebuendelt; komplexe Querytypen bekamen passende Typaliase statt
Clippy-Unterdrueckungen.

Abgedeckte Regressionen:

- Stabile Slotindizes, Merge/Swap, Stapellimit 128 und Laden alter Inventare.
- Echte Rezeptkosten, mehrfach referenzierte Zutaten und atomarer Fehler bei
  fehlendem Platz.
- Zwei echte UDP-Clients: Creative-Befuellung, Verschieben, Crafting,
  veraltete Befehle, Survival-Verweigerung und unveraendertes fremdes Inventar.
- Zwei identische Stapel: Platzierung verbraucht nur den ausgewaehlten;
  leere Quellen und alte Inventarrevisionen veraendern weder Terrain noch Items.
- Pflanzenplatzierung und Verbrauch, sichtbarer logischer Eintrag in beiden
  Client-ChunkStores, Reload und zweifache Delta-Anwendung ohne Duplikation.
- Speicherung platzierter Pflanzen, Slotluecken und Dropgeschwindigkeit/-alter
  in der bisherigen `edits.toml`, inklusive Legacy-Saves.
- Lose Ressourcen fallen, kollidieren mit Terrain und verfallen serverseitig.
- Der Player-Interaktionstest erzeugt nur einen autoritativen Pflanzenrequest;
  der lokale Client zieht noch keine Ressource ab.

Der vorhandene native Zwei-Client-Smoke prueft zusaetzlich reale replizierte
Spielerpositionen gegen die erzeugten Remote-Avatartransforms, sechs nichtleere
Koerperteilmeshes und genau einen Avatar pro Replikat. Ein Screenshotauftrag gilt
erst nach `ScreenshotCaptured` mit nichtleerem, mehrfarbigem Bild als fertig.
Die Testimplementierung allein ist kein Nachweis eines ausgefuehrten GPU-Laufs.

## Native Netzwerk- Und Chatpruefung

Ein echter Dedicated Server und zwei getrennte GPU-Clientprozesse wurden mit
dem produktiven Handshake-/Replikationspfad gestartet. Beide Clients beendeten
ihren Smoke mit Exitcode 0. Der Observer bestaetigte ein Fremdspieler-Replikat
und genau einen dazugehoerigen Avatar mit gueltigen sechs Koerperteilmeshes.
Der Companion bewegte sich autoritativ um 6.696 m. Beim spaeteren Companion-
Screenshot hatte der Observer bereits beendet; dort sind deshalb null entfernte
Spieler/Avatare protokolliert. Eine beidseitig gleichzeitig aufgenommene
Sichtpruefung wird damit nicht behauptet. Der Server beendete nach SIGINT seinen
ECS-Loop sauber mit Exitcode 0.

Protokolle: [Server](logs/terra-inventory-network-server.log),
[Observer](logs/terra-inventory-network-observer.log),
[Companion](logs/terra-inventory-network-companion.log).
Aufnahmen: [Observer](captures/terra-inventory-network-observer.png),
[Companion](captures/terra-inventory-network-companion.png).
Dieser kurze Funktionslauf ist kein Mehrspieler-FPS-Benchmark oder Langzeitsoak.

Der native Chat-Smoke bestand ebenfalls mit Exitcode 0 und drei abgeschlossenen
GPU-Captures. Er pruefte TiltUI-Tastatureingaben, `/locate biome desert`, farbige
Ergebnisdarstellung, Escape-Eingabesperre und serverbestaetigten Token-Teleport.
Der Server bestaetigte Generation 1 und die neue Position
`(-1815.5, 30.910006, -1799.5)` ausgehend von `(8.5, 113.91008, 24.5)`.
Die kompakte Ansicht wurde von Root visuell betrachtet.
[Chatprotokoll](logs/terra-inventory-chat.log),
[breite Ansicht](captures/chat-locate-wide.png),
[kompakte Ansicht](captures/chat-locate-compact.png),
[Zielregion](captures/chat-desert-destination.png).

## Native Inventar- Und Avatarabnahme

Die finalen Creative- und Survival-Inventarlaeufe endeten beide mit Exitcode 0
ohne Warnungen. Creative benutzt echte OS-`uinput`-Klicks fuer die Auswahl;
der Integrated Server bestaetigte einen Stapel mit 128 Stone und
Inventarrevision 1. Suche und deckende Itemtooltips wurden im laufenden TiltUI
geprueft. Die Aufnahmen decken die breite 1280x800-Ansicht und die angeforderte
kompakte 960x640-Ansicht ab. Die beiden kompakten PNGs haben durch die native
Fenster-/DPI-Rundung tatsaechlich 960x641 Bildpunkte. Root betrachtete beide
Creative-Bilder; der UI-Agent pruefte die Bilder beider Spielmodi.

Protokolle: [Creative](logs/terra-inventory-final-creative.log),
[Survival](logs/terra-inventory-final-survival.log).
Aufnahmen: [Creative breit](captures/inventory-creative-wide.png),
[Creative kompakt](captures/inventory-creative-compact.png),
[Survival breit](captures/inventory-survival-wide.png),
[Survival kompakt](captures/inventory-survival-compact.png).
Der Survival-Inventarlauf ist eine Mode-/Layoutabnahme; der echte Ressourcen-
Verbrauch wird im separaten Bau-Smoke unten nachgewiesen.

Der Avatar-Smoke bestand mit vier GPU-Captures. Ein Waldlaeufer besitzt 6384
Vertices in sechs geteilten Koerperteilmeshes; der Lauf zaehlte 12768 Vertices
ueber lokalen und entfernten Avatar zusammen und genau sechs gemeinsame
Mesh-Handles. Zwischen zwei Laufposen aenderten sich alle vier Gliedmassen.
Das 320x512-Portrait wurde aus denselben Assets mit echtem GPU-Readback erzeugt.
Dieser Test ist ausdruecklich eine Rendererfixture: Er steuert beide Avatar-
Renderpfade, ist aber kein Beleg fuer eine echte Netzwerkverbindung. Diese
liefert der getrennte Zwei-Client-Lauf oben.

[Avatarprotokoll](logs/terra-avatar-native.log),
[Portrait](captures/terra-avatar-portrait.png),
[Ruhepose](captures/terra-avatar-idle.png),
[Laufpose A](captures/terra-avatar-walk-a.png),
[Laufpose B](captures/terra-avatar-walk-b.png).

## Native Bau- Und Menueabnahme

Beide produktiven Bau-Smokes endeten mit Exitcode 0. Survival baut einen echten
Block ab, erhaelt bestaetigt ein Item und verbraucht dieses beim erfolgreichen
Wiederplatzieren: Inventarmenge 1 -> 0. Creative behaelt seinen zuvor erzeugten
128er-Stapel nach der Platzierung und erzeugt beim Abbau keine Drops. Die
laufenden Renderpfade bestaetigten Bruchpartikel; beim Survival-Test wurde
zusaetzlich ein Drop beobachtet. Das ist kein kuenstliches Setzen des
erwarteten Inventarergebnisses.

Protokolle: [Creative-Bau](logs/terra-inventory-building-creative.log),
[Survival-Bau](logs/terra-inventory-building-survival.log).
Archivierte Auswahl: [Creative-Inventar kompakt](captures/building/creative/building-inventory-small.png),
[Creative wiederhergestellt](captures/building/creative/building-restored.png),
[Survival-Inventar kompakt](captures/building/survival/building-inventory-small.png),
[Survival wiederhergestellt](captures/building/survival/building-restored.png).

Der abschliessende native Menue-Smoke endete mit Exitcode 0. Sein Protokoll
bestaetigt Seitennavigation, Welten erstellen/loeschen, Server hinzufuegen/
bearbeiten, Controls, Konflikterkennung, Escape, Apply, Verwerfen und den
Display-Rollback bei Timeout.
[Menueprotokoll](logs/terra-inventory-menu-final.log).

## Separate Headless-Abnahme

`CARGO_BUILD_JOBS=2 cargo build -p terra-server` bestand unabhaengig vom
Client-/Workspacebuild mit Exitcode 0. Anschliessend startete die Binary mit
entfernten `DISPLAY`- und `WAYLAND_DISPLAY`-Variablen:

```bash
env -u DISPLAY -u WAYLAND_DISPLAY ./target/debug/terra-server \
  --bind 127.0.0.1:0 --no-save --smoke-test
```

Ergebnis: Exitcode 0, echter UDP-Listener auf `127.0.0.1:32778`, trockener Spawn
ueber `[8, 112, 24]` und 27 geladene Sections. Dieser kleine Funktions-Smoke ist
kein Ladezeitbenchmark fuer die volle Sichtweite.
[Serverprotokoll](logs/headless-server.log).

`cargo tree -p terra-server -e features` wurde separat archiviert. Der Baum
enthaelt keine `bevy_render`, `bevy_pbr`, `bevy_winit`, `bevy_window`, `bevy_ui`,
`bevy_audio`, `wgpu`, `winit`, `tilt-ui`, `terra-render` oder `terra-ui`-
Abhaengigkeit. [Feature-Baum](logs/headless-features.txt).

## Reale Wayland-Eingaben

Die Root-Pruefung wurde mit nativen OS-Pointereingaben durchgefuehrt:
`uinput`-Maus -> KWin -> Winit, bei 960x640 physischen Pixeln und 150 % Skalierung.
Der logische Clientbereich war entsprechend etwa 640x427 Pixel gross.
Geprueft wurden Creative-Auswahl im Weltdialog, Scrollen, Sliderbewegung
12 -> 8, Verwerfen ungespeicherter Aenderungen und produktives Quit.
Der erfolgreiche zweite Clientlauf endete mit Exitcode 0. Ein vorheriger
normaler Clientlauf endete durch Timeout mit Code 124; dieser ist ausdruecklich
**kein bestandener Smoke-Test**.

Aufnahmen liegen unter `captures/terra-wayland-real-*.png`, insbesondere
[Dropdown](captures/terra-wayland-real-dropdown.png),
[Creative-Auswahl](captures/terra-wayland-real-creative.png),
[Scrollposition](captures/terra-wayland-real-scroll.png),
[Slider](captures/terra-wayland-real-slider.png) und
[Verwerfen](captures/terra-wayland-real-discard.png).
`tools/ui/wayland_pointer.py` ist ein optionales Testhilfsmittel, keine neue
Anwendungsabhaengigkeit und kein synthetischer TiltUI-Hit-Test.

## Performance Und Verworfene Messung

`logs/forest-before-incompatible.log` und die zugehoerige
`captures/forest-before.png` stammen von einer alten Clientbinary zusammen mit
bereits veraenderten TiltUI-Templates. Dieser Vorherlauf ist **VERWORFEN**.
Er ist weder eine gueltige Baseline noch ein Nachweis fuer Performanceparitaet
oder eine Verbesserung. Seine FPS duerfen nicht mit dem aktuellen Stand als
belastbarer Vorher-/Nachher-Vergleich verrechnet werden.

Die nativen Netzwerk-, Chat-, Avatar-, Inventar-, Bau- und Menueabnahmen sind
oben einzeln dokumentiert. Einige dieser GUI-Funktionstests liefen kurzzeitig
gleichzeitig. Aus ihnen werden **keine Performancewerte** abgeleitet.

Nach Abschluss aller Builds lief ein isolierter Nachher-Benchmark ohne weitere
von uns gestartete Cargo-/GPU-Prozesse. RX 6950 XT, RADV Mesa 26.2.4, Vulkan,
Wayland bei 150 Prozent Skalierung, tatsaechlich 1920x1200 physische Pixel,
VSync aus. Konfiguration siehe `configs/`, Seed 42042, Sichtweite 16 Sections,
LOD 96/160 m. Der historische Szenenname `--smoke-test-forest` bezeichnet hier
die aktuelle bewachsene Pilzparklandschaft bei Kamera `(-622, 53.88851, 680)`;
er ist kein reiner Waldtest. Offline-Renderingfixture, keine laufende
Netzwerksimulation und kein Bewegungs-/Streamingflug.

| Kennwert | Gemessener Nachherstand |
| --- | ---: |
| Messfenster / Frames | 18 s / 2808 |
| Durchschnittliche FPS | 155.97 |
| p95 / p99 CPU-Wall-Framezeit | 8.087 / 9.584 ms |
| Erster Ready-Zeitpunkt, inklusive Rendering | 3.154 s |
| Geladene / gerenderte Sections | 7807 / 4115 |
| Mesh-Assets / sichtbare Mesh-Entities | 9635 / 4546 |
| Repraesentierte Props / Section-Prop-Batches | 92789 / 1306 |
| Terrain-Meshing, Summe der Worker-Spannen | 0.590 s |
| Prop-Meshing, Summe der Worker-Spannen | 0.308 s |
| Prozess-RSS am Messende, `/proc` | 929876 KiB |
| Peak-RSS, separates `getrusage` | 925112 KiB |
| GPU Opaque-Pass Mittel / p95 / p99 | 1.188 / 1.253 / 1.739 ms |

Worker-Spannen sind weder serielle Ladezeit noch ein neuer reiner
CPU-Generierungsbenchmark. Die beiden Linux-Speicherzaehler stammen aus
unterschiedlichen Messwegen und werden unverrechnet wiedergegeben.
GPU-Passzeit ist keine gesamte GPU-Framezeit; exakte Draw Calls sind weiterhin
nicht instrumentiert. Die Propzahl bezeichnet in Batches repraesentierte
Objekte, nicht ausschliesslich gerade sichtbare Einzelpflanzen.
[Rohlog](logs/terra-inventory-forest-after.log),
[tatsaechlich betrachtete Aufnahme](captures/terra-inventory-forest-after.png).

Wegen der verworfenen Baseline bleibt dieser Benchmark ausdruecklich ein
Nachherlauf ohne belastbaren Vorher-/Nachhervergleich, keine behauptete
Beschleunigung oder Performanceparitaet. Fuer den Worst Case mit 2048
gleichzeitig bewegten Drops liegt bisher kein isolierter Benchmark vor.

## Grenzen

- Nur die drei beschriebenen persoenlichen Rezepte sind implementiert; kein
  allgemeiner Rezepteditor, keine Werkbaenke und kein Equipmentbackend.
- Slot-Umsortieren verwendet die Auswahl von Quelle und Ziel. Eine API fuer
  Teilmengen ersetzt nicht automatisch eine entsprechende Drag-/Split-UX.
- Prop-Items bewahren die Pflanzenart, aber keine individuelle Variantenform
  des geernteten Exemplars; eine Platzierung erhaelt eine deterministische
  Variante aus ihrer Position/Epoche.
- Automatisch durch Bodenentfernung ungueltige Pflanzen werden weiterhin ohne
  zusaetzlichen Drop entfernt; nur das gezielt abgebaute Objekt wird geerntet.
- Drop-Kollision ist eine kleine voxelbasierte AABB-Simulation, keine allgemeine
  Rigid-Body-Interaktion. Unbekannte/unbeladene Bereiche pausieren Bewegung.
- Lokale Profil-UUIDs sind weiterhin keine verifizierten Online-Accounts.
- Erfolgreiche CPU-/UDP-Tests ersetzen keine vollstaendige visuelle Abnahme und
  keine mehrstuendige Mehrspieler-/Maximalressourcen-Lastpruefung.
