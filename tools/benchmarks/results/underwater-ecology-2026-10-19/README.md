# Phase 78: Unterwasser-Riffe Und Aquatische Tiere

## Architektur Und Kompatibilitaet

Erweiterung der vorhandenen Block-, Prop-, Tier-, Interaktions-, Audio- und
Regionssysteme. Keine neuen Dependencies, kein UI-/Enginewechsel, keine neue
logische Voxelaufloesung oder konkurrierende Speicherung. Seed, Welt-IDs und
Generatorversion 5 bleiben erhalten; bestehende Landvegetation bleibt erhalten.

Append-only Block-IDs: 44 CoralRed, 45 CoralBlue, 46 CoralYellow,
47 CoralPurple, 48 Mud. Alte IDs bleiben unveraendert. Alle fuenf besitzen
einzelne JSONs in `assets/terra/blocks/`, Beschreibungen, Mechanikwerte und
eigene matte 512px-Miptexturen in der bestehenden Cube-Pipeline, keine GLBs.
Die vier Korallen besitzen unterschiedliche geschlossene Einwaertsreliefs;
Nachbargrenzen und volle logische Kubus-Bounds bleiben erhalten. Diese Bloecke
sind platzierbar; natuerliche Riffverteilung verwendet die neuen Props, keinen
heimlichen Austausch gespeicherten Unterwasserterrains durch Korallenbloecke.

`ResourcePropKind` und `PropKind` erhalten dieselben sieben angehaengten
Arten. Gemeinsame ALL-Listen und Konversionen ersetzen wiederholte alte
Sieben-Arten-Matches. Neue Mobarten sind ebenfalls angehaengt. REG9 und PLY3
bleiben erhalten und neue Leser verstehen alte Arten/Inventare. Ein Downgrade
mit neuen Arten/Items in einem alten Binary wird nicht versprochen.
Netzwerkprotokoll jetzt **43**, da alte Clients neue Enumvarianten nicht
dekodieren koennen. Client und Dedicated Server gemeinsam aktualisieren.

## Pflanzen Und Bodenbewuchs

Seegras, Seegurken, verzweigte Korallen, Roehrenkorallen, Tellerkorallen,
Seeanemonen und kleine Steingruppen besitzen einzelne Prop-JSONs. Die vorhandene
asynchrone Sectiongenerierung verwendet weiterhin denselben Ground-Prop-Pfad.
Stabile IDs stammen aus Seed und Supportkoordinaten, nicht aus Laufzeit-Zufall.

Die Verteilung benutzt zusammenhaengende Riffpatches, Tiefe, natuerlichen
Untergrund und Biomtemperatur. Korallen bevorzugen warme flache Meeresbereiche;
Seegras und Steine kommen auch im Suesswasser vor. Pflanzen bleiben am Boden,
ihre gesamte Geometrie muss unter die Wasseroberflaeche passen. Es entstehen
keine kuenstlichen Hoehlenseen oder neuen vertikalen Wasserintervalle.
Lichtvertraeglichkeit wird ueber Tiefe/offenes Oberflaechenwasser angenaehert,
nicht ueber einen neu erfundenen Voxel-Light-Floodfill.

Gegenueber der ersten nativen Ansicht wurden glatte Flaechen verfeinert:
Roehren haben gestufte blaue Waende und echte offene Bohrungen, verzweigte
Korallen besitzen feinere Gabelungen/Noppen, Teller gelbe unregelmaessige
Raender, Seegurken gestufte Koerper und Anemonen dunkle Steinsockel.
Deterministische Vertexfarben variieren die Flaechen ohne neue Materialien.
Budget je Art/Variante: Near maximal 2400 Vertices, Middle 800, Far 160;
die detailreichste Roehrenvariante liegt bei 2376 Vertices.

Alle Details gehen in bestehende Section-Batches. Keine Entity pro Pflanze
oder Microvoxel. Vorlagen/Handles werden wiederverwendet. Picking baut jetzt
nur wirklich benoetigte Art-/Varianten-Geometrie einmalig auf statt vor dem
ersten Treffer alle Artenvarianten vorzurechnen.

Unterwasserpflanzen sind ausdruecklich wasservertraeglich, ebenso bestehende
Schilfpflanzen; Landpflanzen werden von Wasser weiterhin zerstoert. Seegras
und Anemonen schwingen sanft mit einer gemeinsamen shader-/CPU-identischen
Wasserbewegung, unabhaengig von atmosphaerischen Sturmboeen. Das ist kosmetische
Stroemungsanmutung, keine Hydrodynamik fuer jedes einzelne Blatt.

Auswahl/Abbau verwenden dieselben echten Dreiecke wie der Renderer. Der
gemeinsame Planting-Picker erreicht den Boden auch durch Wasserquellen;
andere Props und feste Bloecke bleiben Hindernisse. Der normale Picker behaelt
Wasserquellen zum Ersetzen, findet aber auch davor liegende Pflanzendreiecke.
Autoritaet, Inventarverbrauch, genau ein Drop und entfernte Props nach Reload
verwenden das bestehende Journal und REG9. Keine parallelen Tombstones.

## Aquatische Tiere

Eigene Mob-JSONs: Squid, Moon Jelly, Fire Jelly, Void Jelly, Sun Jelly,
Crystal Jelly, Reef Shark, Hammerhead Shark, Whale Shark.
Individuelle Groessen werden stabil aus der Tier-ID innerhalb der gewuenschten
Spannen abgeleitet; Render-, Treffer- und Bewegungsabmessungen stimmen ueberein.

| Art | Laenge | Breite/Hoehe | Verhalten |
| --- | --- | --- | --- |
| Squid | 2.00 m | 0.95 m | Schwimmen, Festhalten, Flucht/Tinte |
| Fuenf Quallen | Hoehe 0.35-0.95 m | Breite/Tiefe 0.15-0.35 m | Schwimmen, gelegentlich Bodenruhe |
| Reef Shark | 1.35-2.75 m | 1.10-1.45 m | Spielerbiss, Fischjagd |
| Hammerhead Shark | 1.25-2.15 m | 0.85-1.05 m | Freundlich zum Spieler, Fischjagd |
| Whale Shark | 2.25-4.00 m | 1.45-2.35 m | Auf ausdruecklichen Wunsch feindlich, Fischjagd |

Haie und Quallen spawnen nur im Meer, Squids dort haeufiger und selten in
Oberflaechenseen. Bestehende gespeicherte Fische behalten Identitaet/Art;
neue Arten werden bei neuen Habitatkandidaten gewaehlt. Die Meeresbindung
betrifft Spawns, nicht eine unsichtbare Wand an jeder Biomgrenze.
Bestehende lokale Spieler-/Populations- und KI-Budgets gelten weiter.

Gemeinsame geladene Voxelhabitate pruefen Wasser, Wandfreiheit und unbekannte
Bereiche. Gesweepte lokale Kurse vermeiden Ufer statt Drehschleifen, ersetzen
aber keine globale Wegplanung. Fischfang/Bisse werden serverseitig einmalig
ueber den bestehenden Effektpfad verarbeitet, einschliesslich Clownfischen.

Neun geteilte Modelle mit wenigen animierten Meshgruppen: acht Squidarme,
kontrahierende verschiedenfarbige Quallenschirme und Tentakel, gegliederte
Haikoerper, Flossen, Schwanz und Biss. Quallenmaterial teilweise transparent,
ohne Schattenwurf. Keine Atemblasen fuer diese aquatischen Arten.
Die Umsetzung folgt der Referenz, ist keine pixelidentische 1:1-Kopie.

Vier leise Bewegungs-/Jet-/Puls-/Tinten-Foleyvarianten nutzen die bestehende
CC0-Wasseraufnahme und den raeumlichen Audiomixer. Keine erfundenen Hai-Laute,
keine neue Audiodependency; Quellen und Bearbeitung in `assets/audio/CREDITS.md`.
Eine menschliche Hoerabnahme ist nicht behauptet.

## Unterwasser-Optik Und Tinte

Klarere Nahsicht, tiefenabhaengiger Nebel, bestehende Biome-Wasserfarben und
dezenteres Grading erhalten die erkennbaren Korallenfarben. Vorbereitete
Fluiddaten und ein kleiner Cache ersetzen pro-Frame-Noise-/Wassersaeulensuchen.
Ruhend maximal 5 Hz, bei Zell-/Wasser-/Weltwechsel unmittelbare Invalidierung;
eine Abfrage ist auf 64 vertikale Zellen begrenzt.

Medium/High erweitern den vorhandenen Cave-Postprocessingpass um sehr kleine
Brechung, weltverankerte dezente Caustics und Tiefenabsorption. Low behaelt
Fog/Grading ohne diesen optionalen Optikpass. Die vorhandene Horizon-Shell
schliesst unter Wasser den schwarzen oberen Hintergrund; identische lineare
Fog-/Biomefarben vermeiden einen harten andersfarbigen Streifen.
Keine neue Fullscreen-Pipeline, keine Aenderung trockener Hoehlenwahrnehmung.

Squid-Tinte wird bei einer replizierten Ink-Verhaltensflanke gegen den echten
untergetauchten Spielerkopf geprueft: 2.5 m Radius, geladene Fluidzelle,
2 s lokaler Cooldown. Schwarze organische Streifen laufen 4.2 s nach unten ab.
Low und High beeintraechtigen die Sicht gleich; beim Auftauchen oder
Sessionwechsel wird sauber zurueckgesetzt. Der serverseitige Bedrohungspfad
betrachtet derzeit Survival-Spieler, Creative loest keine Angstreaktion aus.
Die Trefferpruefung ist koerperbasiert, nicht an die Drittpersonkamera gebunden.

Die native Sichtpruefung deckte einen zusaetzlichen Farbraumfehler auf:
bereits lineare Wasser-Streuung wurde nochmals als sRGB dekodiert und fiel
im Tonemapping nahezu schwarz aus. Fog, Horizon und Postpass erhalten jetzt
dieselbe lineare Farbe. Der trockene Oberflaechenwert bleibt numerisch gleich.

## Karte, Licht Und Wolken

Die Ursache des Cave-Panfehlers war `drag=None` in `enforce_layer` auf jedem
Hoehlenframe. Nur echte Ebenen-/3D-Wechsel setzen jetzt den Drag zurueck.
Abgelehnte 3D-Kommandos in Cave unterbrechen keinen aktiven Drag. TiltUI setzt
echte `InteractionDisabled`-Zustaende, zusaetzlich bleiben Hintergrund und Icon
des gesperrten 3D-Buttons auch beim Hover grau.

Fackel-Umgebungslicht steigt ab 4 m um 20%, mit sanftem Uebergang. Die direkte
Hotspotzone bis 1 m bleibt unveraendert. Derselbe Soft-Core-Pfad gilt fuer
gesetzte, gehaltene und entfernte Fackeln; keine zusaetzlichen Lichter.

Wolkenbanken besitzen deterministisch verschiedene Hoehen. Reale Vertices
bleiben einschliesslich maximalem Sturmwachstum zwischen Y200 und Y312.
Beim Aufklaren koennen High/Ultra tagsueber Strahlen durch echte Bankluecken
zeigen: maximal ein zusaetzlicher Batch mit 72 Vertices, weich ausblendend.
Das ist eine tiefengepruefte Weltgeometrie-Streuungsapproximation, kein echtes
volumetrisches Raymarching. Nachts, unter Wasser, in Hoehlen und Low/Medium
ist der Effekt deaktiviert; es gibt keinen neuen Fullscreen-Pass.

## Native Validierung

Linux/Wayland, Vulkan RADV, RX 6950 XT, Mesa 26.2.4. Reale Fenster und
GPU-Screenshots, kein HTML-Mockup und keine behauptete Playwright-Pruefung
eines nativen Bevy-Clients.

- `--smoke-test-reef`: 14 Bilder, 29.613 s, Exit 0. Echter generierter
  Meeresboden bei (1224.5, 0, 328.5), Wasserspiegel 8.0. 5381 natuerliche
  Wasserprops im geladenen Bereich, sieben explizite Fixture-Props und fuenf
  Testbloecke. Zusammen mit Land-/Randbewuchs 7294 dargestellte Props in 378
  Batches. High-/Low-Tinte, Abklingen und Auftauchreset bestanden. Peak-RSS
  857564 KiB, Peak-VRAM 2299016 KiB. Keine ERROR-/WARN-/WGSL-/Slab-Meldungen.
  Nach dem Farbraumfix wurden Fernfarbe, Nahkorallen und Tiefenwirkung erneut
  visuell geprueft. Explizite Fixtures sind kein Naturverteilungsnachweis fuer
  sieben Arten an derselben Stelle.
- `TERRA_AQUATIC_INSPECT=1 --smoke-test-creatures`: 13 Ansichten, 32.882 s,
  Exit 0, alle neun Modelle und Hold/Ink/Rest/Bite-Posen; echte geteilte
  Renderassets. Diese Galerie steht auf einer Testwiese und prueft nicht
  Schwimm-KI im Meer. Keine ERROR-/WARN-/WGSL-/Slab-Meldungen. Squid,
  Reef Shark und Sun Jelly wurden visuell kontrolliert.
- `--singleplayer --smoke-test-minimap`: 52 Stufen, 169.450 s, Exit 0.
  Realer integrierter Server und UDP-Testpeer, Mob-/Spielermarker, Cave-
  Teleport, mehrframeiger Cave-Drag trotz abgelehntem 3D-Command, echte
  `InteractionDisabled`-/Tilt-Disabled-Zustaende, Welt-/Waypoint-CRUD und
  Boostflug bestanden. Cave-Ansicht mit grauem 3D-Icon visuell kontrolliert.
  Drei Wayland-Cursor-Warp-Fehler stammen vom synthetischen Testzeiger;
  keine Slab-/WGSL-Fehler. Ein vorheriger Lauf wurde gezielt beendet:
  sein Drag-Test startete im noch offenen Wegpunktdialog. Der korrigierte
  Fixture wartet auf dessen Schliessen und misst ab dem Mausdruck.
- `--smoke-test-weather`: 54 Stufen, 131.509 s, Exit 0. Regen, fuenf Schneestufen, fuenf Wolkenlagen,
  Tag/Nacht, Blitze, Ufer, Unterwasser-Regen, Aufklaren und Low-Budgets
  bestanden. Die neue Wolkenhoehe ist zusaetzlich fuer alle Meshvertices
  einschliesslich maximalem Sturmwachstum getestet. Strahlen-Counter bei
  aufklarender High-Sicht 1 Batch / 72 Vertices, bei Low 0 / 0. Die Kamera
  der Strahlenansicht wurde fuer den Bildnachweis gesondert gegen den Horizont
  gerichtet. Sichtbare dezente Strahlen und deren Abschaltung visuell
  kontrolliert; keine ERROR-/WARN-/Slab-/WGSL-Meldungen. Diese Pruefung fand
  einen Premultiplikationsfehler im eigenen Additivfragment: RGB wird jetzt
  wie in Bevys PBR-Add-Pfad mit dem Sichtbarkeitsfaktor multipliziert und
  Output-Alpha auf null gesetzt. Vorher wurde ein zu heller weisser Streifen
  erzeugt; der finale Screenshot belegt die korrigierte dezente Beimischung.
- Dedicated Server mit entfernten `DISPLAY` und `WAYLAND_DISPLAY`,
  `--bind 127.0.0.1:34578 --advertise 127.0.0.1:34578 --no-save --smoke-test`:
  Exit 0, 27 Sections bereit. Normaler Dependency-Baum ohne `terra-render`,
  `terra-ui`, `bevy_render`, `wgpu` und `bevy_audio`. Der bereits laufende
  Benutzer-Server wurde nicht beendet oder veraendert.

## Kontrollierte Messpaare

Ryzen 9 5900X, RX 6950 XT, native 1920x1080, Seed 42042, Generator 5,
Sichtweite 8, vertikal 3, VSync aus, klare pausierte Welt um 10 Uhr,
High-Wasser, keine Chunkanimation. Dieselben Konfigurationen und Kamerapfade
fuer Vorher-/Nachher. Jeweils ein Prozess, keine gleichzeitigen eigenen
Cargo-/Client-Laeufe. Desktopprogramme und ein bereits laufender
Benutzer-Dedicated-Server blieben unveraendert; kein isolierter Laborrechner.

Vorher: gesichertes vorhandenes Phase-77-Binary und separater Asset-Snapshot
mit 43 Bloecken/altem Propkatalog. Nachher: Phase-78-Produktionscode mit 48
Bloecken. Der fruehe Snapshot enthaelt bereits zusaetzliche Mob-JSONs, die
das alte Binary nicht laedt. Diese Offline-Paare simulieren keine Tiere;
sie sind weder ein KI- noch ein Unterwasser-Postprocessing-Stresstest.

| Messwert | Flug vorher (60 s) | Flug nachher (60 s) | Meer vorher (30 s) | Meer nachher (30 s) |
| --- | ---: | ---: | ---: | ---: |
| Durchschnitt FPS | 143.69 | 144.14 | 190.07 | 189.01 |
| p95 Frame, ms | 10.011 | 9.874 | 6.448 | 6.473 |
| p99 Frame, ms | 11.802 | 11.551 | 7.356 | 7.732 |
| Bereitschaft, s | 2.605 | 2.664 | 2.739 | 2.821 |
| Peak-RSS, KiB | 898244 | 903392 | 794608 | 809824 |
| Peak-VRAM, KiB | 3060492 | 3322680 | 1796900 | 1797132 |
| Peak-GTT, KiB | 69780 | 69780 | 69780 | 69780 |
| Geladene Sections (Ende) | 2035 | 2035 | 2667 | 2667 |
| Gerenderte Sections (Ende) | 869 | 869 | 615 | 615 |
| Meshassets (Ende) | 3531 | 3582 | 2766 | 3096 |
| Repraesentierte Props (Ende) | 24583 | 24627 | 0 | 1617 |
| Prop-Batches (Ende) | 320 | 325 | 0 | 284 |
| Terrain-Komposition, Worker-s | 13.487 | 13.333 | 1.062 | 1.076 |
| Prop-Komposition, Worker-s | 1.331 | 1.323 | 0 | 0.022 |
| GPU Opaque-Pass Mittel, ms | 1.261 | 1.299 | 0.442 | 0.450 |
| GPU Transmissive-Pass Mittel, ms | 0.0484 | 0.0484 | 1.572 | 1.566 |
| PBR Draw-API-Aufrufe (letzter Snapshot) | 69 | 69 | 123 | 125 |

Workerzeiten sind aufsummierte parallele Terrain-/Prop-Komposition,
keine reine CPU-Weltgenerierungszeit und kein Mainthread-Stallmass.
Bereitschaft beinhaltet Streaming/Renderbereitschaft und ist nicht die
historische reine 4375-Section-CPU-Messung. Passzeiten ergeben keine gesamte
GPU-Frametime. Draw-API-Aufrufe sind nicht die interne MultiDraw-Subdrawzahl;
letztere bleibt unbekannt. RSS und GPU-Speicher sind Prozess-/Treiber-Peaks,
keine isolierten neuen Mesh-Bytes.

Das Flugpaar ist bei FPS nahe beieinander, braucht aber rund 5 MiB mehr
Peak-RSS und 256 MiB mehr Peak-VRAM. Im Meer kommen 1617 Props hinzu, mit
rund 15 MiB mehr Peak-RSS, 0.56% weniger FPS und schlechterem p99. Diese
Regressionen bleiben sichtbar; aus einzelnen kurzen Messpaaren wird weder
allgemeine Paritaet noch eine generelle Verbesserung abgeleitet. Dichte Riffe,
viele transparente Quallen, lange Reisen und weitere GPUs bleiben offen.

Reproduktion mit `tools/benchmarks/measure.py`, `TERRA_BENCH_SECONDS=60`
und `--smoke-test-lod` beziehungsweise 30 und `--smoke-test-ocean`;
`TERRA_RENDER_DIAGNOSTICS=1`, `TERRA_DRAW_DIAGNOSTICS=1` und
`TERRA_MEASURE_GPU_MEMORY=1`. Konfiguration und Rohlogs sind diesem Bericht
beigelegt. Die benoetigten Vorher-Assets/Binaries liegen lokal unter
`target/phase78/`, nicht als grosse Binaries im Quellarchiv.

## Cargo-Pruefungen

Gezielte feature-gatete World-Pruefung: `cargo test --locked -p terra-world
--features bevy --lib`, **294 bestanden**, einschliesslich sechs Aquatic-KI-
Tests, echter Wasserquellen-Auswahl, Wasservertraeglichkeit, Determinismus
und Geometrie-/LOD-Budgets. Der Standard-World-Test ohne Feature ersetzt
diese Pruefung nicht. Core-/Network-/Storage-Tests decken neue IDs,
REG9-Reload, Propdrops und Replikation aller neun Arten zu zwei realen
UDP-Clients ab.

Finaler Stand mit unveraendertem Lockfile:

- `cargo fmt --all --check`: bestanden.
- `cargo check --locked --workspace`: bestanden.
- `cargo test --locked --workspace`: **1241 bestanden, 0 fehlgeschlagen,
  1 regulaer ignoriert**. Der ignorierte Test ist der separat aufzurufende
  180-s-UDP-Soak, nicht eine uebersprungene Aquatic-Pruefung. Darunter
  322 Render-, 229 UI-, 294 World- und 105 Netzwerkpruefungen.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: bestanden.
- `cargo build --locked --workspace`: bestanden, Client und Dedicated Server.

Anfaengliche Fehler betrafen alte hartcodierte Prop-/Kataloganzahlen,
einen lokalisierten UI-Namen und eine Wolkenhoehenprobe mit ausschliesslich
niedrigen Seedbits. Korrigiert wurden Registry-/JSON-basierte vollstaendige
Katalogtests sowie ein separater Hoehenhash mit erhaltener Variationspruefung.
Die neue Additiv-Premultiplikation ist ebenfalls als Regression geprueft.

## Bilder Und Grenzen

[Riffansicht](captures/reef-08-reef-wide.png),
[Korallenbloecke und Schlamm](captures/reef-07-reef-blocks.png),
[Tinten-Treffer](captures/reef-11-squid-ink-hit.png),
[abgeklungene Tinte](captures/reef-12-squid-ink-cleared.png),
[Squid-Modell](captures/creatures-0.png),
[Reef Shark](captures/creatures-6.png),
[Cave-Karte mit deaktiviertem 3D](captures/map-20.png),
[dezente Strahlen beim Aufklaren](captures/clearing-cloud-gaps-sunshafts.png).

Die Art-Direction ist eine eigenstaendige stilisierte Annaeherung, keine
pixelidentische Reproduktion. Tier-Picking verwendet orientierte Koerper-
Huellen, nicht animierte Dreiecke fuer jede Tentakelspitze. Mob-JSONs steuern
Titel, Groessen/Minimumgroessen, Werte und Kampfparameter; Spawngewichte und
Biomregeln stehen noch in Rust. Bestehende gespeicherte Fischhabitate werden
nicht nachtraeglich durch neue Arten ersetzt. Globale Navigation, langfristige
Mehrspieler-/GPU-Abnahme, echte artspezifische Tieraufnahmen und menschliche
Hoerabnahme bleiben offen; siehe die entsprechend ergaenzte TODO.md.
