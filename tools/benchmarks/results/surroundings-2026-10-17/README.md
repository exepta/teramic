# Phase 40: Rundum-Vorbereitung, Abendlicht Und Propdichte

## Aenderungen

Das CPU-Streaming und die Section-Mesh-Jobs arbeiteten bereits nach Entfernung
rund um den Spieler. Die optionale Chunk-Animation hielt fertig vorbereitete
Wurzeln ausserhalb des Kamerafrustums jedoch unbegrenzt unsichtbar. Diese werden
jetzt im Hintergrund freigegeben: maximal 64 Wurzeln pro 100-ms-Queue-Refresh,
ohne temporaere Fade-Materialien oder Belegung der 96 Animationsplaetze.

Die Freigabe verlangt gueltige Bounds und propagierte Welttransformationen.
Frisch erzeugte Kinder besitzen kurzfristig noch Identity-GlobalTransforms;
deren vorlaeufige Position darf keine Entscheidung ueber Hintergrundfreigabe
ausloesen. Sichtbare neue Batches warten weiterhin auf ihren ersten Farb-Draw
und blenden ab Alpha null ein. Job-, Sichtweiten- und Uploadbudgets sind gleich.
Normales Frustum-Culling bleibt aktiv.

Nacht-Ambientlicht 90 -> 300, mit der vorhandenen glatten Tagesinterpolation und
neutralerem Blauton (sRGB 0.65/0.75/1.0 statt 0.40/0.52/1.0). Das erhoeht das
Fuelllicht fuer Schatten zwischen 19 und 22 Uhr. Tageslicht,
Belichtung und Sternenhimmel bleiben erhalten. Auch das Unterwasser-Environment
verwendet den gemeinsamen Helligkeitsfaktor.

Natuerliche Blumen und kleine Pilze werden nach Auswahl aller Gruppenmitglieder
mit unabhaengigem stabilem ID-Hash auf 25 Prozent reduziert. Frei gewordene
Plaetze werden nicht neu belegt. Andere Pflanzenarten und platzierte Props
bleiben vorhanden; verbleibende Gruppen nutzen die bisherige Zentrierung.
Seeds, IDs und Generatorversion aendern sich nicht. Die neue Verteilung gilt
beim Generieren/Reload von Gruppen, nicht rueckwirkend fuer bereits geladene
RAM-Gruppen. Grosse Pilzbaeume sind keine kleinen Pilz-Props.

## Dichtepruefung

Identische 49.152 Stuetzzellen mit Forest-/Plains-/Mushroom-Park-Profilen,
gleiche Kandidaten vor und nach der Reduktion. Synthetische feste Profile fuer
eine reproduzierbare Quote, keine FPS-/Welt-Streamingmessung.

| Profil | Art | Vorher | Nachher |
|---|---|---:|---:|
| Forest | Blumen | 3634 | 894 |
| Forest | Pilze | 1673 | 463 |
| Plains | Blumen | 22829 | 5704 |
| Plains | Pilze | 503 | 133 |
| Mushroom Park | Blumen | 3121 | 774 |
| Mushroom Park | Pilze | 26045 | 6534 |
| Gesamt | Beide | 57805 | 14502 |

Behalten: 25,09 Prozent. Die Tests pruefen zusaetzlich stabile IDs/Varianten/
Groessen, Wiederholbarkeit, erhaltene andere Arten, fehlende Ersatzkandidaten
und vorhandene Save-/Reload-Interaktionen.

## Native Reproduktion

Vom Repository-Root mit isolierten Smoke-Configs:

```bash
env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra-surroundings-menu \
  TERRA_CAPTURE_DIR=/tmp/terra-surroundings-animations \
  target/debug/terra-client --smoke-test-animations
```

Fuer Beleuchtung `--smoke-test-daylight` und ein eigenes Capture-Verzeichnis.
Die Fixture nimmt zusaetzlich zu den bisherigen neun Ansichten dieselbe
Uebersicht um 19:00, 20:30 und 22:00 auf. Das Log erfasst Ambient-Helligkeit und
Belichtung. Der Animationstest prueft erste Uploads, vier Bewegung-/Fade-Modi
und reale CPU-Sections/Terrainmeshes in acht Richtungen um den festen Fokus.
Nach Drehung zu einem vorbereiteten Hintergrundmesh darf dessen Queue/Fade
nicht erneut beginnen.

Es wird keine FPS-/Speicherparitaet oder Langzeitabnahme behauptet. Lange
32er-Streaminglaeufe und weitere GPUs bleiben im TODO-Backlog.

## Validierung

Alle fuenf Cargo-Pruefungen erfolgreich: `cargo fmt --all --check`,
`cargo check --workspace`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings` und
`cargo build --workspace`. 638 Tests bestanden, ein vorhandener Langzeittest
ignoriert. Builds mit `CARGO_BUILD_JOBS=2` begrenzt. Logs liegen daneben.

Die erste Testsuite zeigte an zwei Prop-Fixtures, dass noch nicht propagierte
GlobalTransforms nicht fuer eine sofortige Hintergrundfreigabe verwendet
werden duerfen. Die neue Transformpruefung behebt diesen Fall, inklusive
Regression fuer frische Bounds an Identity-Transforms. Auch ein komplett
belegtes Animationsbudget blockiert die Hintergrundfreigabe nicht.

Native Vulkan-Pruefung auf RX 6950 XT/RADV erfolgreich: erster Terrainupload
von Alpha null durch Teildeckkraft, FadeSlow/Bounce/Rise/Slide, acht generierte
Richtungen und Kameradrehung zu vorbereiteter Geometrie ohne neue Queue.
`animations.log` dokumentiert die Assertions, `animations/` die Aufnahmen.

Zwoelf Beleuchtungs-/Unterwasseraufnahmen in `lighting/`; die drei Abendansichten
wurden geoeffnet. Nacht-EV100 bleibt 10.7 und Tages-Ambient 760; Abend-Ambient
ist im finalen Lauf 300. Die erste Probe mit Ambient 130 war besonders um 19 Uhr
noch zu dunkel und bleibt unter `lighting-ambient130/` dokumentiert. Fuer die
gleiche 19-Uhr-Kamera steigt der Grauwertmittelwert der unteren 40 Prozent von
1.899 auf 7.581 (0..255, ImageMagick `-colorspace gray`). Das ist nur eine
Pixelprobe dieser Zwischenstands-/Finalbilder, keine physikalische Lichtmessung
oder Vergleich mit dem urspruenglichen Ambient-90-Build. Wind und Wellen laufen
weiter. Das finale Bild zeigt mehr Details in dunklem Laub und an Ufern.

Dedicated Server ohne DISPLAY und WAYLAND_DISPLAY gestartet: 27 Sections,
erfolgreicher Headless-Smoke-Test (`server.log`).
