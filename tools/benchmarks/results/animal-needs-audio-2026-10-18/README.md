# Phase 56: Herdenspawns, Tierbeduerfnisse Und Aufgenommenes Audio

## Spawns Und KI

Die bestehende deterministische 32-m-Habitatgenerierung erzeugt Gruppen derselben
Art. Zwei Drittel der geeigneten Habitate pruefen drei/vier Landtiere oder
zwei bis vier Fische; Positionen liegen etwa drei Meter auseinander und pruefen
geladenen Boden, Koerperfreiraum und echtes Zellwasser. Bestehende Tiere und tote
IDs bleiben erhalten. Die vorherigen vier Einzelchancen lagen bei 25 Prozent
je Landkandidat; die neue volle flache Testflaeche besitzt rechnerisch im Mittel
2.33 statt 1 Landtier je Habitat. Reales Terrain kann Kandidaten ablehnen.
Belegungswahrscheinlichkeit und Artwahl verwenden unterschiedliche Hashbits;
ein gemeinsamer Modulo-Filter darf keine Tierart vollstaendig ausschliessen.

Der Server aktualisiert weiterhin maximal 128 Tiere pro 100-ms-Takt. Maximal acht
Tiere pro Takt duerfen eine neue Ressourcensuche beginnen; abgelaufene Such-Timer
und die Ruhezeit nach einer Suche verteilen das Budget ueber die rotierenden
KI-Batches, auch bei 256 aktiven Tieren. Pro Suche sind bis zu
2048 geladene Props beziehungsweise 192 Wasserproben erlaubt. Futter-/Wasserziele
verfallen nach 15 Sekunden. Ferne Tiere pausieren weiterhin.

Landtiere verlieren bei aktiver Simulation langsam Nahrung und Wasser, mit
erhoehtem Verbrauch beim Laufen. Bei leeren Werten entsteht langsam Schaden.
Schafe/Kuehe fressen Grasprops; Schweine Pilze, Blumen und kleine essbare
Bodenpflanzen. Wandpilze und Steindekoration bleiben erhalten. Eine abgeschlossene
Fressaktion entfernt genau ein Prop ueber commit_prop_removal, einschliesslich
Journal, Render-Epoche, Reflow, Replikation und spaeterem Reload. Das Tier bekommt
erst nach erfolgreichem Commit Nahrung. Ein Tier greift nicht auf dasselbe schon
gefressene Prop zu. Spielerfuetterung fuellt ebenfalls Nahrung auf.

Durstige Tiere suchen echte Wasserzellen und pruefen einen trockenen Uferpunkt;
auch fast volle Tiere koennen Wasser in direkter Naehe aufsuchen. Trinken dauert
2.4 Sekunden und fuellt bis zu acht Wasserpunkte auf. Die Wasserquelle wird dabei
nicht entfernt. Die Navigation bleibt lokales Steering; komplexe verborgene
Umwege und raeumlich unerreichbare Ziele koennen weiterhin scheitern.

| Art | Leben | Nahrung | Wasser |
| --- | ---: | ---: | ---: |
| Kuh | 10 | 30 | 30 |
| Schaf | 7 | 35 | 25 |
| Schwein | 8 | 20 | 40 |
| Fisch | 3 | -- | -- |

Faustschlaege verursachen einen Lebenspunkt, auch in Creative. Survival kostet
weiterhin Ausdauer; echte Werkzeuge verursachen derzeit fuenf Punkte. Alle
Treffer pruefen serverseitig Blick, Reichweite, Sichtstrecke und Cooldown.

## HUD Und Speicherung

Die neue belegbare Aktion Inspect Animal Stats verwendet standardmaessig F3+E.
Die bestehende Chord-Aufloesung unterdrueckt dabei das normale Inventar-E.
Ein unabhaengiges TiltUI-Panel zeigt das anvisierte Tier mit Leben, Nahrung,
Wasser und Verhalten. Es braucht das allgemeine Debug-Overlay nicht. Zielverlust,
Inventare und Menues blenden es aus; Fische zeigen fuer Nahrung/Wasser `--`.

REG7 erweitert die bestehenden Regionen um Beduerfnisse und deren Suchziel.
Der explizite REG6-Leser liest die alte binaere Tierstruktur und rechnet Leben
anteilig auf die neuen Art-Maxima um; Nahrung/Wasser starten voll. Tote Tiere
bleiben tot. Zuvor leere Habitate werden einmal zur erneuten Gruppenpruefung
freigegeben, Habitate mit gespeicherten Tieridentitaeten behalten ihre Marker.
REG5 und aeltere Leser, PLY3, Weltformat 2, Generator 5, Seeds und Block-IDs
bleiben erhalten. Client und Server benoetigen gemeinsam Protokoll 30.

## Audio

Vierzehn bearbeitete Aufnahmen ersetzen die synthetisierten Tierstimmen.
Schaf-, Kuh- und Schweinrufe besitzen je zwei Varianten. Fressen, Laufen,
Schafschuetteln, Schwimmen und Trinken loesen eigene Cues aus. Schritte und
Schuetteln verwenden natuerliche Boden-/Raschel-Foley; Fische erhalten leise
Wasserbewegungen. Schafschuetteln und Trinken besitzen passende Modellposen.

Alle PCM-Aufnahmen werden einmal auf einem Hintergrundtask mit Bevys vorhandenem
WAV-Decoder geladen und geprueft. Die Bevy-Feature-Erweiterung `wav` aktiviert
`hound 3.5.1`; keine bestehende Dependency-Version wurde aktualisiert. Ein
Audiostream mischt maximal 24 aktive Cues plus 24 wartende Cues und drei sanft
ueberblendete Umgebungsschichten. Pro 50-ms-Szenenupdate entstehen hoechstens
acht neue Cues. Keine Audio-Entity pro Tier, Baum oder Block.

Entfernung und gemeinsames Voxel-Occlusion-Verfahren verwenden die Spielerposition;
die Kamera beeinflusst nur das Panning. Master/Ambience/Mute wirken auch auf
bereits laufende Stimmen. Unterwasser werden die Aufnahmen weich gedaempft und
tiefpassgefiltert. Wind und Laub reagieren auf geladene Kronen; natuerlicher
Hoehlenraum wird ueber Terrainhoehe und reale Deckenproben erkannt. Sanfte
Lautstaerkeuebergaenge und Crossfades vermeiden harte Loopgrenzen.

Quellen, Autoren, Anpassungen und Lizenzen:
[Audio Credits](../../../../assets/audio/CREDITS.md). Audio verwendet CC0,
CC BY 4.0 und fuer vier Farmaufnahmen CC BY-SA 3.0; die jeweiligen Lizenztexte
werden mitgeliefert. Es wurden keine Minecraft-/Modaufnahmen uebernommen.

## Validierung

895 Workspace-Tests bestanden, mit `CARGO_BUILD_JOBS=2` und
`RUST_TEST_THREADS=1`. Der vorhandene explizite 180-s-UDP-Soak bleibt ignoriert.
Zusaetzlich wurden die drei Tests des aufgenommenen Audiomixers gezielt gestartet.
Formatierung, Workspace-Check, strenges Clippy fuer alle Targets und Workspace-
Build sind erfolgreich; die finalen Logs liegen neben diesem Bericht.

```sh
cargo fmt --all --check
cargo check --locked --workspace
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace
```

Nachweise umfassen die natuerliche Artwahl ueber 16 Seeds, Gruppenkonsistenz,
einzeln gefressene Props, Trinken an einer tieferen Wasserzelle vom trockenen
Ufer, angepasste Maxima, echte Ein-Punkt-Fausttreffer, REG6/REG7-Uebernahme sowie
F3+E ohne Inventaroeffnung. Ein echter UDP-Test repliziert 128 Tiere samt
Beduerfnissen zu zwei Clients. Ein weiterer prueft 256 aktive Tiere mit zwei
Clients und beobachtet, dass alle eine Ressourcensuche erhalten.

Die native Vulkan-Aufnahme `captures/creatures-0.png` zeigt die anvisierte Kuh
mit Leben 10/10, Nahrung 30/30 und Wasser 30/30 bei ausgeschaltetem allgemeinem
Debug-Overlay. Layout, Lesbarkeit und Auswahl wurden visuell geprueft. Diese
explizite Modellfixture setzt feste Tiere und aktiviert die Inspektion; die
Tastenkombination selbst ist per Input-/UI-Test geprueft. Sie ist kein nativer
End-to-End-Nachweis eines trinkenden Multiplayer-Tiers. Der erste Bildlauf
(`discarded-parent-gate.log`) deckte die Kopplung der uebergeordneten Seite an F1
auf; danach wurde die Komponente unabhaengig eingebunden und der Bildlauf
wiederholt. Der Renderingtest wartet auch auf den tatsaechlichen Tiernamen im
Textbaum, nicht nur auf einen berechneten Sichtbarkeitswert.

Der native Start bestaetigt das erfolgreiche Decodieren aller 14 Aufnahmen.
Mixbegrenzung, Live-Mute und endliche Samples werden numerisch geprueft;
Raumklassifikation prueft auch eine 40 m hohe Decke und hohe Baumkronen.
Umgebungsproben sind auf 576 Decken- und 96 Kronenzellen je 500 ms begrenzt.
Die nativen Laeufe verwenden `env -u DISPLAY ALSA_CONFIG_PATH=/dev/null`, da hier
kein verlaesslicher Audioausgang zur Verfuegung steht. Keine akustische
Klangabnahme behaupten. Die Wind-/Hoehlenmischung bleibt menschlich abzustimmen.

`terra-server --no-save --smoke-test` startet ohne DISPLAY/WAYLAND_DISPLAY auf
einem eigenen kurzlebigen Port erfolgreich (`headless.log`). Der laufende
Benutzer-Server wurde nicht beendet. Zusaetzliche Erweiterungen stehen in
TODO.md; lange Mehrspieler-/Mehr-GPU-Laeufe bleiben offene Arbeiten.

## CPU-Stichprobe

Direkt gestartete Testbinaries ohne gleichzeitig laufenden Compiler:
128 Landtiere, zwei Spielerpositionen, 100 Takte auf geladener flacher Testwelt.

| Messwert | Phase 55 | Phase 56 |
| --- | ---: | ---: |
| Gesamtzeit (s) | 0.027326 | 0.025443 |
| p95 je 128er-Takt (ms) | 0.328127 | 0.312406 |
| p99 je 128er-Takt (ms) | 0.534161 | 0.425009 |

Rohdaten: `herd-before.log`, `herd-after.log`. Reihenfolge war Nachher, dann
Vorher. Das ist eine sehr kurze CPU-Stichprobe mit anfangs vollen Beduerfnissen,
keine statistisch abgesicherte Verbesserung und kein Maximum hungriger Suchen.
Verhaltenswahl und Art-Maxima unterscheiden sich beabsichtigt. Netzwerk, GPU,
Audioausgabe und persistentes Schreiben sind darin nicht enthalten.

## Nativer Waldvergleich

Je ein 60-s-Lauf, zuerst das vor Arbeitsbeginn gesicherte Phase-55-Binary
`target/debug/terra56-before`, dann das finale Workspace-Binary. Identische
Konfigurationen aus `configs/`, Seed 42042, klare pausierte Tageszeit 10:00,
8er-Sichtweite, VSync aus und 1920 x 1080 physische Pixel. GPU: AMD Radeon RX
6950 XT, RADV/Vulkan. Kein Compiler oder weiterer Testclient lief parallel;
der vorhandene Benutzer-Server blieb unveraendert aktiv.

```sh
env -u DISPLAY ALSA_CONFIG_PATH=/dev/null \
  TERRA_CONFIG_DIR=tools/benchmarks/results/animal-needs-audio-2026-10-18/configs \
  TERRA_BENCH_SECONDS=60 TERRA_RENDER_DIAGNOSTICS=1 TERRA_MEASURE_GPU_MEMORY=1 \
  python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-forest
```

| Messwert | Phase 55 | Phase 56 |
| --- | ---: | ---: |
| Durchschnittliche FPS | 210.48 | 209.48 |
| p95 Framezeit (ms) | 5.937 | 5.863 |
| p99 Framezeit (ms) | 6.664 | 6.500 |
| Initiale Ladezeit (s) | 1.714 | 1.707 |
| Gesamter Prozesslauf (s) | 66.431 | 65.929 |
| Abgetastetes Peak-RSS (MiB) | 749.86 | 742.17 |
| Abgetastetes DRM-Peak-VRAM (MiB) | 2224.96 | 1970.50 |
| Abgetastetes DRM-Peak-GTT (MiB) | 68.14 | 68.14 |
| GPU Opaque-Pass Mittel (ms) | 1.033 | 1.036 |
| GPU Early-Prepass Mittel (ms) | 0.348 | 0.354 |
| Terrain-Komposition, Worker gesamt (s) | 0.739 | 0.734 |
| Geladene / dargestellte Sections | 2041 / 875 | 2041 / 875 |
| Mesh-Assets / sichtbare Mesh-Entities | 3511 / 1745 | 3511 / 1745 |
| Repraesentierte Props / Prop-Batches | 25808 / 328 | 25808 / 328 |

Rohdaten: `before-forest.log`, `after-forest.log`. Die nachtraegliche Last umfasst
unter anderem das Decodieren aller 14 Aufnahmen und lokale Umgebungsproben.
Diese Offline-Szene hat keine aktive oder sichtbare Tierherde. Sie misst weder
die Mehrspieler-KI noch die Audiogeraeteausgabe oder bewegtes Langzeitstreaming.
Der vorherige Lauf enthielt einzelne Ausreisser bis 73.71 ms; daraus wird keine
Verbesserung durch die Aenderungen abgeleitet. Speicherwerte sind abgetastete
Prozess-/DRM-Werte und koennen von Treiber-Caching und Messintervallen abhaengen;
insbesondere der VRAM-Unterschied ist kein nachgewiesener Optimierungseffekt.
GPU-Passzeiten sind keine vollstaendige GPU-Framezeit. Draw-/Subdrawzahlen
wurden in diesem Lauf nicht aktiviert. Eine Stichprobe auf einer GPU belegt
weder allgemeine Leistungs-/Speicherparitaet noch Langzeitstabilitaet.
