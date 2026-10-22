# Phase 94: Bauteile, Betten, Kalender Und Inventar

Stand: 21. Oktober 2026. Dieser Bericht beschreibt die implementierten Pfade,
ausgefuehrten Pruefungen und verbleibenden Grenzen. Inventar, Gameplay-Menue,
Bett und Chat wurden nativ bedient; die korrigierte Winterdarstellung wurde
zusaetzlich am tatsaechlichen Clientbild geprueft.

## Pruefstand

| Pruefung | Ergebnis |
| --- | --- |
| Zentrale Workspace-Tests | 1624 bestanden, 2 bestehende Tests ignoriert. Einschliesslich neuer Stream-/Provenienz-, Prepass-, Preview- und Creative-Objektregressionen. |
| Format, Check, Clippy und Build | `cargo fmt --all --check`, `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` und `cargo build --workspace` bestanden. |
| Bauteilgalerie | Finaler nativer Lauf bestanden, Exit 0 und fuenf Galerieaufnahmen. |
| Bett-Ablauf | Finaler nativer Lauf bestanden, vier Aufnahmen: Zweizellen-Platzierung, genau ein Survival-Farbstoff, replizierte Liegepose und tatsaechliches Aufwachen um 08:00. |
| Jahreszeiten | Finaler nativer Lauf mit sieben Aufnahmen bestanden, Shader ohne ERROR/WARN; Winterbild selbst angesehen: keine schwebenden Holzfragmente oder Geister-Laubschatten mehr. 448 Blockhashes, 281 Meshhandles und 701 abgeschlossene Meshjobs bleiben ueber die Zustandswechsel erhalten; keine neuen Saison-Meshjobs. |
| Chat | Finaler nativer Lauf bestanden, Exit 0 und fuenf Aufnahmen; einschliesslich Fokusverlust, Escape, Locate/Teleport, Hilfeseiten und autoritativem Wetterwechsel. |
| Inventory | Finaler nativer Lauf bestanden, neun Aufnahmen. Echter OS-Zeiger prueft Tools/All-Dropdown und aktuelle 3D-Previews, Inventar-/Crafting-MouseTweaks, Dropkonservierung, Tooltips, Scrollen und Trash fuer Cursor- und ausgewaehlten Stack mit Serverbestaetigungen. |
| Gameplay-Menue | Finaler nativer Lauf bestanden: echter TiltUI-Tab, Popup, Widgets, staged Apply, TOML-Reload, begrenzter MiniMap-Reset und expliziter Gesamtreset. |
| Headless-Server | Ohne DISPLAY und WAYLAND_DISPLAY mit 27 Sections bestanden; [Protokoll](phase94-server.log). |
| FPS und Langzeitperformance | Keine neuen FPS-, GPU-Framezeit- oder Langzeitmessungen fuer Phase 94. |

Vorhandene Laufprotokolle: [Bauteile](construction.log), [Bett](bed.log),
[Jahreszeiten](seasons.log), [Chat](chat.log), [Inventory](inventory.log),
[Gameplay-Menue](menu.log).
Ein vorhandenes Bild allein gilt nicht als bestandener Gesamttest; die
erfolgreichen Endmeldungen stehen in den zugehoerigen Laufprotokollen.

Archivierte zentrale Nachweise: [Check](phase94-check.log),
[1624 Workspace-Tests](phase94-tests.log), [strenges Clippy](phase94-clippy.log),
[Build](phase94-build.log). `cargo fmt --all --check` endet ebenfalls mit 0.
Elf Python-Tests der vorhandenen Benchmark-Werkzeuge bestehen; die vier
Avian-Stufentests sind Teil der Workspace-Pruefung.

## Bauteile Und Materialien

24 Bauteile erweitern die bestehende Blockregistry um IDs 74..97:

| Form | IDs | Materialien |
| --- | --- | --- |
| Treppe mit drei Stufen | 74..81 | Eiche, Birke, Fichte, Stein, Erde, feiner Sand, Glas, Eis |
| Halbplatte | 82..89 | Dieselben acht Materialien |
| Zaun | 90..97 | Dieselben acht Materialien |

Die einzelnen JSON-Definitionen unter `assets/terra/blocks/` liefern
Boxprofile und vorhandene Materialreferenzen. Rendering, Picking und
Kollision verwenden dieselben Profile. Zaunarme verbinden sich abhaengig
von Richtung und Nachbarblock. Die Teile bleiben in den vorhandenen
Section-Batches; es gibt keine eigene Entity fuer jedes Teilstueck und
keine neue Modellbibliothek als Laufzeitabhaengigkeit.

Die normale Bewegung ueberwindet Stufen bis 0,52 m. Ein voller Meter bleibt
an die vorhandene Auto-Climb-Regel gebunden. Die Gestaltung ist eine eigene
Umsetzung, keine 1:1-Kopie einer Referenz.

## Betten Und Farbstoffe

Eichen-, Birken- und Fichtenbetten sind als IDs 71..73 implementiert. Die
drei wiederverwendeten Modelle enthalten Pfosten, Kopf-/Fussteil, Kissen
und eine gesteppte Decke. Ein Bett belegt zwei horizontale Zellen; beide
teilen Holzart, Ausrichtung und Farbe. Die vorhandenen Objektzustands- und
Speicherpfade erhalten diese Angaben auch beim Abbau und erneuten Platzieren.

Sechs Farbstoffe besitzen eigene Rezepte unter `assets/crafting/`: Rot,
Gelb, Blau, Orange, Gruen und Violett. Rot entsteht aus einer Blume; die
anderen Grundfarben kombinieren Blumen mit vorhandenem Pflanzenmaterial.
Orange, Gruen und Violett entstehen durch Mischen der Grundfarbstoffe.
Mit einem gehaltenen Farbstoff wird ein Bett eingefaerbt. Survival verbraucht
dabei genau ein Item aus dem ausgewaehlten echten Inventarslot.

Ohne Farbstoff beginnt die Bettbenutzung den autoritativen Schlafablauf.
Im Fruehling/Herbst beginnt die Schlafzeit um 19:00, im Sommer um 20:00,
im Winter um 18:00; Schlafen ist jeweils auch vor 08:00 moeglich. Sind alle
verbundenen lebenden Nicht-Spectator-Spieler mindestens zwei Sekunden in
verschiedenen gueltigen Betten, setzt der Server die Uhr auf 08:00. Nach
Mitternacht wird kein weiterer Tag uebersprungen. Bewegung, Springen,
Entfernung, Tod, Bettabbau und Disconnect beenden ungueltige Schlafplaetze.

Avatar und Kamera liegen sichtbar im Bett. Die physikalische Kapsel bleibt
aufrecht am sicheren benachbarten Einstiegspunkt; eine horizontale
Schlafkapsel ist nicht implementiert.

## Kalender, Wetter Und Vegetation

Der Kalender folgt dem gespeicherten `WorldClock.day` und der vorhandenen
autoritativen Uhr. `[seasons]` in `world.toml` konfiguriert `enabled`,
`spring_days`, `summer_days`, `autumn_days`, `winter_days`, `start_season`
und `transition_days`. Defaults sind sieben Tage pro Jahreszeit, Start im
Fruehling und zwei Tage weicher Uebergang. Alte Weltmanifeste ohne diese
Felder erhalten die Defaults; Root-Konfigurationen ueberschreiben keine
bereits gespeicherten Weltmanifeste.

Jahreszeiten beeinflussen Gras-/Laubfarben, Beleuchtungsanmutung,
Wetterwahrscheinlichkeiten und lokale Temperatur. Biomklima und Hoehe
bestimmen weiterhin Regen oder Schnee. Bestehende Biome werden nicht gegen
Winterbiome ausgetauscht; der Kalender regeneriert keine komplette Welt.
Wiederbesuchte Spalten erhalten eine begrenzte Klima-Nachfuehrung mit maximal
3600 normalisierten Wettersekunden und 8192 Eintraegen. Das ersetzt keine
lueckenlose historische Wettersimulation.

Die Schneedecke und das Schmelzen laufen kontinuierlich ueber die bestehenden
Shader. Lokale Temperatur, Exposition und gespeicherte Oberflaechenzustaende
steuern die Bedeckung. Eine eigene Schneeroof-Maske laesst Schnee unter
kahlen natuerlichen Laubkronen zu, waehrend Spielerdaecher und gesetztes Laub
abschirmen. Natuerliche Laubblaetter und ihre Schatten werden saisonal
ausgeduennt; Holzaeste bleiben sichtbar. Fichten behalten ihre Kronen und
koennen exponierte Schneekappen tragen. Dach-, Wasser- und gemischte
Baumartenfaelle wurden im abschliessenden Korrekturpfad beruecksichtigt.

Laubfall, Schneekappen und Bedeckung sind kosmetisch: Blattvoxel und ihre
Kollision bleiben bestehen. Ein gemeinsamer 80-Byte-Wetterpuffer traegt die
dynamischen Werte; Saisonanimationen erzeugen keine permanenten
Mesh-Rebuilds oder Materialien pro Zelle. `SeasonSample` stellt gemeinsame
Regeln fuer kuenftige Systeme bereit; neue saisonale Ernte- oder
Tiermechaniken sind damit nicht implementiert.

Netzwerkprotokoll 52 verlangt passende Client-/Serverbuilds. Generator- und
Regionsversionen bleiben unveraendert.

## Winter-Prepass-Korrektur

Die native Winteraufnahme zeigte zunaechst scheinbar schwebende Holzsplitter.
Die erste Vermutung zur Blatt-Mikrogeometrie war nicht die Bildursache:
Auch nach eindeutiger CPU-Ownerzuordnung blieb das Muster bestehen.
Die lokale Bevy-0.19.1-Quelle belegt den eigentlichen Fehlerpfad:
`pbr_prepass_functions::prepass_alpha_discard` beruecksichtigt Vertex-Alpha
nicht. Der Farbpass entfernte kahle Blaetter, der Depth-/Shadow-Prepass
schrieb aber weiter ihre Masken und verdeckte das Holz bis auf kleine Loecher.

`LeafExtension::prepass_fragment_shader` verwendet jetzt den materiallokalen
`terra_foliage_prepass.wgsl`: Basisfarbe, Texturalpha und bereits saisonal
berechnete Vertex-Alpha werden mit demselben Mask-Cutoff geprueft. Normals,
Normalmaps, Motion-Vectors und Orthodepth bleiben erhalten. Fasern/Flammen
delegieren weiterhin an die Standardpruefung; Standardterrain wird nicht
global gepatcht. Kein zusaetzlicher Renderpass oder PointLight.

[Vorher](seasons-before-prepass/season-3-winter.png) und
[nachher](seasons/season-3-winter.png) wurden im nativen Client aufgenommen
und angesehen. Der finale Sieben-Zustaende-Lauf endet ohne ERROR/WARN;
zusammenhaengende Aeste sind sichtbar, die Geistermasken verschwunden.
Das ist ein Funktions-/Bildnachweis, kein gemessenes FPS-Verbesserungspaar.

## Inventar, Chat Und Abbau

Die Creative-Muelltonne loescht zuerst den gehaltenen Mausstack, sonst den
ausgewaehlten echten Inventarslot. Der normale Request-/Revisionspfad und
eine serverseitige Creative-Pruefung bleiben massgeblich. Es entsteht kein
Welt-Drop; Survival und Spectator duerfen den Befehl nicht verwenden.
Der alte Action-Abbrechen-Footer samt gehaltenem Itemnamen ist entfernt.
Die Muelltonne liegt ausserhalb des scrollbaren Inventarinhalts.

TMI zentriert Raster, Ueberschrift und Kategorien innerhalb seiner
zugewiesenen Sidebar. Bei Platzmangel bleibt die echte TiltUI-Kategorieauswahl
aktiv. Die Standardgruppen lauten All, Building, Decoration, Materials,
Animal Materials, Food und Tools; die bisherige Wood-Gruppe gehoert zu
Building. Der native Test verlangt einen echten Inhaltswechsel und prueft
die angezeigten Item-Previews, nicht nur die Anzahl von UI-Elementen.

Ein geoeffneter Chat bleibt bei Enter, Klick ausserhalb, Fokusverlust und
Teleport offen; Escape schliesst ihn. Die Hilfeseitenpruefung vergleicht
Befehle, Beschreibungen und Navigation mit der aktuellen Befehlstabelle,
einschliesslich `/spawn`. Survival-Abbau von Gras liefert Erde; Creative-
Picking waehlt weiterhin Gras.

## Aufnahmen

| Motiv | Dateien |
| --- | --- |
| Bauteile und drei Holzbetten | [Galerie 0](construction/construction-0.png), [1](construction/construction-1.png), [2](construction/construction-2.png), [3](construction/construction-3.png), [4](construction/construction-4.png) |
| Bettbenutzung | [Intaktes Bett](bed/bed-intact.png), [Survival-Farbe](bed/bed-purple-survival.png), [Liegepose](bed/bed-sleeping-pose.png), [Morgen](bed/bed-morning-awake.png) |
| Vier Jahreszeiten | [Fruehling](seasons/season-0-spring.png), [Sommer](seasons/season-1-summer.png), [Herbst](seasons/season-2-autumn.png), [Winter](seasons/season-3-winter.png) |
| Schnee und Schmelzen | [Schneefall](seasons/season-4-winter-snow.png), [Zwischenstand](seasons/season-5-spring-melt-mid.png), [Geschmolzen](seasons/season-6-spring-melt.png) |
| Inventory | [Geoeffnetes Inventar](inventory/inventory-open.png), [breite Ansicht](inventory/inventory-creative-wide.png), [Crafting-Ziehen](inventory/crafting-mouse-drag.png), [Muelltonne](inventory/inventory-creative-trash.png); kompletter nativer Lauf bestanden |
| Chat | [Breite Ansicht](chat/chat-locate-wide.png), [kompakte Ansicht](chat/chat-locate-compact.png), [Ziel](chat/chat-desert-destination.png), [Hilfe 1](chat/chat-help-page1.png), [Hilfe 2](chat/chat-help-page2.png); finaler nativer Lauf bestanden |

## Verbleibende Grenzen

- Die lokale Winter-Sichtpruefung ist nach der Depth-/Shadow-Prepass-Korrektur
  bestanden. Langzeit-/Mehrklima-Art-Abnahme bleibt getrennt davon offen.
- Reine Streaming-Ankuenfte von Dach-/Laubdaten besitzen noch keine abschliessend
  langfristig profilierte Kostengarantie. Die gezielte Invalidierung ist jetzt
  implementiert: nur verfolgte gleiche-XZ-Besitzer, begrenzte Maskenvergleiche,
  kein Remesh bei identischen Masken und eine Pre-Upload-Barriere fuer alte Jobs.
- Liegepose und Kamera verwenden weiterhin die aufrechte Kapsel am Einstieg.
- Kosmetischer Blattfall entfernt weder Blockdaten noch Kollision.
- Die Klima-Nachfuehrung ist begrenzt und keine historische Vollsimulation.
- Dieser Bericht enthaelt keine neue FPS-Abnahme und keine Aussage ueber
  Leistungsgewinne gegenueber Phase 93.

Architektur und Benutzung sind im
[Phase-94-Abschnitt der Hauptdokumentation](../../../../README.md#bauteile-betten-und-kalender-phase-94)
beschrieben. Die funktionalen Pruefungen ersetzen keine langfristige
FPS-/Speicherabnahme oder kuenstlerische 1:1-Freigabe.

## Reproduzieren

Native Pruefungen laufen auf der RX 6950 XT (RADV/Vulkan) mit getrennten
Konfigurations- und Weltpfaden unter `target/phase94/`, Seed 42042,
Generator 5 und Sichtweite 6. Nutzerwelten/-konfigurationen bleiben unberuehrt.
Galerie, Bett und Saisonaufnahmen besitzen 2880 x 1620 physische Pixel;
der Inventarlauf prueft kompakte und breitere Fenster gesondert.
Diese funktionalen Szenen sind keine 4K-FPS-Messung oder Langzeitabnahme.

Gemeinsame Umgebungsvariablen fuer die Clientlaeufe:

```bash
export TERRA_CONFIG_DIR="$PWD/target/phase94/configs"
export TERRA_MENU_CONFIG_DIR="$TERRA_CONFIG_DIR"
export TERRA_WORLD_DIR="$PWD/target/phase94/worlds-test"
export TERRA_CAPTURE_DIR="$PWD/tools/benchmarks/results/seasons-construction-2026-10-21/repeat"
```

Die Testkonfiguration wird vor dem Aufruf separat vorbereitet; die
Saison-Forestansicht verwendet eine pausierte Mittagsuhr und die Kalendertage
0/7/14/21. Keine alte binaere Phase-93-Datei gegen neue Shader verwenden:
der gemeinsame Wetterpuffer besitzt jetzt 80 Bytes.

```bash
target/debug/terra-client --smoke-test-construction
target/debug/terra-client --smoke-test-seasons
TERRA_FURNITURE_MINING_SMOKE=bed TERRA_BUILD_CREATIVE=1 target/debug/terra-client --smoke-test-building
TERRA_NATIVE_OS_POINTER=1 target/debug/terra-client --smoke-test-inventory
target/debug/terra-client --smoke-test-chat
TERRA_MENU_SMOKE_DIR="$PWD/target/phase94/menu-repeat" target/debug/terra-client --smoke-test-menu-gameplay
env -u DISPLAY -u WAYLAND_DISPLAY target/debug/terra-server --smoke-test --no-save
```

Die Inventarpruefung bewegt den echten OS-Zeiger ueber den vorhandenen
Wayland/uinput-Helfer; synthetische Eventdaten allein gelten hier nicht als
Mausabnahme. Der Bettlauf benutzt den integrierten UDP-Server und Survival-
Inventarverbrauch. Persistenz ueber Regionsgrenzen, Jahreswechsel und
abweichende Generierungsreihenfolgen werden zusaetzlich in Rust getestet.
