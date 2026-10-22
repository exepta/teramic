# Flusstunnel, Hohe Wasserfaelle Und Audio-Optionen

## Umsetzung

Das bestehende Fluss-Hoehenprofil schnitt auch bei grosser Ueberdeckung die
gesamte Landschaft bis zum Wasserbett weg. Daraus entstanden tiefe offene
Canyons und massive Abschlusswaende. Der vorhandene gecachte RiverPath kennt
jetzt auch die unveraenderte Gelaendehoehe. Bei mindestens 14 m Ueberdeckung
bleibt das Aussengelaende erhalten; die Dichtefunktion schneidet darunter einen
gewoelbten, wasserfuehrenden Hohlraum aus. Es gibt keine Portal-Attrappe und
keinen zweiten Hoehlengenerator.

`SurfaceColumn.river_cavity` trennt Hoehlenboden/-decke von `height`, der
aeusseren Oberflaeche. Wasserabfragen und Meshing benutzen denselben quantisierten
Hoehlenboden. Section-Envelope und Headless-Generierung umfassen auch tiefe
Wasserlaeufe. Pflanzen bleiben auf dem Aussengelaende; Baumwurzeln ueber
offenen Fallschaechten werden abgewiesen. Verdeckte Pfadenden erhalten maximal
40 m Anschlussgalerie mit gefluteter Rundkammer, kein unendliches Flussnetz.

Kleine Gefaelle bleiben geneigt, Wasserfaelle beginnen weiterhin bei 5 m.
Ein bereits vorhandener natuerlicher Hoehenverlust von 30 bis 44 m innerhalb
64 m kann einmal pro Pfad zu einem Katarakt gebuendelt werden. Nur der Unterlauf
wird abgesenkt. Bei verdeckten Laeufen entsteht ein realer vertikaler Schacht
bis oberhalb des oberen Pegels, damit Wasser nicht durch eine massive Decke
faellt. Die vorhandenen gekruemmten Fallbaender, Schaumattribute und
weltweiten 1-m-Unterteilungen werden weiterverwendet.

Generatorversion 5, Seed, Material-/Welt-IDs, Journalformat und Netzwerkprotokoll
bleiben unveraendert. Die korrigierte Grundgeometrie betrifft bestehende
`rivers = true`-Welten; gespeicherte Block-Edits bleiben an ihren Koordinaten.
Fuer Vergleiche eine neue Welt anlegen. Welten ohne Fluesse bleiben geometrisch
unveraendert. Keine Dependency-Aenderung und keine neuen Render-Paesse.
Client und Server gemeinsam neu bauen/starten: Trotz unveraenderter
Protokollnummer duerfen alte und korrigierte Flussgenerator-Binaries nicht
innerhalb derselben Sitzung gemischt werden.

## Audio

Der bestehende TiltUI-Optionsdialog besitzt jetzt Graphics, Controls und Audio.
Master Volume, Ambient Sounds, Water & Shore, River Current, Waterfalls und
Mute verwenden die vorhandenen Settings-Rows und den gemeinsamen Entwurf.
`configs/audio.toml` wird validiert geladen bzw. bei Fehlen erzeugt. Apply
speichert und aktiviert; Reset aendert nur den aktuellen Entwurf, Back fragt
nach Verwerfen. Display-Timeout und fehlgeschlagenes Speichern rollen auch
Audio korrekt zurueck. Es gibt keine wirkungslosen Musik-/SFX-Regler.

Die tatsaechlichen Stereo-Pegel sind `master * ambience * category`; Mute
setzt alle drei Stimmen auf null. Der Fluss-Grundpegel sinkt von 0.19 auf 0.10.
Ein weicher gefiltertes Stroemen mit seltenen gedaempften Plaetscherimpulsen
ersetzt den hellen Rauschanteil. Glatte Entfernungsdaempfung und 140-ms-Gain-
Uebergaenge vermeiden abrupte Mixwechsel. Wasserfall und Unterwasser-Tiefpass
bleiben unterscheidbar.

Hoerposition bleibt der Spielerkopf, nicht die Kamera. Maximal drei bestehende
Voxel-DDA-Strahlen alle 125 ms pruefen Abschirmung durch Fels und Waende;
verdeckte Quellen erhalten 5 Prozent Pegel, unbekannte/besetzte Endpunkte
bleiben stumm. Kein Raycast im Audio-Thread, kein Audio-Entity pro Wasserzelle.
Es bleibt ein AudioPlayer mit drei Stimmen. Menschliche Hoerabnahme ist offen.

## Wasser Als Zellen

Fuer Umleitung nach Uferabbau ist ein logisches Zellraster sinnvoll, aber
sichtbare Einzelwuerfel sind dafuer nicht erforderlich. Der aktuelle Stand ist
weiterhin statisches prozedurales Wasser: Uferedits remeshen vorhandene Flaechen,
simulieren jedoch keinen Zufluss in neue Zellen. Das ist ausdruecklich offen.

Geeignete Integrationspunkte fuer den naechsten Schritt:

- Sparse Fluid-Zellen mit Fuellstand, Quelle und Stroemung in den vorhandenen
  16er-Sections, ohne bestehende feste Material-IDs umzudeuten.
- Autoritativer, budgetierter Active-Cell-Solver im gemeinsamen Integrated-/
  Dedicated-Server; `commit_block_edit` weckt betroffene Nachbarzellen auf.
- Persistenz und Replikation ueber `EditJournal`/`SectionDelta`, nicht ueber
  konkurrierende Client-Zustaende; benoetigt eine bewusste Protokollerweiterung.
- Bestehende Worker erzeugen zusammenhaengende Wasseroberflaechen aus
  Fuellstaenden; keine GLBs, Mesh-Entities oder Draw Calls pro Wasserzelle.
- Gemeinsame Fluid-Abfrage fuer Schwimmen, Nebel und Audio; begrenzte Aktivierung
  an Chunk-Grenzen sowie Tests fuer Reload, Late Join und Massenerhaltung.

## Validierung

Alle fuenf Workspace-Pruefungen bestehen mit dem finalen Code:
```bash
cargo fmt --all --check
cargo check --locked --workspace -j 2
cargo test --locked --workspace -j 2
cargo clippy --locked --workspace --all-targets -j 2 -- -D warnings
cargo build --locked --workspace -j 2
```
270 Tests bestanden, ein vorhandener 180-s-Netzwerk-Soak standardmaessig
ignoriert; in dieser Runde nicht erneut ausgefuehrt. Separater
`cargo build --locked -p terra-server -j 2` und Start mit entfernten
DISPLAY/WAYLAND_DISPLAY, `--smoke-test --no-save --bind 127.0.0.1:0`, bestehen
mit 27 Sections. Der separate Feature-Baum enthaelt kein bevy_audio, bevy_render,
bevy_ui oder TiltUI.

Neue Tests pruefen deterministische Tunnel samt solider Decke, identische
Headless-/Render-Daten, negative Koordinaten, tiefe Streaming-Grenzen und
gueltige Wasserfallgeometrie ueber drei vertikale Sections. Der Defaultseed
42042 erzeugt einen echten Fall bei X=-937, Z=-5116, Pegel
54.8844249 -> 23.2988539, also **31.585571 m**. Keine synthetische Mesh-Fixture
als Nachweis fuer eine generierte Landschaft.

Native Vulkan-Pruefungen und angesehene Aufnahmen:
- Tunnelportal nahe X=65.5, Z=109.5 und Unterwasser-/Umweltrueckkehr.
- Hoher Wasserfall im tatsaechlichen Schacht, Blick vom Kamm nach unten und
  vom unteren Becken nach oben. Die erste Aussenkamera war durch die
  erhaltene Landschaft verdeckt; die Fixture wurde daraufhin korrekt im
  freien Schacht positioniert.
- TiltUI-Audio-Seite bei 1280x800 und 960x640; der kleine Pointertest unter X11.
  Reale Pointer-Navigation und die grosse Aktionssuite pruefen Slider,
  Apply/Reset/Discard, Mute, TOML-Reload und Integrated-Session-Neustart.
- Echter AudioSink und Decoder-RMS pruefen Einstellungen, Kategorien,
  Stummschaltung, Spieler-/Kameratrennung, Unterwasserfilter und Cleanup.
- Physik-Smoke fuer Fallen, Landen, Springen, Wand, Menue und Supportabbau.
  Ein erster Lauf fand einen Fehler im Testaufbau: Grounded blieb nach dem
  Fixture-Teleport vom alten Ort gesetzt. Der Test setzt jetzt Kontakt,
  Geschwindigkeit und Interpolationspose zurueck; anschliessend bestanden.
  Der produktive Controller wurde dabei nicht veraendert.

Aufnahmen liegen unter [captures](captures/), Protokolle unter [logs](logs/).
Ein anfangs fehlendes Audio-Tab-Icon wurde durch das tatsaechlich vorhandene
TiltUI-`headphones`-Icon ersetzt. Keine menschliche Hoerabnahme behauptet.

## Performance

Zwoelf isolierte native Laeufe: sechs unmittelbar vor dem Umbau und sechs mit
dem finalen Build. Je 18 s Steady State, identische Kameras, 1920x1200, VSync
aus, Sichtweite 16/3, RX 6950 XT / Vulkan / Mesa 26.2.4, maximal vier parallele
Chunk-Jobs.
Keine Builds, anderen Testclients oder CPU-Benchmarks gleichzeitig. Der feste
Flussblick verwendet wie zuvor `TERRA_RIVER_BASELINE_VIEW=1`, nicht eine nach
der Aenderung anders gefundene Wasserfallposition.

```bash
TERRA_RIVER_BASELINE_VIEW=1 bash tools/benchmarks/run-hydrology-suite.sh OUTPUT
```

Jede Zelle: vorher / nachher. RSS ist Prozess-Peak in KiB.

| Szene | FPS | p95 ms | p99 ms | Initialladen s | Peak RSS KiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| forest-1 | 120.193 / 119.962 | 9.931 / 10.396 | 13.803 / 13.944 | 3.040 / 3.084 | 745452 / 777404 |
| forest-2 | 119.544 / 119.748 | 9.987 / 10.109 | 14.089 / 13.853 | 3.208 / 3.255 | 750432 / 760616 |
| offline | 146.663 / 147.335 | 8.271 / 8.742 | 9.222 / 10.902 | 2.960 / 3.044 | 756820 / 776868 |
| integrated | 146.579 / 146.869 | 8.537 / 8.634 | 9.693 / 10.197 | 3.120 / 3.009 | 784888 / 784608 |
| river-1 | 148.143 / 149.504 | 8.409 / 8.185 | 10.739 / 10.406 | 2.921 / 2.931 | 768972 / 781928 |
| river-2 | 149.123 / 149.158 | 8.140 / 8.252 | 9.917 / 10.671 | 2.902 / 2.987 | 763952 / 768252 |

FPS bleiben in diesen kurzen Laeufen praktisch auf demselben Niveau:
Wald -0.19/+0.17 Prozent, fester Flussblick +0.92/+0.02 Prozent.
**Kein allgemeiner Speedup-Nachweis.** Die Offline-p99 steigt
9.222 -> 10.902 ms, Integrated 9.693 -> 10.197 ms; auch einzelne p95- und
Speicherwerte sind hoeher. Keine Behauptung regressionsfreier Frametime-Tails.

Wald: unveraendert 7704 geladene / 3531 gerenderte Sections, 9171 Mesh-Assets,
89866 repraesentierte Props, 1277 Prop-Batches. Fester Flussblick:
7635 geladene Sections, 4342 -> 4344 gerenderte Sections,
9023 -> 9028 Mesh-Assets, 63355 -> 63245 Props, 1240 Batches,
49 Wasser-Tiles. Die Geometriekorrektur aendert in dieser Flussregion auch
den generierten Inhalt; FPS-Differenzen sind kein isolierter Shadervergleich.
Terrain-Kompositionsarbeit Wald: 0.809/0.837 -> 0.738/0.892 summierte
Worker-Sekunden; Fluss: 0.592/0.595 -> 0.603/0.618. Diese summierten Zeiten
sind weder gesamte CPU-Weltgenerierung noch Mainthread-Stalls.

GPU-Wasser-/Transmissionspass Fluss:
0.24830/0.24890 -> 0.24755/0.24761 ms Mittelwert.
Das sind instrumentierte Einzelpaesse, **keine gesamte GPU-Framezeit** und
keine gemessene Draw-Call-Zahl.

Zusaetzliche neue Tunnelansicht, nur nachher gemessen und daher kein A/B-Wert:
136.437 FPS, p95 9.368 ms, p99 12.161 ms, Initialladen 3.997 s,
Peak RSS 831632 KiB. 7691 geladene / 5948 gerenderte Sections,
10580 Mesh-Assets, 73890 Props, 1252 Prop-Batches, 56 Wasser-Tiles.
GPU-Transmissionspass im Mittel 0.51543 ms. Kamera und Flags stehen in
`after/tunnel-scene.log`; Reproduktion mit `TERRA_RIVER_TUNNEL=1` statt
`TERRA_RIVER_BASELINE_VIEW=1`. Kein Vergleich der Tunnelansicht mit einer
anders gefuellten alten Landschaft als behauptete Optimierung.

Der separate CPU-Streamingbenchmark bleibt bei 4375 Sections / 158 Frames /
2.626 s; Spaltenwechsel 0.117 s, Eviction 4.933 s, Reload 2.626 s.
Peak RSS diesmal 200108 KiB; historischer unmittelbar vorheriger Bericht
173164 KiB. Kein frischer CPU-RSS-A/B-Lauf, die Speicherabweichung wird nicht
als Verbesserung ausgegeben. Dieser Benchmark ist auf 60-Hz-Streamingbudgets
getaktet, ohne GPU-Uploads und mit `rivers=false`; er misst nicht die
Initialgenerierung einer neuen Tunnelregion. Rohdaten: `logs/cpu-streaming.log`.
