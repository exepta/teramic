# Phase 38: Weltzeit Und Tag/Nacht

## Reproduktion

```bash
env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra-daylight-menu \
  TERRA_CAPTURE_DIR=tools/benchmarks/results/daylight-2026-10-17/captures \
  target/debug/terra-client --smoke-test-daylight
```

Die native Fixture verwendet eine echte generierte Oase (Seed 42042), dieselbe
Geometrie fuer Tag/Nacht und den produktiven Atmosphaeren-/Wasserpfad. Nur fuer
die Aufnahmen setzt sie die Uhr auf feste Tageszeiten und wechselt die Kamera.
Sie prueft Screenshot-Rueckmeldungen, nichtleere Bilddaten und korrekten
Unterwasserstatus. Das Netzwerk wird separat ueber echte Loopback-Verbindungen
mit zwei Clients geprueft.

## Architektur Und Grenzen

Die Uhr wird vom gemeinsamen Integrated-/Dedicated-Server fortgeschrieben,
im bestehenden binaeren Index gespeichert und mit Protokoll 15 synchronisiert.
`IDX2` besitzt einen expliziten Leser fuer den alten `INDX`-Record. Die
Region-/Player-Formate aendern sich nicht. Uhr-Metadaten umgehen ausschliesslich
die Terrain-Aenderungserkennung; Dedicated-Autosave verfolgt diese Metadaten
separat. Ein Regressionstest prueft, dass reine Zeitfortschritte keine
Terrain-/Fluid-Neuberechnung melden.

Der bestehende Himmelabschluss erhaelt zwei Uniform-Vektoren fuer Mond und
Sternrotation. Ein zweites Richtungslicht stellt Mondlicht dar; hoechstens eines
der beiden Himmelslichter wirft zur selben Zeit Schatten. Sterne nutzen keine
eigenen Entities oder Texturen. Tagsueber wird der Sternshader uebersprungen.
Die bestehende atmosphaerische Environment-Map und SSR beleuchten/reflektieren
Wasser weiter. Mondoberflaeche und Sternpunkte sind ein dekorativer Himmel-Layer
und nicht Bestandteil der atmosphaerischen Environment-Map.

Die Tests sind keine FPS-/Speicher-Paritaetsmessung. Es liegt kein vergleichbarer
Vorher-/Nachher-Benchmark fuer diese neue Tagesbeleuchtung vor. Historische
Messreihen werden nicht auf diesen Stand uebertragen.

## Native Pruefung

Acht Aufnahmen unter `captures/`, RX 6950 XT / RADV / Vulkan, 1280x800 logische
Fenstergroesse und 1920x1200 Screenshotpixel bei 150 Prozent Desktop-Skalierung.
Tag, Morgen, Abend, Nacht, Mond/Sterne, Unterwasser Tag/Nacht und Fog Off wurden
aufgenommen. Alle Ansichten wurden geoeffnet; Shaderkompilierung und Lauf
ohne Warnungen/Fehler. Der erste Durchlauf zeigte zu dunkles Nachtterrain bei
zu hellem Himmel. Mondlicht, Nacht-Himmelabschluss und die Belichtungsintegration
mit den Grafikoptionen wurden daraufhin korrigiert und erneut nativ geprueft.

Im finalen Lauf: EV100 Tag 13.7, Morgen/Abend 12.402, Nacht 10.7. Der Mittelwert
der RGB-Kanaele in den unteren 40 Prozent der Bilder (0..255, keine physikalische
Luminanzmessung) liegt bei Tag/Nacht 116.08/22.32, unter Wasser 129.33/20.67.
Die Paarbilder verwenden identische Kamera/Geometrie; Wind/Wellen laufen weiter.
Diese Pixelprobe ergaenzt die Sichtpruefung, sie ersetzt keine Art-Abnahme.

`native.log` dokumentiert acht Screenshot-Rueckmeldungen und den jeweils korrekten
Unterwasserstatus. `server.log` dokumentiert einen erfolgreichen Headless-Start
ohne Display mit 27 Sections. `server-dependencies.log` enthaelt weder
bevy_render noch bevy_ui, bevy_winit oder terra-render.

## Automatisierte Pruefungen

Alle fuenf Abschlusspruefungen erfolgreich: `cargo fmt --all --check`,
`cargo check --workspace`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings` und
`cargo build --workspace`. Logs liegen als `fmt.log`, `check.log`, `test.log`,
`clippy.log` und `build.log` in diesem Verzeichnis.

`cargo test --workspace`: 629 bestanden, ein bereits vorhandener Langzeittest
ignoriert. Neue Abdeckung: validierte Zeitkonfiguration, Mitternacht und mehrere
Tage, Pause/Resume, echter Zwei-Client-Late-Join, Speicherformat-Kompatibilitaet,
Reload des Integrated-Server-Kalenders und stabile Regiondateien bei Uhr-Saves.
Der Belichtungstest sichert Grafik-Aenderungen bei Nacht ab. Ein ECS-Test
schliesst Terrain-/Fluid-Invalidierung durch die Uhr aus.

Cargo-Pruefungen werden mit `CARGO_BUILD_JOBS=2` ausgefuehrt. Ein anfaenglicher
unbegrenzter Build erschoepfte mit parallelen Linkern den freien RAM/Swap und
wurde beendet; der begrenzte Build und die Tests wurden danach neu ausgefuehrt.
