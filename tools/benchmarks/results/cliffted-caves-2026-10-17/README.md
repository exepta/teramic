# Phase 52: Cliffted Desert, Hoehlen Und Renderkosten

Stand: Implementiert; Workspace- und native Funktionsabnahme bestanden.
Weitere Messwerte und die Grenzen der kurzen Abnahme stehen unten.

## Weltvertrag

Neue Welten speichern `geology_revision = 1`. Alte Konfigurationen ohne dieses
Feld lesen weiterhin Revision 0 und behalten ihr bestehendes Grundterrain,
ihre Hoehlen und Baumaterialien. Ein Laden aktiviert keine zerstoerende
Neugenerierung. Generatorversion 5, Seed und bestehende Block-IDs bleiben erhalten.
Der Dedicated-Resolver uebernimmt den gespeicherten Vertrag auch dann, wenn die
globale Konfigurationsdatei das neue Feld noch nicht enthaelt. Protokoll 26
verlangt gemeinsam aktualisierte Clients und Server.

`Cliffted Desert` ist ein felsiger Teilbereich innerhalb starker bestehender
Wuestenregionen, keine unabhaengige Wuesteninsel. Ein normaler Desert-Saum trennt
die neue Klimaregion von fremden Biomen. Sand, tiefere Sandsteinschichten und
kontinuierlich geformte Mesakanten bilden ihre Oberflaeche. Gras-/Wasserfarben
uebernehmen die vorhandene weiche Desert-Palette. Debuganzeige, `/locate biome
cliffted_desert` und Chatvervollstaendigung kennen das neue Biom.

## Hoehlen Und Salz

Die vorhandene Hoehlendichte und ihre verbundenen Tunnel werden erweitert,
nicht durch ein zweites Hoehlensystem ersetzt:

- Cheese: hohe, unregelmaessige Kammern mit Verbindungen zum Tunnelnetz.
- Dripstone: breite Gewoelbe mit waermeren mineralischen Wand-/Bodenbloecken.
- Spaghetti: schmale, verschlungene Gaenge aus dem vorhandenen Noise-Schnittfeld.

Zehn Prozent der geeigneten 192-m-Hoehlenregionen mit Zentrum im Cliffted Desert
erhalten eine breite Salzkammer als Dripstone-Variante. Das ist eine feste
deterministische Chance pro Region, kein erneuter Wurf pro Chunk und kein
Salzanteil des Weltvolumens. Die uebrige Welt erzeugt im neuen Vertrag kein Salz.
Kammergeometrie bleibt am Rand geschlossen und besitzt ein schuetzendes Dach.
Ein begrenzter Cache teilt unveraenderliche regionale Beschreibungen zwischen
vertikalen Jobs. Der Geologietest mit Seed 42042 untersucht 37 249 regionale
Mittelpunkte: 339 sind geeignet, 37 besitzen Salz, also 10.914 Prozent der
geeigneten beziehungsweise 0.0993 Prozent aller untersuchten Regionen. Das ist
eine Seed-Stichprobe, keine garantierte globale Biomflaeche. Eine reale Kammer bei
`[16800, -35, -17952]` besitzt auf der geprueften Mittelachse 113 freie Bloecke.
Rohdaten: [Geologietests](geology-tests.log).

Salz nutzt die bereits berechnete Kammerdichte fuer seine oberflaechennahe
Beschichtung. Der alte dichte 10-m-Salzsuchraster und zusaetzliche Cave-Noise-
Auswertungen entfallen fuer neue Welten. Alte Welten behalten ihre gespeicherte
Verteilung. Spitz zulaufende, facettierte Boden-/Deckendetails sind am tragenden
Voxel befestigt. Geschlossene Felskerne, gemeinsames Section-Batching und das
geteilte praezise Picking bleiben erhalten; keine Entity pro Tropfstein.

## Holz Und Stein

ID 36 ist `teramic::spruce_log`; ID 37 ist `teramic::dripstone`. Vorhandene IDs
0..35 werden nicht verschoben. Neue Fichten tragen das eigene dunkle Rinden-
und Stirnholzmaterial. Das einzelne Rezept `assets/crafting/spruce_planks.json`
verbraucht einen Fichtenstamm und erzeugt vier Fichtenplanken, niemals Eiche oder
Birke als Ersatz. Gespeichertes altes Holz wird nicht willkuerlich umbenannt.

Die geteilte Steintextur verwendet matte, leicht warme Grautoene und gebrochene
Gesteinskoerner statt kuehler, horizontal wirkender Wiederholungen. Erz-Felskerne
teilen weiterhin dasselbe Steinmaterial. Stein nutzt Roughness 0.99 und
Reflectance 0.08. Das neue validierte JSON-Feld `cube_material.reflectance`
besitzt den kompatiblen Default 0.5 und gehoert zum Material-Cache-Schluessel.
Alle Texturen erhalten ihre Mipmaps. Blaues Umgebungs-/Schattenlicht ist weiterhin
sichtbar; das ist keine neue Lichtausbreitung oder neutrale Hoehlenbeleuchtung.

## Performance-Aenderungen

- Vorhandenes Bevy-Zweiphasen-Occlusion-Culling nutzt ab Sichtweite 12 den
  bestehenden Depth-Prepass. Vollstaendig verdeckte Batches muessen keine ganzen
  Vertexmengen mehr transformieren. Kleine Sichtweiten behalten den einphasigen
  Pfad; Settingswechsel schalten die bestehende Kamera um. Kein eigener Renderer
  und keine zusaetzliche Dependency.
- Fernes Gras behaelt alle vier markanten Wurzeln und flachen Spitzen, verwendet
  aber beidseitige schmale Baender: 32 statt 80 Vertices. Nah-/Mittelgeometrie,
  Prop-IDs, aktive Pflanzenanzahl, Wind im Nahbereich und Interaktion bleiben.
- Ferne Prop-Normalen/Farben nutzen die bestehende normalisierte Packung. Das
  regulaere Vertexlayout sinkt von 48 auf 28 Bytes, ohne neue Materialhandles
  pro Pflanze. Wachstum fuehrt seine eigenen Zeit-/Ankerattribute weiter.
- Erzdetails besitzen maximal 52 statt 104 Dreiecke pro freiem Face. Grobe LODs
  behalten die Hauptsilhouette; Salz-/Tropfsteinpunkte konzentrieren sich auf
  geeignete Boden-/Deckenflaechen.

## Messbedingungen

Host: Ryzen 9 5900X, Radeon RX 6950 XT, Vulkan/RADV, Mesa 26.2.4. Die Rohlogs
enthalten tatsaechliche physische Aufloesung, FPS, p95/p99, Ladezeit, RSS,
DRM-VRAM/GTT, Section-/Mesh-/Propzahlen und instrumentierte einzelne GPU-Paesse.
GPU-Passzeiten werden nicht als summierte Gesamt-GPU-Zeit ausgegeben.

Das strenge native Vergleichspaar nutzt dieselben gespeicherten
`configs/client.toml`/`world.toml`: Sichtweite 32, Terrain-LOD 192/320 m vor
bestehender 1.5-Verlaengerung, FOV 65, VSync aus, 60 s Messfenster, Seed 42042,
Generator 5 und alter Geologievertrag. Dadurch werden neue Kammern/Biome nicht
mit Renderoptimierungen vermischt. Die Weltzeit steht fest auf 10 Uhr.
Ein anfaenglicher, nicht zeitfixierter Lauf reproduzierte rund 20 FPS bei
3828x2043 Pixeln und ueber 46 Millionen Vertex-Shader-Aufrufen pro Hauptpass.
Seine Zahlen ersetzen nicht das abschliessende fixe Messpaar.

## Nativer Vorher-/Nachhervergleich

Optimiertes Dev-Profil, physisch 3828x2043 Pixel (Fenster-/Monitorbegrenzung,
kein behauptetes exaktes 3840x2160), dieselbe Kamera und 60 s nach Bereitschaft.
Keine gleichzeitig laufenden Cargo-/Compiler- oder anderen Testprozesse.
Der Vorherbuild nutzt eine eingefrorene Kopie seiner eigenen Assets, der
Nachherbuild die aktuellen Assets. Zwei zusaetzliche Cube-Definitionen werden
geladen, sind aber nicht kuenstlich in die alte Vergleichswelt eingesetzt.

| Messung | Vorher | Nachher |
| --- | ---: | ---: |
| Durchschnittliche FPS | 20.127 | 55.511 |
| p95 Frame, ms | 52.428 | 20.216 |
| p99 Frame, ms | 67.873 | 23.300 |
| Initiale Szenenbereitschaft, s | 54.289 | 42.961 |
| Peak Prozess-RSS, MiB | 2399.90 | 2383.32 |
| Peak DRM-VRAM, GiB | 10.399 | 7.468 |
| Peak DRM-GTT, MiB | 1092.14 | 68.14 |
| Early-Prepass GPU-Mittel, ms | 19.318 | 2.254 |
| Main-Opaque-Pass GPU-Mittel, ms | 20.982 | 4.463 |
| Main-Opaque Vertex-Shader-Aufrufe pro Frame | 46 659 331 | 18 932 422 |
| Geladene Sections | 31 872 | 31 872 |
| Gerenderte Sections | 20 420 | 20 420 |
| Mesh-Assets | 48 970 | 48 970 |
| Aktive / repraesentierte Props | 401 244 / 401 244 | 401 244 / 401 244 |
| Prop-Batches | 4657 | 4657 |
| Wasser-Tiles | 89 | 89 |

FPS +175.8 Prozent, p99 -65.7 Prozent, Peak-VRAM -28.2 Prozent in diesem
Messpaar. Die GPU-Passzeiten sind einzelne instrumentierte Paesse, keine
summierte Gesamt-GPU-Framezeit. PBR-Draw-API-Zaehler waren im Messpaar nicht
aktiviert; keine erfundene Drawzahl oder Gleichsetzung mit GPU-Subdraws.
Alle Props bleiben repraesentiert; Sichtweite, Nah-/Mittelgras und Pflanzendichte
wurden nicht reduziert. Sichtbare ECS-Meshentities unterscheiden sich gering
(15 519 / 15 515) durch neue genaue Boundingboxen der Ferngeometrie.

Rohdaten: [Vorher](before-4k32.log), [Nachher](after-4k32.log).
Der fruehe [unpausierte Lauf](before-4k32-unpaused.log) und ein
[Lauf mit Compilerueberlappung](before-4k32-build-overlap.log) sind bewusst nicht
Teil dieses finalen Paars. 55.5 FPS in dieser extremen Szene sind keine
universelle 60-FPS-Garantie. Eine GPU und ein kurzes Messfenster sind kein
Mehr-GPU-/Langzeitnachweis.

## Kleine Sichtweite Und Cullingbudget

Die erste pauschal aktivierte Occlusion-Fassung lag in der kleineren Szene
bei 147.35 statt zuvor 164.30 FPS, ohne nennenswert weniger Geometrie zu
verarbeiten. Dieser Zwischenstand wurde verworfen, nicht als Verbesserung
verkauft: [Zwischenlauf](after-forest-always-occlusion.log). Ab Sichtweite 12
bleibt die Zweiphasenverarbeitung aktiv, darunter nutzt die bestehende Kamera
wieder den einphasigen Pfad. Der 32er-Messpfad oben bleibt dadurch unveraendert.
Tests pruefen auch den Livewechsel 8 -> 32 -> 8 ohne neue Kameras.

Ein frisches, direkt aufeinanderfolgendes Vergleichspaar mit finalem Build nutzt
dieselben Daylight-Konfigurationen, Seed 42042, alte Geologie, Erze aus,
Sichtweite 8, FOV 65, VSync aus, 1920x1200 physische Pixel und je 60 s.
Hier laeuft die Weltzeit in beiden Builds ab 10 Uhr; das ist bewusst nicht
dieselbe Szene/Qualitaetslast wie im zeitfixierten 32er-Paar.

| Messung | Vorher | Nachher |
| --- | ---: | ---: |
| FPS | 164.535 | 164.596 |
| p95 Frame, ms | 7.395 | 7.510 |
| p99 Frame, ms | 8.492 | 8.741 |
| Initiale Szenenbereitschaft, s | 1.733 | 1.743 |
| Peak RSS, KiB | 699 640 | 689 240 |
| Peak DRM-VRAM, KiB | 2 020 184 | 2 020 120 |
| Sections / gerenderte Sections | 2041 / 875 | 2041 / 875 |
| Mesh-Assets / Props | 2386 / 25 808 | 2386 / 25 808 |

FPS bleiben hier praktisch gleich; p99 ist um 0.249 ms beziehungsweise
2.93 Prozent schlechter. Keine allgemeine Frame-/Speicherparitaet aus diesem
Einzelpaar ableiten. Rohdaten: [Vorher](before-small-final.log),
[Nachher](after-small-final.log). Der aeltere [Baseline-Lauf](before-forest.log)
bleibt erhalten, wird aber nicht mit dem frischen Nachherlauf zu einem anderen
Prozentwert vermischt.

## Bewegung Und Streaming

Der finale Build absolvierte zusaetzlich zwei Hin-/Rueckfahrten ueber 224 m
und negative Sections mit Sichtweite 32, physisch 3828x2043 Pixeln und 60 s
Messfenster. Alle ausstehenden Mesh-Jobs wurden vor erfolgreichem Ende
abgeschlossen, 17 172 LOD-Wechsel ausgefuehrt. Fuenf native GPU-Aufnahmen,
davon Rueckkehransichten 2 und 4 gesichtet, zeigen Terrain, Laub und Props.
Kein beobachteter leerer Frame oder Shaderfehler; Einzelbilder beweisen keine
voellige zeitliche Artefaktfreiheit. Dieser Flug beinhaltet keine 360-Grad-
Drehung und keinen aktivierten Fade-Modus.

Gemessen: 33.686 FPS, p95 38.289 ms, p99 43.553 ms, Peak-RSS 2 763 172 KiB,
Peak-VRAM 9 260 864 KiB. Wieder 31 872 geladene / 20 420 gerenderte Sections,
401 244 repraesentierte Props, 4657 Prop-Batches und 48 970 Mesh-Assets.
Streaming ist damit weiterhin teurer als die ruhende Szene. Ohne passenden
Vorherflug ist das kein behaupteter historischer Bewegungs-Speedup und kein
behobener Langzeit-Frame-Spitzenbefund.

Die zusaetzlich aktivierte PBR-Instrumentierung erfasst im letzten Frame 218
indirekte API-Aufrufe, davon 217 GPU-gezaehlte Multidraw-Aufrufe, keine direkten
Aufrufe; UI-Aufrufe `[1, 0, 0]`. Das ist ein Frame-Snapshot, kein Mittelwert,
und nicht die variable Anzahl GPU-interner Subdraws (`None`). Fullscreen-
Draws und vollstaendige Frame-Drawzahlen bleiben offen.
Rohdaten: [Streaminglauf](native-streaming.log).

## CPU-Worker

Ryzen 9 5900X, vier Worker, 4375 Sections, Oberflaechenzentrum `(0,7,1)`,
Radien 12/3, ein Warmup und fuenf Messungen. Kein Renderer/GPU-Upload. Die
Render-Vorbereitung enthaelt gepaddete Voxel, Greedy-Meshing, Props und Wasser;
ihre Differenz zur Headless-Zeit ist keine isolierte Meshingzeit.

| Median, s | Vorher, alte Geologie | Nachher, alte Geologie | Nachher, neue Geologie |
| --- | ---: | ---: | ---: |
| Headless-Generierung | 0.278028 | 0.291779 | 0.264253 |
| Render-Vorbereitung | 0.489486 | 0.512423 | 0.468152 |
| Greedy-Quads | 512 458 | 512 458 | 471 440 |
| Props | 43 952 | 43 952 | 43 952 |
| Material-Checksumme | 25 753 415 | 25 753 415 | 23 756 359 |

Die alte Geologie ist in dieser Probe materialidentisch, aber CPU-seitig
4.95 beziehungsweise 4.69 Prozent langsamer. Das wird nicht als CPU-Gewinn
ausgegeben. Die neue Geologie erzeugt absichtlich andere Hoehlen/Salzbelegungen
und weniger Quads; ihre kuerzeren Zeiten sind kein bitidentischer Speedup.
Rohdaten: [Vorher](before-worker.log), [Nachher alt](after-worker-legacy.log),
[Nachher neu](after-worker-native.log).

## Funktionsabnahme

- [Formatierung](fmt.log), [Workspace-Check](check.log), [Clippy mit -D warnings](clippy.log)
  und [Workspace-Build](build.log) bestanden.
- [Workspace-Tests](tests.log): 853 bestanden; ein vorhandener 180-s-UDP-Soak
  bleibt standardmaessig ignoriert.
- [Native Geologie](native-geology.log): drei echte Hoehlentypen, Salzkammer und
  Cliffted Desert mit fuenf GPU-Aufnahmen; alle fuenf Ansichten gesichtet.
  Eine feste warme Inspektionslampe macht die natuerlichen Kammern sichtbar,
  sie ersetzt keine neue Gameplay-Lichtquelle oder kuenstliche Cave-Geometrie.
- [Native Inventarregression](native-inventory.log): neun Aufnahmen, echte
  OS-Mausgesten, Crafting-Drag, autoritative Transfers, Suchfeld nach Resize,
  scrollfreie Tooltips und letzte Scrollreihe bestanden. Die Testsequenz wartet
  nach Resize vor der Suche auf das neue TiltUI-Feldelement.
- [Wasser-Palettenfixture](water-fixture-tests.log) sucht ausschliesslich echte
  Gewaesser. Desert umfasst die neue Cliffted-Desert-Gewichtung; geforderte
  Biomdominanz bleibt erhalten, keine injizierten Testwasserflaechen.
- [Headless-Start](headless.log): 27 Sections ohne DISPLAY/WAYLAND_DISPLAY;
  [Server-Abhaengigkeitsbaum](server-tree.log) ohne bevy_render/bevy_ui/bevy_winit/wgpu/TiltUI.
- [Nativer Bewegungs-/LOD-Lauf](native-streaming.log): 17 172 abgeschlossene
  LOD-Wechsel, fuenf GPU-Aufnahmen, keine offenen Mesh-Jobs beim erfolgreichen Ende.

## Grenzen

Neue Geologie erfordert eine neue Welt, vorhandene Welten behalten ihren
Vertrag. Tropfsteinspitzen gehoeren zum tragenden Block und besitzen keine
eigene Fall-/Schadenssimulation. Kuehles Umgebungslicht in Hoehlen bleibt eine
separate Art-/Lichtfrage. Lange, dichte Multiplayer-/Fade-Streaminglaeufe und
weitere GPUs bleiben im technischen Backlog; keine pauschale Leistungsparitaet.
