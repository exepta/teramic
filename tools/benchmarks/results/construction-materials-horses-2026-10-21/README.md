# Phase 95: Bauteilausrichtung, Materialdetails Und Pferde

Stand: 21. Oktober 2026. Implementierung im bestehenden Workspace, ohne neue
Dependencies, Rendering-Engine, Wasserlogik oder Weltgeneratorversion.
Die folgenden Angaben unterscheiden Implementierung, Messung und Art-Abnahme.

## Bauteile Und Inventar

- Halbplatten derselben Materialart passen paarweise in eine Zelle. Untere,
  obere und vier seitliche Haelften werden durch die angeklickte Auflageflaeche
  bestimmt. Ein weiterer Klick auf die innere Schnittflaeche vervollstaendigt
  die Platte. Survival verbraucht eine weitere Haelfte; der Doppelblock gibt
  beim Abbau zwei Halbplatten zurueck. Obere und zusammengefuegte Platten
  zaehlen auch fuer die autoritative Bett-Bodenpruefung als volle Auflage,
  untere und seitliche Haelften nicht. Verschiedene Materialien innerhalb
  derselben Zelle sind nicht implementiert.
- Treppen besitzen 24 eigentliche Wuerfelrotationen. Ihre Stufen zeigen zum
  ankommenden Spieler; Boden, Decke und vier Waende sind Auflageflaechen.
  Rechtwinklige Nachbartreppen erzeugen Innen-/Aussenecken im Auflageplan.
  Wandverkleidungen mit gleichem Anstieg verbinden sich auch an senkrecht
  zueinander stehenden Auflageflaechen durch vereinigte, feste Drittelprofile.
- `terra-core::construction` liefert dieselben begrenzten Boxprofile fuer
  Meshing, Raycast und Physik. Die Profile bleiben im Section-Batch; es gibt
  keine Entity und kein eigenes GLB fuer jedes Treppenteil.
- Sandzaun-ID 95 bleibt fuer alte Spielstaende lesbar. Rezept, Katalogeintrag
  und neue Platzierung sind gesperrt; bestehende Block-IDs bleiben unveraendert.
- Der Klickpfad fuer Farbstoff-Vorschaubilder verwendet nun dieselben IDs wie
  der Itemkatalog (`item-red_dye` statt `item-red-dye`). Die sechs vorhandenen
  Pflanzen-/Mischrezepte bleiben erhalten. Kategoriehosts haben feste Breiten
  in einem die gesamte Seitenleistenbreite nutzenden, zentrierten Grid.

Die bisherigen Plattenzustaende 0..3 bleiben untere Haelften. Neue Zustaende
4..8 sind die anderen Haelften, 9 ist die Doppelplatte. Treppen behalten ihre
alten aufrechten Rotationstags 0..3. Die Erweiterung verwendet vorhandene
Objektmetadaten, Revisionspruefungen, Netzwerkdeltas und Regiondateien.

## Kalender Und UI

`/season set spring|summer|autumn|winter` funktioniert ueber die gemeinsame
Command-Grammatik, Vervollstaendigung und Serverkonsole ohne Slash. Der Befehl
springt vorwaerts zum ersten Tag der gewaehlten Jahreszeit, behaelt die Uhrzeit
und schreibt den vorhandenen autoritativen Weltkalender. Deaktivierte Saisons
werden mit einer Fehlermeldung abgewiesen. Die MiniMap-Uhr nennt die Jahreszeit.

`Options > Graphics > UI FPS` verwendet standardmaessig 60. Die Einstellung
begrenzt die Publikation der retained Menu-/Inventar-/Status-/Zielansichten.
Die oeffentliche TiltUI-Resource `UiFrameRate` wird mitgefuehrt. TiltUI 0.1.0s
`with_ui_fps` installiert dagegen einen Sleep fuer die gesamte Bevy-Schleife;
dieser wird bewusst nicht installiert. Gameplay, Eingabe, Cursor, MiniMap-
Eigenfrequenz und GPU-Komposition laufen unabhaengig weiter. Es handelt sich
nicht um einen separaten, auf 60 Hz gecachten Offscreen-UI-Renderpass.

Alle neuen Einstellungen verwenden die vorhandenen Entwuerfe, Apply, Reset,
Verwerfen und TOML-Speicherung. Keine zweite Grafik-Konfiguration.

## Material- Und Geometriepfad

Der vorhandene `WetMaterial`-PBR-Pfad erhaelt in
`assets/shaders/terra_wet_surface.wgsl` einen gemeinsamen Materialparameterblock.
Keine zusaetzlichen Fullscreen-Passes und keine Shader-Variante pro Block.

- Stein: versetzte Segmente, Fugen, kleine Risse, prozedurale Normalen und
  dezente Kontaktabdunklung. Reliefgeometrie bleibt innerhalb der geschlossenen
  Blockhuelle (maximal 45 mm Einrueckung) und wird im vorhandenen Near-Mesher
  gebuendelt. Das bestehende Rand-/Tiefenbudget wird nicht erhoeht.
- Erde: feinere Erdklumpen, matte Oberflaeche und kleine Farbabweichungen.
- Gras: feinere Normalen, gruene Oberflaechen und eine geometrisch unregelmaessige
  Gras-Erde-Kante. Ein gemeinsames Kantengitter schliesst beide Materialflaechen
  ohne T-Junctions; kleinere Basisfelder reduzieren die jeweilige Reliefaufloesung,
  statt pro Unterteilung nochmals das volle Raster auszugeben.
- Sand: Korn-/Rippelstruktur und nur 4,5 mm maximales geometrisches Relief,
  keine groben Kieswuerfel. Schnee besitzt feinere 6-mm-Vertiefungen und ein
  ruhiges, mattes Mikroprofil.
- Materialkontakte erhalten im Nahbereich einen schmalen Farbsaum fuer
  Gras/Erde, Erde/Stein, Stein/Sand und Stein/Schnee. Ein 18er-Jobhalo liest
  reale Nachbar-IDs einschliesslich Section-Grenzen, danach wird er freigegeben.
  Dies ist keine beliebige Mischung saemtlicher Texturen und aendert keine IDs.
- Eiche, Birke und natuerliche Fichte verwenden unterschiedliche Kronenprofile
  und vorhandene Artenfarben. Birke bleibt luftiger, Fichte kompakter/dunkler.
  1536 moegliche Profile sind einzeln lazy initialisiert, nicht bei jeder
  Section oder als unbeschraenkter Cache erzeugt. Masken, Wind und der
  bestehende korrekte Alpha-/Schatten-Prepass bleiben erhalten.

LOD/Greedy-Meshing, Materialidentitaet und Mipmaps bleiben bestehen. Der
Micro-Abstand verwendet weiterhin Hysterese. Grafik-Aenderungen invalidieren
eine reine Renderrevision; sie manipulieren weder Weltedits noch Chunkdaten.
Veraltete Jobs werden durch die bestehende Upload-/Revisionspruefung verworfen.

Neue echte Regler unter `[blocks]` in `configs/graphics.toml`:
`detail_quality`, `micro_geometry`, `micro_geometry_distance`, `material_detail`,
`normal_mapping`, `surface_variation`, `leaf_geometry_quality`.
Qualitaeten: Low/Medium/High; Default High, 48 m Micro-Abstand. Low-Blattqualitaet
verwendet maskierte Greedy-Flaechen, Medium vereinfachte Volumina, High volle
Kronenprofile. Logische Props werden dadurch nicht entfernt.

Die Normalendetails sind analytisches Bump-Mapping im vorhandenen WGSL-Pfad,
keine neue Normalmap-Dateibibliothek. Es gibt keine neue GI-, SSS-, Tessellation-
oder Displacement-Engine und keine behauptete exakte Referenzkopie.

## Pferde

`assets/terra/mobs/horse.json` registriert die neue Art am Ende der vorhandenen
Artenliste. Leben 20, Hunger 30, Durst 25; natuerliche Dreiergruppen kommen
seedbasiert auf geeigneten Plains-Grasflaechen vor. Vorhandene Populationen und
die bestehenden lokalen/globalen Sicherheitsbudgets bleiben erhalten.

16 Fellvarianten: Bay, Black, Chestnut, White, Dapple Gray, Buckskin, Palomino,
Roan, Black with Socks, Chestnut with Blaze, Brown Pinto, Black Pinto,
Appaloosa, Gray, Cream und Brown. Je Variante gibt es acht wiederverwendete
Modellteile und ein gemeinsames Standardmaterial, keine Materialkopie je Tier.
Gehen, Fressen, Trinken, Schwimmen, Flucht, Blinzeln, Schweif, Schlafen und
ballistische Hindernisspruenge verwenden beziehungsweise erweitern die
vorhandenen Systeme. Karte und Ziel-HUD kennen die Art.

Ausdauer-Kapazitaet 65..200 und maximale Sprunghoehe 1,1..2,25 m werden stabil
aus der gespeicherten ID ermittelt und im Ziel-HUD angezeigt. Reload wuerfelt
sie nicht neu. Ein Reit-/Zaehmungs-/Zuchtsystem oder laufender Reiter-Ausdauer-
verbrauch ist damit nicht implementiert. `/spawn teramic::horse` ist verfuegbar.
Pferde verwenden vorerst die vorhandene Huf-Foley-Bank; eigene Wieher-Aufnahmen
wurden nicht hinzugefuegt.

Netzwerkprotokoll 53: Client und Dedicated Server gemeinsam neu starten.
Generator 5, Block-IDs, RGA0-Regionen und PLY4-Spielerdaten bleiben erhalten.

## Messung Und Pruefungen

Die unveraenderte native Baseline liegt in `before-idle12.{log,json,csv,png}`.
Der 900-Sekunden-Lauf verwendet einen separaten Binaer-/Asset-Snapshot und
isolierte Konfigurationen: Seed 42042, integrierter 60-Hz-Server, Sichtweite 12,
3840 x 2124 physische Pixel, pausierter Mittag, klares Wetter, High-Schatten,
High-Wasser und vorhandene UI/Minimap. VSync und Spiel-FPS-Limit sind aus.
Hardware: RX 6950 XT / Ryzen 9 5900X / 32 GiB. Keine Compiler oder zweiten
Testclients liefen gleichzeitig mit diesem FPS-Lauf. Der normale Desktop blieb
aktiv; dies ist keine hardwareisolierte Laborumgebung.
TiltUI verweist weiterhin auf die Repository-Templates unter `src-ui`; nur
Binaerdatei, Spielassets und Konfiguration sind fuer die Baseline separat.
Die zwischenzeitlich bearbeiteten Options-/Inventartemplates waren im
geschlossenen Gameplay-HUD dieses Laufs nicht geoeffnet. Das Messpaar ist kein
vollstaendig eingefrorener UI-Quellbaum und keine isolierte Shader-Mikromessung.

### Native Funktionspruefungen

- `--smoke-test-materials`: echte Journal-Edits, Section-Mesher und 97 GPU-
  gepruefte Blockvorschauen; acht Aufnahmen einschliesslich Terrain, Laub,
  gedrehten Treppen/Halbplatten, verbundenem Glas ueber X=16 und solidem
  Wasser. Galerieposen verwenden die echte Objektmetadaten-/Remeshfunktion;
  die autoritative Verbrauchs-/Platzierungspruefung ist davon getrennt.
  Die letzte Materialgalerie verwendet nur fuer diese explizite Testszene
  `snow_height = 256`, damit ihre hohe Plattform nicht die Grasprobe mit der
  vorhandenen kosmetischen Schneedecke verdeckt. Die Performance-Konfiguration
  bleibt dagegen unveraendert bei 112. Die alte Testszene setzte
  Deckenpflanzen ohne Halterung; sie besitzt nun reale Stuetzzellen. Kein
  Entfernen der Supportregel, um einen Test gruen zu machen.
- `TERRA_HORSE_INSPECT=1 --smoke-test-creatures`: vier Aufnahmen aller 16
  Varianten sowie Gehen/Ruhe/Schlafen mit den realen gemeinsamen Tiermeshes.
  Ziel-HUD zeigt Leben und die stabilen individuellen Kapazitaeten. Dies ist
  eine Modell-/Poseninspektion, kein Nachweis autonomer Herdennavigation.
- `--smoke-test-seasons`: sieben generierte Waldansichten inklusive Winter,
  Schneefall und Schmelzen. 448 behaltene Blocksection-Hashes und 281
  Mesh-Besitzer bleiben unveraendert; keine zusaetzlichen Meshingjobs durch
  den reinen Saisonwechsel (701 vor/nach).
- `TERRA_NATIVE_OS_POINTER=1 --smoke-test-inventory`: neun Screenshots und
  echte OS-Mausereignisse mit Serverbestaetigung fuer Dropdownkategorien,
  Creative-Entnahme, MouseTweaks, Crafting-Drag, Drop und Muelltonne.
- Zusaetzlicher 4K-Layoutlauf: alle 146 Katalogitems auf einer Seite und
  Kategorie-/Itemzentrierung anhand realer `ComputedNode`-Grenzen geprueft.
  Ein anschliessender 4K-OS-Zeigerversuch scheiterte am Desktop-Zeigerlimit:
  Ziel X=5276,7 lag ausserhalb des vom Helfer erreichbaren X=5119. Dies ist
  kein bestandener 4K-Klick-/Tooltiplauf. Die vollstaendige OS-Klickabnahme
  bei normaler Fenstergroesse und die 4K-Layoutpruefung sind getrennte Nachweise.
- `--smoke-test-menu`: reale TiltUI-Optionsdaten, Welt-/Serverseiten,
  Erstellen/Loeschen, Rebinding, Konflikte, Apply/Verwerfen, TOML-Reload und
  Display-Rollback. UI FPS 60 wird als Default aufgebaut und 120 tatsaechlich
  gespeichert. Der Bildvergleich deckte eine unerwuenscht inline ausgegebene
  dynamische Optionsliste auf; feste Optionen wie im vorhandenen FPS-Limit
  korrigieren das Dropdown. Der Testtreiber fordert jetzt kontinuierliche
  Frames an, weil seine synthetischen Events keine nativen Fensterereignisse
  erzeugen. Das aendert nicht den produktiven Fokus-/Fenstermodus.
- `terra-server --bind 127.0.0.1:0 --no-save --smoke-test` ohne DISPLAY oder
  WAYLAND_DISPLAY: erfolgreich, 27 Sections, echter UDP-Listener, sauberer Exit.

Die Material-/Pferde-/Wald- und korrigierten Menuebilder wurden tatsaechlich
geoeffnet und kontrolliert. Keine exakte 1:1-Referenzabnahme behauptet. Der
isolierte Inventarfixture meldet einmal einen ungenutzten CreatureStates-
Nachrichtenleser; dies ist getrennt von Shader-/Allocatorfehlern und kein
Beleg fuer eine vollstaendig warnungsfreie Mehrspielerabnahme.

### Automatisierte Pruefungen

1642 Workspace-Tests erfolgreich, zwei bestehende Tests ignoriert.
`cargo fmt --all --check`, `cargo check --workspace`,
`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
und `cargo build --workspace` erfolgreich. Elf Benchmark-Werkzeugtests bestehen.
Die Protokolle liegen unter `target/phase95/`; abschliessende Tests und native
Pruefprotokolle sind zusaetzlich in diesem Ergebnisordner archiviert.

Neue Regressionen pruefen sechs Halbplattenlagen, Zusammenbau/Verbrauch/
Abweisung einer dritten Haelfte, 24 eigentliche Treppenrotationen, Ecken in
allen Auflageplaenen, Wandprofile, Farbstoffauswahl ueber echte UDP-Transfers,
Kalenderbefehl ueber Chat/Serverkonsole mit Replikation/Speicherung,
UI-Takt ohne Gameplay-Limit, Grafik-Apply/Reset/Reload, Bettauflagen ueber
allen acht Plattenmaterialien und sieben Lagen, negative Section-Halos,
ungefaerbte transparente Nachbarn, geschlossene Cube-/Materialkanten,
begrenzte Kronengeometrie und Pferdevarianten/-attribute/-Hindernisspruenge.
Zwei neue Grasrelief-Prototypen wurden durch die bestehenden Geschlossenheits-
und Detailtests abgewiesen und vor der Endmessung korrigiert; die Tests wurden
nicht entfernt oder durch angehobene Geometriebudgets abgeschwaecht.

### 15-Minuten-Messpaar

Beide nativen Laeufe sind mit Exit 0 abgeschlossen. Daten:
`before-idle12.{log,json,csv,png}` und `after-idle12.{log,json,csv,png}`.
Die Tabelle zeigt jeweils das tatsaechliche 30-Sekunden-Messfenster am
Checkpoint, nicht einen hochgerechneten oder interpolierten Durchschnitt.

| Laufzeit | FPS vorher | FPS nachher | RSS vorher MiB | RSS nachher MiB | p99 vorher ms | p99 nachher ms |
|---|---:|---:|---:|---:|---:|---:|
| 1 Minute | 108,88 | 111,46 | 1227,92 | 1185,77 | 14,660 | 14,317 |
| 5 Minuten | 108,40 | 113,49 | 1272,67 | 1245,50 | 14,917 | 14,256 |
| 10 Minuten | 108,38 | 113,85 | 1312,89 | 1282,72 | 15,355 | 14,069 |
| 15 Minuten | 108,89 | 110,43 | 1330,46 | 1298,61 | 15,696 | 15,386 |
| 30 Minuten | nicht gemessen | nicht gemessen | nicht gemessen | nicht gemessen | nicht gemessen | nicht gemessen |

Ueber alle Messfenster gewichteter Durchsatz: 108,43 -> 112,26 FPS. Median
am 15-Minuten-Punkt: 9,077 -> 8,918 ms; p95 13,143 -> 12,891 ms. Das maximale
Einzelframe im gesamten Lauf betraegt 169,184 -> 150,581 ms, einschliesslich
des ersten Messfensters. Der finale 30-Sekunden-Abschnitt hat dagegen
33,174 -> 39,577 ms Maximum. Keine allgemeine Verbesserung aller Spikes
behauptet. In dieser stehenden Szene tritt kein fortlaufender FPS-Kollaps auf;
auch die Baseline hatte hier keinen solchen Kollaps.

Beide Starts haben 4375 Sections, 625 vorbereitete Spalten, 43953 gespeicherte
Props, 575 Propbatches und 62 replizierte Tiere. Keine reduzierte Sichtweite,
Vegetationsdichte, Wasser- oder Schattenqualitaet. Die KI bleibt aktiv;
Grasfressen und Pflanzenwachstum erzeugen im Verlauf reale, begrenzte Edits.
Dies ist kein vollstaendig unveraenderliches Standbild.

- Mesh-Assets: vorher 6356 -> 6360, nachher 6484 -> 6487. Die initiale Differenz
  von 128 sind acht gemeinsame Modellteile fuer 16 Pferdefelle, keine neue
  Geometrie pro gespawntem Tier. CPU-residente Meshvertices im ersten Fenster:
  294355 -> 391147. Terrain-Vertexdaten werden weiterhin nach Upload freigegeben.
- StandardMaterial-Assets bleiben in beiden Laeufen bei 224. Nachher 10835 ->
  10685 lebende Entities statt einer fortlaufenden Vermehrung. Dies zaehlt
  lebende Entities, nicht die 16384 reservierten Entity-Indizes.
- Am letzten Checkpoint jeweils keine ausstehenden Generierungs-/Terrainjobs,
  keine wartenden Terrainspalten und keine aktiven/queued Wasserzellen. Diese
  trockene Testkamera belegt keine aktive Wasser-/Wasserfall-Langzeitparitaet.
- Erste Terrain-Komposition ueber 2490 Jobs: 2,446 -> 2,944 s aufsummierte
  Workerzeit (+20,4 %), laengster Job 15,923 -> 26,605 ms. Das ist ein
  nachgewiesener Mehraufwand, keine behauptete kostenlose Geometrieverbesserung.
  GPU-Uploadspitze 189030 -> 189535 Vertices je Frame; CPU-Uploadscope und
  GPU-Uploaddauer sind nicht dieselbe Messgroesse.
- Gemessener DRM-VRAM-Peak: 4167,74 -> 4190,73 MiB. Der spaete aktive Bestand
  ist ebenfalls wichtig: vorher 3911,93 MiB, nachher 4190,73 MiB (rund 279 MiB
  mehr). Nachher steigt er im Lauf von 4116,35 auf 4190,73 MiB. Keine VRAM-
  oder allgemeine Speicherparitaet behauptet; Assetzahlen allein beweisen keine
  konstanten internen GPU-Allocator-Kapazitaeten. In beiden Laeufen kein Swap.
- Vorhandene PBR-Submissionzaehler melden an Start/Ende jeweils 103 indirekte
  API-Aufrufe, UI 20/1/0. Das sind nicht saemtliche privaten UI-/Fullscreen-
  Draws und nicht die variable GPU-Subdrawzahl eines Multidraw-Aufrufs.
- Letzter GPU-Opaque-Pass nachher 2,213 ms, Early-Prepass 1,082 ms, UI 0,075 ms.
  Dies sind einzelne letzte Passsamples, weder Intervallmittel noch eine
  aufsummierte komplette GPU-Framezeit. Vollstaendige Triangle-/Bufferbyte-
  Captures, Main-/Renderthread-Gesamtzeiten und GPU-Uploadzeiten fehlen.

Dieser Lauf isoliert nicht allein den Shader: UI-Takt und neue Tierbibliothek
sind Teil des Builds. Andere Seeds, bewegte Regen-/Schneestrecken, viele
Spielerbauten, 30-Minuten-Laeufe und andere GPUs wurden in Phase 95 nicht
erneut als Performance-Messpaar ausgefuehrt. Vorhandene historische Wetter-
und Streamingberichte bleiben eigenstaendige Nachweise.
Nach diesem Lauf wurden die Testkameras/-posen, das im Messlauf geschlossene
Inventar-Kategorieraster und die punktuelle Bett-Auflagepruefung oberer/
zusammengefuegter Halbplatten nachgebessert. Im Messlauf war kein Bett aktiv;
Terrain-/Shader-/KI-Implementierung und Grafikoptionen blieben unveraendert.

## Integrationspunkte

- `terra-core`: `construction.rs`, `seasons.rs`, `config/block_details.rs`,
  Grafik-/Clientkonfiguration und `creatures/horses.rs`.
- `terra-world`: zustandsabhaengiges Targeting, gemeinsame Interaktionsprofile,
  Objektmetadaten sowie vorhandene Herdenerzeugung/Beduerfnisse und
  `creatures/horses.rs` fuer Hindernisspruenge.
- `terra-network`: `server_validation.rs`, `server_furniture.rs`, gemeinsame
  Command-Grammatik, Chat-/Konsolenausfuehrung und Protokollversion 53.
- `terra-render`: `blocks/cube.rs`, `blocks/surface.rs`,
  `meshing/material_edges.rs`, `meshing/tree_details.rs`, Renderrevision/LOD,
  `weather/surfaces.rs`, `assets/shaders/terra_wet_surface.wgsl` und das neue
  geteilte `creatures/model/horse.rs`.
- `terra-ui`/`src-ui`: Katalogbezeichner, Kategorie-Grid, `cadence.rs`,
  Optionsentwuerfe/-reihen, Uhr, Kartenmarker und Ziel-HUD. Kein UI-Ersatzframework.
- `apps/client`: native Material-/Pferde-/Inventar-/Menue-Testtreiber,
  Huf-Foley-Zuordnung und diagnostische Pferdeattribute.
- Inhalte: `assets/terra/mobs/horse.json`; das Sandzaunrezept
  `assets/crafting/fine_sand_fence.json` ist entfernt. Keine bestehenden
  Blockdefinitionen oder Welt-IDs wurden umnummeriert.

## Ansichten Und Reproduktion

- [Materialgalerie](materials/materials-reference-terrain.png)
- [Treppen Und Platten](materials/materials-construction-orientations.png)
- [Pferde Und Individuelle Werte](horses/horses-1.png)
- [Natuerliche Waldansicht](seasons/season-0-spring.png)
- [Zentrierte 4K-Kategorien](inventory-4k-final/inventory-open.png)
- [UI FPS Im Korrigierten Menue](menu-final/graphics.png)

Die verwendeten TOMLs ohne Karten-Caches liegen unter `configs-benchmark/`
und `configs-gallery/`. Beispiel fuer die Materialgalerie, vom Repositoryroot:

```bash
TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/construction-materials-horses-2026-10-21/configs-gallery" \
TERRA_SCREENSHOT=/tmp/teramic-materials.png \
target/debug/terra-client --smoke-test-materials
```

Im normalen Spiel: `/season set winter` und `/spawn teramic::horse` mit den
bestehenden Environment-/Spawnrechten. Fuer Multiplayer beide Programme auf
Protokoll 53 aktualisieren. Eigene Pferdegeraeusche, Reiten/Zaehmung/Zucht,
gemischte Plattenmaterialien, vollstaendige Materialtexturmischung und eine
exakte Referenz-Art-Abnahme bleiben die oben genannten konkreten Grenzen.
