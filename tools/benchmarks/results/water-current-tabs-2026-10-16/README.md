# Wasser, Stroemung, Bildtabs Und Pflanzen - 16.10.2026

Phase 29 verwendet weiter dieselben Crates, TiltUI, BlockRegistry, Section-
Batches, Physik und das vorhandene EditJournal. Keine Dependency-Upgrades.
Weltformat 1, Generatorversion 5, Seed und bestehende Block-IDs bleiben erhalten.
Die angehaengte ID 27 und die neuen Gameplayvertraege benoetigen gemeinsam
aktualisierte Clients/Server mit Netzwerkprotokoll **14**.

## Wasserblock Und Stroemung

`teramic::solid_water` (14) bleibt ein animierter, kollidierender Dekorationsblock.
`teramic::fluid_water` (27) ist eine platzierbare, anwaehlbare und wieder
entfernbare Fluessigkeitsquelle. Ihre logische Zelle liegt im bestehenden
Blockjournal; ein voller `FluidCell`-Eintrag mit Sourceflag liegt im selben
SectionDelta. Nachbarwasser bleibt endlich und erzeugt keine weiteren Quellen.
Speichern, Reload, Replikation, Inventarverbrauch und Abbau nutzen vorhandene
Pfade, keine zweite Wasserwelt und keine separate GLB-Geometrie.

Terrainmeshing behandelt Quelle 27 als leer und laesst Nachbarflaechen sichtbar;
das bestehende asynchrone Fluessigkeitsmesh zeigt Quelle und Abfluss gemeinsam.
Collider, freie Koerperbereiche, Droprays und Einsammelpfade respektieren den
nichtsoliden Quellanker. Solid Water und Glas bleiben dagegen kollidierend.
Die Quellenmenge/Fuellstaende wecken dieselbe begrenzte 10-Hz-Simulation wie
ein abgebautes Flussufer. Entfernen der Quelle beseitigt den unendlichen Nachschub;
bereits ausgelaufenes Wasser bleibt endlich, statt sofort zu verschwinden.

`water::fluid_motion_at` prueft die tatsaechlich geladene Fluessigkeitsbelegung,
Fuellhoehe, feste Barrieren und die lokale Richtung. Meer/stehendes Wasser hat
keine erfundene Stroemung. Fluesse verwenden die deterministische Flussrichtung,
Zellwasser den gespeicherten Transfervektor. Volle Referenzgeschwindigkeit ist
2.4 m/s, davon wirken **40 Prozent** mit Eintauchgewichtung auf den Koerper.
Fallwasser liefert zusaetzlich eine begrenzte Abwaertskomponente.

Die sichtbare natuerliche Fallgardine liegt oberhalb des unteren Wasserpegels.
Ihr Kontakt wird separat anhand derselben gemeinsamen Nachbarkante und
Projektionshuelle wie `channels::waterfall` erkannt, nicht als unsichtbar voller
Wasserblock im gesamten Tal. Feste Zellen bleiben trocken.

Spieler verwenden diese Stroemung im gemeinsamen CharacterSimulation-Pfad von
Prediction und Server, nach dem aktiven Schwimmantrieb. Gegenbewegung und
Springen bleiben moeglich. Dynamische Avian-Bodies erhalten zeitstabilen Drag
vor dem Physikschritt, mit Fuss-/Mittelpunktproben ihrer ColliderAabb und
Aufwecken schlafender Koerper. Static Bodies und der bereits behandelte
CharacterBody sind ausgeschlossen. Lose 3D-Items nutzen dieselbe Driftformel
in ihrer bestehenden begrenzten Simulation, ohne Journalclone pro Item.

Wenn aktives Zellwasser Pflanzen beruehrt, werden nur betroffene Prop-IDs im
bestehenden Entfernungsjournal entfernt. Kontaktpruefungen beschraenken sich auf
die im Schritt beruehrten Wasserzellen und nahe Ownersections, einschliesslich
negativer und vertikaler Sectiongrenzen. Es wird nicht laufend die gesamte
Pflanzenwelt abgesucht. Nur Prop-Epochen aendern sich; benachbarte Pflanzen,
Terrainmeshes und Collider bleiben erhalten. Ueberflutung ist automatische
Zerstoerung wie entfernter Untergrund, keine zusaetzliche manuelle Ernte.

## Wasserfallgelaende Und Darstellung

Die vorherige breite plane Steilwand entstand, weil der mindestens fuenf Meter
springende hydraulische Pegel ueber die volle bis zu 64 m breite Talzone auch
die Gelaendehoehe und roof_blend steuerte. Jetzt bleibt der Sprung am nassen
Kanal, waehrend das aeussere Tal eine kontinuierliche, seitlich versetzte
Erosionsrampe verwendet. Dach-/Uferuebergaenge richten sich nach diesem
kontinuierlichen Profil. Hohe Faelle bleiben erhalten; logische Meterbloecke
werden nicht in Smooth Voxels umgewandelt. Uneditiertes Grundterrain von
`rivers = true` kann sich dadurch aendern. Gespeicherte Edits bleiben an ihren
Koordinaten, bitidentische alte Flusslandschaft wird nicht behauptet.

Vorhandene Fall-/Aufprallschaumkanaele sind sichtbar verstaerkt. Ein gemeinsamer
naher Spruehbatch erzeugt einen leichten Nebel aus global maximal 96 weichen
Quads innerhalb von 48 m. Low-Qualitaet schaltet ihn ab. Es gibt keinen
zusaetzlichen Reflection-/Volumetrikpass und keine Entity pro Spruehtropfen.
Das ist eine begrenzte Spruehnebelapproximation, keine echte Volumetriksimulation.

## Figur, Pflanzen Und Licht

Der Hals ist von 0.14 auf 0.049 m gekuerzt, also um 65 Prozent. Der Kopf sitzt
entsprechend tiefer; Physikkoerper, Spielersteuerung und logische Blickhoehe
bleiben unveraendert. Beide Portraits und geteilte Avatarteile verwenden das
gleiche Modell.

Alle Glasbatchteile, einschliesslich Holzrahmen, erhalten NotShadowCaster.
Diese Eigenschaft wird an tatsaechliche Renderentities gebunden, nicht nur in
eine ungenutzte Materialdefinition geschrieben. Glas bleibt sichtbar und
kollidierend. Die dunkle Farbgradingsektion verwendet Gain 1.2; das ist rund
20 Prozent helleres lineares Schattengrading, keine Zusicherung exakt gleicher
Pixelhelligkeit nach Tonemapping in jeder Szene. Die Sonne wird nicht global
heller gestellt und Bloom nicht erhoeht.

Eiche, Birke und Fichte behalten vorhandene Block-/Baum-IDs. Ein lesender
Adapter der bestehenden TreeSpecies-Daten liefert die tatsaechliche Baumart,
statt sie aus einem geratenen Biom abzuleiten. Unterschiedliche Rindenfarben,
helle gemusterte Birkenrinde und dunklere Fichtennadeln werden im bestehenden
Material-/Meshingpfad dargestellt. Keine GLB-Instanzen pro Holzblock.

Gras verwendet nun quadratische, dreidimensionale, stumpfe Halmsegmente mit
aufrechteren Knicken, sechs kleinen bodennahen Blattgruppen und vereinzelten
weissen Blueten. Drei Silhouetten orientieren sich am Short/Medium/Wide-Motiv
der Referenz. Bestehende Windanimation, Biome-/Zustandstoenung, Prop-IDs,
1.3-fache Darstellung und Section-Batches bleiben erhalten. Das ist reale
prozedurale Geometrie, kein Screenshot der Referenz als Ersatzmodell.
Near hat jetzt maximal 536 statt 476 Vertices je Grasvorlage; Middle/Far bleiben
bei 160/80. Diese zusaetzliche Detailarbeit wird nicht als kostenlos bezeichnet.
Baumarten-Snapshots werden nur fuer Holz-/Laubsections im asynchronen Meshing
erzeugt; sie bringen neue CPU-Arbeit mit, keine Mainthread-Neugenerierung.

## Creative-Kategorien Und Tooltips

Bildtabs liegen ausserhalb und oberhalb des gerahmten Inventarpanels, auch im
kompakten Layout mit reserviertem Platz. Sie zeigen geteilte echte
Itemvorschauen. Aktive Reiter sind hervorgehoben, ihr Titel erscheint beim
Hover. Dieselben TiltUI-Komponenten bilden die UI, keine manuell erzeugte
parallele Bevy-Oberflaeche.

`assets/terra/inventory/categories.json` (Schema 1) enthaelt ID, Name, Icon und
Sortierreihenfolge. Block-JSONs und Eintraege in `assets/terra/props/items.json`
verwenden optional `creative_category`. Die bestehende BlockPalette traegt
diese Daten bis zum UI. Namespaced Custom-IDs werden unterstuetzt; eine Kategorie
ohne expliziten Metadateneintrag wird deterministisch aus ihren echten Items
ergaenzt, statt ihre Items zu verwerfen. Ungueltige doppelte IDs oder nicht
auflosbare deklarierte Icons ergeben einen sichtbaren Ladefehler, keine Fakebilder.
Dies ist ein Kategorien-/Assetvertrag, kein behaupteter vollstaendiger Modloader.

Beispiel eines zusaetzlichen Eintrags im vorhandenen categories-Array:

```json
{"id":"my_pack::masonry","name":"Masonry","icon":{"block":3},"order":70}
```

Ein Block mit `"creative_category":"my_pack::masonry"` erscheint dann dort.
Neue Beschreibungen bleiben im jeweiligen Block-JSON. Der All-Reiter bleibt
als gemeinsamer Einstieg erhalten.

Slottooltips beginnen bevorzugt rechts unten ausserhalb des Slotrechtecks.
Wenn dort kein Platz ist, weichen sie auf freie Seiten aus und bleiben im
Viewport. Die Ursache der alten Ueberdeckung waren layoutneutrale Ghost-Nodes
zwischen Tooltip und Slot: TiltUI konnte den direkten Parent nicht vermessen.
Die eng begrenzte Platzierungsregel benutzt jetzt den tatsaechlichen
Layoutparent/Slot ueber vorhandene ComponentElementIds. Keine weiteren Entities.

## Pruefnachweise

Die finale Workspace-Testsuite hat **548 bestandene Tests**, ein bereits
vorhandener Langzeit-Soak bleibt ignoriert. Zusaetzliche Regressionen pruefen
Quellplatzierung/-entfernung und Deltareload, einzelne geflutete Props samt
negativen/vertikalen Ownergrenzen, 40-Prozent-Drift und aufwachende Avian-Bodies,
reale Fallgardinen, nichtsolides Terrainmeshing, Glas-Schattenmarker,
Baumartenzuordnung, lokale Grasfarben im tatsaechlichen Upload und externe
Inventartabs mit erhaltenem Cursorstapel.

Alle fuenf geforderten Befehle sind erfolgreich, Clippy ohne Warnungen:
[cargo fmt --all --check](logs/terra-phase29-final-fmt.log),
[cargo check --workspace](logs/terra-phase29-final-check.log),
[cargo test --workspace](logs/terra-phase29-final-tests.log),
[cargo clippy --workspace --all-targets -- -D warnings](logs/terra-phase29-final-clippy.log),
[cargo build --workspace](logs/terra-phase29-final-build.log).

Separater `cargo build -p terra-server` und anschliessender Start ohne DISPLAY
und WAYLAND_DISPLAY sind bereits bestanden. Der Smoke beendete nach 27 Sections
mit Exit 0. `cargo tree -p terra-server -e normal` enthaelt weiterhin weder
bevy_render/bevy_ui noch TiltUI, bevy_winit oder wgpu.
[Build](logs/terra-phase29-headless-build.log),
[Start](logs/terra-phase29-headless.log),
[Featuregraph](logs/terra-phase29-headless-features.txt).

Der native Avatar-Smoke hat fuenf echte Aufnahmen abgeschlossen, Weltblinzeln
beobachtet und vier bewegte Gliedmassen geprueft. Lokaler und entfernter
Fixture-Avatar teilen sechs Koerpermeshes und ein Lidmesh. Das ist eine
Rendererfixture, kein weiterer realer Zwei-Client-Netzwerktest.
Root hat das neue offene Portrait selbst betrachtet: der Hals ist deutlich
kuerzer, das Gesicht weiterhin sichtbar. Beide Portraitkameras werden nach
GPU-Readback entfernt. [Avatarlog](logs/terra-phase29-avatar.log),
[offen](captures/terra-phase29-avatar-portrait.png),
[geschlossen](captures/terra-phase29-avatar-portrait-closed.png),
[Weltansicht](captures/terra-phase29-avatar-idle.png).

Creative und Survival bestanden die finale native UI-Abnahme jeweils mit Exit 0
und warnungsfreien Logs. Echte Wayland-/uinput-Mausereignisse prueften die
externen Bildreiter Wood -> All, nur beim Hover sichtbare Titel, Tooltiprechtecke
ohne Slotueberdeckung und kompakte/breite Viewports. Ein gehaltener 128er-
Cursorstapel blieb beim Klick auf den externen All-Reiter erhalten; Tab/Shell
gelten nicht als Aussenklick. Danach bestanden die vorhandenen Aufnehmen-/Split-/
Verteilen-/Sammel-/Outside-Gesten weiter: 65 im Inventar plus 63 als replizierter
WorldDrop, ohne Verlust. Survival ist hier Mode-/Layoutabnahme, nicht durch
kuenstliche Testitems vorgetaeuschte Ernte. UI-Agent betrachtete alle fuenf
Bilder, Root zusaetzlich die breite Creativeaufnahme.
[Creativelog](logs/terra-phase29-inventory-native-final.log),
[Survivallog](logs/terra-phase29-inventory-survival-native-final.log),
[Creative breit](captures/inventory-creative-wide.png),
[Creative kompakt](captures/inventory-creative-compact.png),
[Kategoriehover](captures/inventory-category-hover.png),
[Survival breit](captures/inventory-survival-wide.png),
[Survival kompakt](captures/inventory-survival-compact.png).

Die erste native UI-Runde entdeckte zwei echte Rendererfehler, trotz zuvor
bestandener CPU-Tests: Nach farbigen Grashalmen angehaengte Blattdetails hatten
keine aufgefuellten Vertexfarben; ausserdem war ein anfangs voellig leeres
Spraymesh als GPU-Asset registriert. Die Appendhelpers sichern nun gleiche
Attributlaengen. Der Spruehbatch erzeugt sein Mesh erst bei einem echten Emitter
mit vollstaendigen Daten und blendet es spaeter nur aus, ohne leere GPU-Uploads.
Kein Bevy-/Dependency-Patch. Beide finalen UI-Laeufe oben bestaetigen das
Ausbleiben der Vertex_Color- und Slab-Allocator-Warnungen.

Die finale botanische Runde bestand mit sieben Aufnahmen und warnungsfreiem
Log. Der Batchupload multipliziert jetzt die lokalen 0.90/1.10-Segmentfarben
mit Material-/Biomfarbe, statt sie wie bisher zu ersetzen. Die Grasaufnahme
wurde von Root und Renderer-Agent betrachtet; Windpaare sind enthalten.
[Botaniklog](logs/terra-phase29-botanical.log),
[Gras](captures/terra-phase29-botanical-grass.png),
[Gras im Wind](captures/terra-phase29-botanical-grass-wind.png),
[Busch](captures/terra-phase29-botanical-bush.png),
[Busch im Wind](captures/terra-phase29-botanical-bush-wind.png).

Der finale native Wasserfalllauf bestand mit acht Aufnahmen, ohne Warnungen
oder Shaderfehler. Er zeigt einen generierten **31.83 m** hohen Fall bei
X43.5/Z1564.5. Die Uebersichtskamera liegt nach acht lokalen Luftproben und
vollstaendiger Sichtstrahlpruefung wirklich ausserhalb des Terrains. Eine
fruehere ungueltige Innenansicht wurde verworfen und nicht archiviert.
Root und Renderer-Agent betrachteten die neue Uebersicht und Nahansicht.
Im Nahbereich waren 54 Spruehquads aktiv, Low reduzierte auf null und High
stellte sie wieder her; Animation und Unterwasser-/Oberflaechenwechsel bestanden.
Die Nahaufnahmen zeigen begrenzten leichten Spruehnebel, keinen dichten
volumetrischen Effekt. Funktions-FPS dieser bewegten Kamerarunde werden nicht
als Benchmark verwendet.
[Flusslog](logs/terra-phase29-waterfall.log),
[gepruefte Aussenuebersicht](captures/river-waterfall.png),
[High nah](captures/river-waterfall-high.png),
[Low nah](captures/river-waterfall-low.png),
[Aufprall](captures/river-fall-base.png).

Die neue native Source27-Abnahme verwendet einen echten integrierten UDP-Server,
CreativeSet/Hotbar und BlockEdit ueber die Produktionsnachrichten. Es werden
keine Wasserzellen oder entfernten Props im Clientjournal vorgetaeuscht.
Block27 bei [10,16,23] und amount8/source=true kamen durch bestaetigte Deltas
an. Nach vier Sekunden waren 105 Nachbarzellen nass, fuenf echte Props einzeln
im Journal entfernt und zwei Wasseroberflaechenmeshes mit 322 Dreiecken vorhanden.
Die Quelle war wirklich anwaehlbar und wurde mit Applied-ACK abgebaut; keine
Sourceflags blieben zurueck. Endlich ausgelaufenes Restwasser bleibt erhalten.
Zwei ScreenshotCaptured-Readbacks und Exit 0, keine Warnungen/Shaderfehler.
Backend-Agent betrachtete beide Bilder, Root zusaetzlich die Flussaufnahme.
Die Fixturekorrekturen betrafen die Startup-Synchronisation, das tatsaechliche
praesentierte Kameraauge, Ziele innerhalb der Blockflaeche und identisches MSAA
der einmaligen Studiokamera, nicht einen simulierten erfolgreichen Backendpfad.
[Sourcelog](logs/terra-phase29-source-native.log),
[fliessende Quelle](captures/source-flow.png),
[entfernte Quelle](captures/source-removed.png).

Die Sourcefixture nutzt eine isolierte flache wasseraktivierte Testwelt, keine
Produktionswelt. `world.water = false` bleibt der vorhandene globale Ausschalter.
Ihre drei Eingabedateien sind unter `source-configs/` archiviert. Wiederholung
nach dem Build mit freier GPU und ohne parallelen Client:

```bash
env -u DISPLAY TERRA_CONFIG_DIR=tools/benchmarks/results/water-current-tabs-2026-10-16/source-configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra-source-menu \
  TERRA_CAPTURE_DIR=/tmp/terra-source-captures \
  target/debug/terra-client --smoke-test-fluid-source
```

Kein neuer nativer Zwei-Client-Lauf in Phase 29; die Workspace-Tests enthalten
reale lokale UDP-Pruefungen, die Phase-28-GPU-Zweiclientabnahme bleibt historisch.

## Isolierter Performancevergleich

Verglichen wird die gueltige Phase-28-Nachherbaseline, kein verworfener
Binary-/UI-Mix. Identische archivierte `configs/`, Seed 42042, Sichtweite 16/3,
LOD 96/160, Wasser High, VSync aus, 1920x1200 physische Pixel bei Wayland 150%,
RX 6950 XT/RADV Mesa 26.2.4 Vulkan. Kamera (-622,53.88851,680), Flag
`--smoke-test-forest`, tatsaechlich bewachsener Mushroom Park. Beide offline,
stationaer, ohne Controller-/Netzwerksimulation; je 18 Sekunden. Keine weiteren
von uns gestarteten Cargo-/GPU-Prozesse liefen waehrend des neuen Messfensters.
Root hat die Nachheraufnahme betrachtet.

| Kennwert | Phase 28 | Phase 29 |
| --- | ---: | ---: |
| Frames / mittlere FPS | 2778 / 154.289 | 2793 / 155.193 |
| p95 / p99 CPU-Wall-Framezeit, ms | 7.846 / 9.090 | 7.557 / 8.708 |
| Erster Readyzeitpunkt inklusive Rendering, s | 3.014 | 2.969 |
| Geladene / gerenderte Sections | 7807 / 4115 | 7807 / 4115 |
| Meshassets / sichtbare Meshentities | 9636 / 4546 | 9636 / 4546 |
| Repraesentierte Props / Batches | 92789 / 1306 | 92789 / 1306 |
| Terrain-Meshing, Worker-Spannensumme, s | 0.630 | 0.732 |
| Prop-Meshing, Worker-Spannensumme, s | 0.311 | 0.305 |
| Prozess-RSS am Ende, KiB | 934600 | 908044 |
| Peak-RSS ueber getrusage, KiB | 931384 | 922988 |
| GPU-Opaque-Pass Mittel / p95 / p99, ms | 1.205 / 1.307 / 1.759 | 1.193 / 1.217 / 1.482 |

Das einzelne Paar zeigt rund 0.6 Prozent mehr mittlere FPS, keinen statistisch
abgesicherten Speedup. Die Terrain-Workerarbeit steigt dagegen um rund
16 Prozent, insbesondere durch echte Baumarten-Snapshots; dies ist eine reale
CPU-Mehrarbeit trotz stabilem Framebild. Worker-Spannensummen sind keine serielle
Ladezeit. Kein erneuter reiner 4375-Section-CPU-Benchmark, kein Wasserfall-/
Stroemungs-Worst-Case und keine Streaming-/Bewegungsabnahme. RSS stammt aus
verschiedenen Linux-Messwegen und wird unverrechnet angegeben. GPU-Passzeit
ist keine vollstaendige GPU-Framezeit, exakte Draw Calls bleiben offen.
Propzahlen bezeichnen in Batches repraesentierte, nicht ausschliesslich gerade
sichtbare Einzelobjekte.
[Vorherlog](../interaction-camera-drops-2026-10-16/logs/terra-interaction-v2-forest-after.log),
[Nachherlog](logs/terra-phase29-forest-after.log),
[Nachheraufnahme](captures/terra-phase29-forest-after.png).

```bash
env -u DISPLAY TERRA_CONFIG_DIR=tools/benchmarks/results/water-current-tabs-2026-10-16/configs \
  TERRA_BENCH_SECONDS=18 TERRA_RENDER_DIAGNOSTICS=1 \
  TERRA_SCREENSHOT=/tmp/terra-phase29-forest-after.png \
  python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-forest
```
