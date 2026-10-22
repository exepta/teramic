# Phase 75: Wolkendecken, Schneestaerken Und Echte Miniaturkarte

Keine neuen Dependencies, keine geaenderten Seeds, Block-IDs, Weltgeneration,
Protokollversionen oder Dateiformate. Protokoll 42 / IDX6 bleiben gueltig.

## Wolken Und Himmelskoerper

Breite, niedrige, unregelmaessige Puffbaenke wachsen von vereinzelten Wolken
ueber aufgelockerte Baenke zu einer geschlossenen grauen/dunklen Decke.
Zwei grobe Fernringe schliessen den bislang kurzen Wolkenhorizont.
Koplanare exponierte Voxel-Faces werden in einem gemeinsamen Batch zusammengefasst, asynchron aufgebaut
und nach Upload ohne vollstaendige CPU-Geometriekopie. Keine Voxel-Entities,
kein Raymarch und kein groesseres Terrain-Streamingbudget.

Die Sonnensphaere lag bei 0.96*far vor den Wolken. Wolken behalten jetzt ihre
physische Tiefe; nur die Himmels-/Sonnensphaere nutzt eine explizite
Reverse-Z-Hintergrundtiefe. Bevy projiziert perspektivisch mit unendlicher
Reverse-Z-Ferne; das vorhandene NoFrustumCulling erhaelt die Wolken ueber Y420
auch bei kurzer Terrain-Sichtweite. Sonne/Mond bleiben hinter Wolken, ohne die
Terrain-Farplane zu vergroessern oder den Atmosphaeren-Tiefenpfad zu verfaelschen.

## Schnee

Die bisherige Blizzard-Optik ist die neue Schneestufe 1. Die Prozentangaben werden
relativ dazu interpretiert, ausser Stufe 5 explizit relativ zu Stufe 4:

| Stufe | Dichte relativ neuer Stufe 1 | Dichte relativ alter leichter Stufe | Groesse relativ alter Stufe |
| --- | ---: | ---: | ---: |
| 1 | 1x | 4x | 2x |
| 2, 400% von 1 | 4x | 16x | 2x |
| 3, +350% von 1 | 4.5x | 18x | 2.6x |
| 4, +400% von 1 | 5x | 20x | 3.5x |
| 5, +800% von 4 | 45x | 180x | 4x |

`/weather set snow 1` bis `5`; `blizzard` bleibt Alias fuer die staerkste Stufe.
Die vorhandenen numerischen gespeicherten Stufen verwenden die neuen Profile.

Die extreme Dichte wird nicht durch Millionen geometrischer Flocken erreicht.
Qualitaetsbudgets begrenzen die Lagen auf 12/24/48/64 je Spalte (Low..Ultra),
maximal 13.872 Schneequads High bzw. 40.000 Ultra. Eine vollstaendige Basisschicht
faellt weiterhin bis zum Boden; zusaetzliche Whiteout-Lagen werden auf ein engeres
Volumen um Augenhoehe verdichtet. Die lokale raeumliche Flockendichte folgt dadurch
dem gewuenschten Faktor, nicht die globale allozierte Quadzahl. Bewegung bleibt
im vorhandenen WGSL-Batch, Dach-/Unterwasserregeln und Qualitaet Off bleiben aktiv.

## Laden Und Audio

Erst bei tatsaechlich fertigen Chunks, Terrain-/Propmeshes und Wasser wird 100%
angezeigt. Danach mindestens 120 ms Halt und 900 ms Smoothstep-Reveal ueber die
bestehende TiltUI-Menuehuelle. Grosse Frameaussetzer ueberspringen den Fade nicht;
der Zeitschritt ist auf 50 ms begrenzt. Keine vorgetaeuschte Fortschrittszeit.

Gameplay-Eingabe und Mausfang werden erst nach dem Reveal freigegeben.
Wasser- und Tier-/Umgebungs-/Regen-/Feuer-/Schrittmixer bleiben waehrend Loading
abgeschaltet und leeren alte Ereignisse. Menue-Klickfeedback bleibt UI-Feedback.
Fehler/Abbruch setzen die Huelle wieder auf undurchsichtig zurueck.

## Wasser Und Regennaesse

Eine explizite Unterwasserklasse in UV1 unterdrueckt sowohl neue Regennaesse
als auch alte gespeicherte Nassfilme. Sie gilt fuer natuerliche, platzierte und
fliessende Wasserzellen, auch unter ueberhaengenden Baumkronen. Greedy-Flaechen
werden nur beim Wechsel der Expositionsklasse aufgeteilt.
Wasserlinien-Caches benoetigen etwa 1 KiB je Spalte und invalidieren Terrain erst
beim Ueberqueren einer Blockoberseite, nicht bei jedem Fluidtick/Fuellstandswechsel.
Vorbereitete Spalten und bestehende Fluiddaten ersetzen neue Noise-Jobs.

Der Wellensaum benutzt interpolierte Kuestenabstaende, feinere Nahbereichsfilme,
rollende ankommende Schaumkaemme und Ruecklauf. Ein gemeinsamer Wetter-Uniform
bewegt gekreuzte Wellen bei Sturm staerker/schneller, erhoeht Schaum/Weisswasser und aendert
die Rauheit. Maximaler Vertexversatz 0.22 m, erweitertes AABB 0.24 m.
Section-Batches und Materialhandles werden wiederverwendet. Keine neuen
Wasser-Draws, Fluidzellen, Reflexionspaesse oder geaenderte Wasserphysik.
Geteilte Eckgewichte pinnen Wellen an Wasserfall-Lippen und dynamischen
Fluidgrenzen; Impact-/Poolflaechen bleiben verbunden. Das zusaetzliche
Float32-Attribut kostet vier Bytes je Wasservertex und wird im bestehenden
asynchronen Meshing berechnet. Shader-Ableitungen stehen ausserhalb
pixelabhaengiger Verzweigungen.

## Echte 3D-Miniatur

Die optionale FullMap-3D-Ansicht verwendet eine zusaetzliche orthografische Kamera
auf dieselben Terrain-, Wasser-, Baum-, Prop-, Tier-, Drop- und Spieler-Meshes.
Originalmaterialien, vorhandene Animationen, Wind und Lichtwerte werden geteilt.
Auch die bereits gefilterten Diffus-/Spiegelungs-Cubemaps der Spielkamera werden
mit denselben Handles, Intensitaeten und Rotation weiterverwendet. Es entstehen
keine zusaetzlichen Cubemap-Assets oder Environment-Filterpaesse.
Keine duplizierte Welt, keine nachgebauten Tiermodelle und keine Hoehenfeld-Pfeiler.

Reservierte Sichtbarkeitslayer 27/28 trennen Kreaturen und Welt von Himmel,
First-Person-Items, Inventarvorschauen und Bildschirmpartikeln.
Layer 26 erhaelt nur die Richtungslichter in der ausgeblendeten Hauptkamera:
Die Atmosphaeren-LUTs und die geteilte Umgebungsbeleuchtung bleiben dadurch
beleuchtet, ohne Weltmeshes nochmals zu zeichnen.
Der lokale First-Person-Spielerkoerper ist nur fuer die Miniatur sichtbar.
Beim Oeffnen wird der redundante Hauptwelt-Meshdurchlauf unter der FullMap unterdrueckt;
die UI-Kamera bleibt aktiv, daher sind ihre Fullscreen-/Hintergrundkosten nicht
vollstaendig entfernt. Beim Schliessen werden Kamera-Layer und Prop-Distanzbereiche
exakt wiederhergestellt. Pan/Zoom und Marker teilen dieselbe Projektion.
Die orthografische Projektionsflaeche wird bereits beim Erzeugen korrekt
aufgeloest und vor Bevys Kameraaktualisierung gesetzt. Dadurch faellt das
Frustum nicht zwischen zwei Frames auf die Standardflaeche zurueck.
Die vorhandenen Sonnen-/Mondkaskaden werden waehrend der Miniatur auf das
tatsaechlich belegte Tiefenband fokussiert, statt den leeren Kameravorraum
abzudecken. Maximal drei Kaskaden und nie mehr als die gewaehlte Qualitaet;
keine zusaetzlichen Lichter oder hoeheren Kaskadenbudgets. View-interne
Schattenressourcen und leere Hauptkamera-Clears sind kein kostenloser Effekt.
Beim Schliessen werden die Originalwerte exakt wiederhergestellt, externe Qualitaetsaenderungen bleiben
erhalten. Deaktivierte Schatten werden nicht eingeschaltet.

Die zusaetzliche Sicht braucht GPU-Arbeit: 1024 Pixel Breite, MSAA4, Tiefen-/
Transmissionspfad und vorhandene Weltmaterialien. Sie erstellt keine eigenen
Meshassets oder neuen Chunks. Nur wirklich geladenes Gebiet ist dreidimensional
sichtbar. Die komplette erkundete Fernkarte bleibt in 2D/2.5D erhalten.
Ueberhaenge verdecken Hoehlen wie in einer echten Weltansicht; keine Roentgenkamera.
An Grenzen des geladenen Volumens sind Schnittkanten sichtbar; die Kamera
generiert bewusst kein entferntes Terrain, um diese zu verdecken.

Die Wolkenunterseiten sind bewusst voxelgestuft und noch kompakter als die
Referenz. Keine volumetrische Mehrfachstreuung oder vollstaendige Referenzparitaet.
Sehr dichter Schnee bleibt stark stilisiert; die hoechste Stufe zeigt grosse
Flocken und ist kein fotorealistischer Whiteout.

## Validierung

Nativ auf AMD Radeon RX 6950 XT, RADV/Mesa 26.2.4, Vulkan, Ryzen 9 5900X:

- Wetterfixture: 45 Phasen bestanden, inklusive aller Schneestufen, fuenf
  Wolkendeckungen, Sonnen-/Mondverdeckung, bewegtem Kuestensaum, Sturm/ruhigem
  Meer und untergetauchtem Boden bei Regen mit gespeichertem Nassfilm.
- Echte integrierte Sitzung: 289/289 Chunks, sichtbarer 100%-Halt und Reveal
  aufgenommen. Beide Welt-Audiostreams bleiben bis nach dem Reveal aus.
- Workspace: Formatpruefung, Check, Tests, Clippy mit `-D warnings` und Build
  erfolgreich, jeweils mit unveraendertem Lockfile. 1177 Tests bestanden,
  ein explizit ignorierter 180-s-UDP-Soak nicht erneut ausgefuehrt.
  Darunter 220 UI-, 295 Render- und 281 World-Tests; der fokussierte
  Wasserlauf besteht alle 14 Tests.
- Dedicated Server ohne `DISPLAY`/`WAYLAND_DISPLAY` gestartet: 27 Sections
  erfolgreich. Normaler Dependency-Baum ohne `bevy_render`, `bevy_pbr`, WGPU,
  Winit, TiltUI, `terra-render` oder `terra-ui`.
- Kartenfixture: alle 52 Stufen bestanden, einschliesslich Pan/Zoom, echter
  Peer-/Mobmodelle, Wiederherstellung, Hoehlen, Karten- und Wegpunktbedienung.
  In der Miniatur sind 2612 Terrain-, 319 Prop- und 22 Spieler-Teilmeshes
  frustum-sichtbar. Der Tier-Schalter wechselt von 0 auf 661 Tier-Teilmeshes,
  nicht 661 Tiere. Drei Schattenkaskaden und geteilte Umgebungsbeleuchtung
  sind aktiv; die fertigen Bilder wurden angesehen.

Alle Cargo-Laeufe wurden mit `CARGO_BUILD_JOBS=1` ausgefuehrt, damit parallele
Bevy-Linker auf diesem bereits belegten Host nicht den Arbeitsspeicher ueberlasten.
Der Check verwendet `--locked`; es gibt keine Dependency-Aktualisierung.
Im simulierten Boost-/Resume-Test protokolliert Winit unter Wayland zweimal,
dass der Cursor ohne echten OS-Lock nicht versetzt werden kann. Der native
Eingabe-/Pan-Test und Exit sind trotzdem erfolgreich; das ist kein Shaderfehler.

### Kurze Framezeitfenster

1920x1080, Sichtweite 8, vertikal 3, Seed 42042, separate Wetterfixture,
kontinuierlicher Winit-Lauf ohne VSync. Je Phase etwa 1.5 s nach 0.5 s Anlauf,
keine zeitgleich laufende Cargo-Kompilierung. Die Zeilen sind unterschiedliche
Wetterzustaende desselben Builds, kein Vorher-/Nachher-Benchmark:

| Zustand / Phase | FPS | p95 ms | p99 ms | Niederschlagsquads |
| --- | ---: | ---: | ---: | ---: |
| Schnee 1 / 16 | 167.16 | 6.713 | 7.522 | 2312 |
| Schnee 5 / 20 | 154.01 | 7.833 | 9.222 | 13872 |
| Sturm am Meer / 38 | 175.47 | 6.629 | 7.260 | 2423 |
| Ruhiges Meer / 40 | 181.47 | 6.335 | 7.004 | 0 |

Wolken bleiben ein Batch mit rund 101000 zusammengefassten Faces; Niederschlag
bleibt ein Batch. Das sind weder die vollstaendige Frame-Drawzahl noch
GPU-interne Subdraws. Keine isolierten GPU-Zeiten, Peak-RSS-/VRAM-Paritaet,
Langzeit-Streamingwerte oder identische Vorher-/Nachher-Messpaare erhoben.
Historische Regressionen bleiben in den vorherigen Berichten erhalten.

Rohdaten: [Wetter](logs/terra75-weather-verified.log),
[Laden](logs/terra75-loading.log),
[Karten](logs/terra75-map.log),
[Workspace-Tests](logs/terra75-tests-final.log),
[Clippy](logs/terra75-clippy-final.log),
[Headless](logs/terra75-headless.log).
Bilder: [100%](captures/loading-100.png),
[Reveal](captures/loading-reveal.png),
[Sturmwolken](captures/cloud-deck-4-sun.png),
[Sturmbrandung](captures/ocean-storm-crossing-waves.png),
[Unterwasser bei Regen](captures/underwater-rain-no-wet-film.png),
[Echte Miniatur](captures/miniature-models.png),
[Miniatur mit Tieren und Pan](captures/miniature-models-panned.png).

Kurze Funktionsproben sind kein Langzeit- oder Mehr-GPU-Nachweis und keine
behauptete FPS-/Speicherparitaet. Die Bilder wurden angesehen; eine gemeinsame
abschliessende Art-Abnahme bleibt offen.
