# Phase 41: Abendlicht, Pilz-LOD Und Menuefeedback

## Umsetzung

Die tatsaechliche Sonnenhoehe lag um 17 Uhr bei etwa 13,5 Grad. Dadurch wurden
Schatten mehr als viermal so lang wie ihre Objekte und horizontale Oberflaechen
erhielten trotz hoher Sonnenintensitaet wenig direktes Licht. Die stilisierte
Richtung von Sonnen-/Mondlicht hat nun mindestens 30 Grad Hoehe, solange das
jeweilige Licht aktiv ist. Schatten auf horizontalem Boden sind damit maximal
etwa 1,73-mal so lang wie der Schattenwerfer. Azimut, Tagesuhr, sichtbare
Himmelskoerper und Mittagsbeleuchtung bleiben erhalten. Die Atmosphaeren-
Beleuchtung folgt ebenfalls dieser kuenstlerischen Lichtapproximation; es ist
kein physikalisch exaktes Sonnen-/Schattenmodell. Keine weiteren Lichtquellen
oder Renderpaesse. Nacht-Ambient und Belichtung aus Phase 40 bleiben erhalten.

Die Anbauten grosser Pilze waren an die feine Blockoberflaeche gekoppelt. Deren
Vorlagen fallen bereits nach etwa 48 Metern weg, deutlich vor dem Terrain-LOD.
Pilzrelief wird jetzt unabhaengig davon in den bestehenden Section-/Material-
Batches erzeugt: volle Details in Near, ein identischer erhaltener Detailkoerper
pro ausgewaehlter Flaeche in Middle, Basisgeometrie in Far. Standardgrenzen
144/240 Meter plus Hysterese; die vorhandene LOD-Einstellung skaliert sie weiter.
Grosse Seitenkoepfe bleiben logische Terrainbloecke. Keine Einzel-Entities oder
globale Erhoehung der Zellauflosung. Zusaetzliche Ferngeometrie ist nicht
kostenlos; allgemeine FPS-/Speicherparitaet wird nicht behauptet.

Der Ladebildschirm zaehlt horizontale Chunk-Spalten anstelle wechselnder CPU-
Sections/Mesh-Zaehler. Eine Spalte gilt erst nach allen angeforderten vertikalen
Sections und ihren Terrain-Uploads als fertig. Gesamtzahl kommt aus dem echten
Streaming-Interesse, nicht aus einer festen Beispielzahl. Anzeige maximal alle
100 ms aktualisiert. Der Einstieg wartet zusaetzlich auf Vegetation und erste
Wasserdarstellungen; laufende Fluidepochen blockieren ihn nicht dauerhaft.
`N / N chunks` kann kurz stehenbleiben, waehrend die letzten Pflanzen-/Wasser-
Uploads fertig werden. Verbindungs-/Fehlerzustaende bleiben eigenstaendig.

Die vorhandene `ui_scale`-Konfiguration ist jetzt direkt oben im Graphics-Tab
erreichbar. Bereich 75-150 Prozent mit sofortiger Entwurfsvorschau; erst Apply
speichert `configs/graphics.toml`. Verwerfen stellt den angewendeten Wert wieder
her. Reset und 15-s-Display-Rollback benutzen denselben bestehenden Ablauf.
Die Skalierung wirkt relativ zur automatisch eingepassten 100-Prozent-Basis,
statt kleinere Fenster immer auf dieselbe absolute Grenze zu kappen. Dadurch
wirken 75/90/100 Prozent auch bei 150-Prozent-DPI. Fuer Bedienbarkeit bleiben
mindestens 900x640 virtuelle UI-Pixel frei; notwendige Begrenzung wird relativ
im Label angegeben, beispielsweise `150% (fit 116%)`. Groessere Fenster stellen
das gespeicherte Wunschmass wieder her. Keine zweite Konfiguration oder UI-
Pipeline.

## Reproduktion

Isolierte Configs und temporaere Welten, keine Aenderung der Benutzerwelten:

```bash
env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra-phase41-lighting-menu \
  TERRA_CAPTURE_DIR=tools/benchmarks/results/menu-lighting-lod-2026-10-17/lighting \
  target/debug/terra-client --smoke-test-daylight
```

Die Fixture ergaenzt 17:00, 17:30 und 18:00 zu den vorhandenen Ansichten.
Pilzpruefung: `--smoke-test-biomes`, `TERRA_BIOME_SCENE=mushroom`,
`TERRA_MUSHROOM_LOD_DISTANCE=96` und eigenes Capture-Verzeichnis.
Menue-/Sessionpruefung: `--smoke-test-menu-session`, frisches
`TERRA_MENU_SMOKE_DIR` und `TERRA_MENU_SCREENSHOTS` fuer Aufnahmen. Dabei wird
der echte TiltUI-Scale-Regler auf 90 Prozent gestellt und seine Vorschau sowie
TOML-Speicherung geprueft. Eine temporaere Welt wird erstellt, gestartet,
veraendert, gespeichert und erneut geladen.

## Validierung

Alle fuenf Cargo-Pruefungen erfolgreich, Builds mit `CARGO_BUILD_JOBS=2`:
`cargo fmt --all --check`, `cargo check --workspace`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo build --workspace`. 643 Tests bestanden, ein vorhandener Langzeittest
ignoriert. Neue Regressionen pruefen Schattenlaenge/Normalisierung/Azimut,
LOD-Distanzen an negativen Koordinaten, Pilzgeometrie ohne Micro-Template,
reduziertes Middle und geschlossenes Far, vollstaendige Chunk-Spalten,
initiales Wasser bei laufenden Fluidepochen sowie UI-Vorschau, Apply, Discard,
Reset, Reload, echtes DPI-Override und Fensteranpassung.

Native Vulkan-Pruefung auf AMD Radeon RX 6950 XT / RADV erfolgreich:

- 15 Beleuchtungs-/Unterwasseraufnahmen; 17:00, 17:30, 18:00 und 19:00
  geoeffnet. Um 17 Uhr gut lesbares Terrain und begrenzte Schlagschatten,
  Nacht bleibt erkennbar Nacht. Mittags-/Unterwasseransichten weiterhin gueltig.
- Zwei echte generierte Pilzparkansichten, Uebersicht mit 96 Metern Abstand
  zum Strukturanker und Nahansicht. Beide geoeffnet, Anbauten vorhanden.
  Middle-/Far-Geometrie zusaetzlich durch CPU-Regression geprueft, nicht als
  separate native Fernaufnahme ausgegeben.
- Vollstaendiger Menue-/Sessionlauf bei 1280x800: echter TiltUI-Scale-Regler,
  90-Prozent-Vorschau, TOML-Reload, Welterstellung, initiale Chunk-Anzeige,
  Integrated Server, Bewegung, autoritativer Blockedit, Save und Weltneustart.
  Ladebild geoeffnet: `22 / 289 chunks`, ein Zaehler, kein Mesh-/Sectionwechsel.
- Zusaetzlicher Menue-Lebenszyklus bei 960x640; Grafikansichten in beiden
  Groessen geoeffnet. 90 Prozent verkleinert die UI sichtbar, Inhalte bleiben
  lesbar und Listen scrollbar. 150-Prozent-Grenze durch automatisierten
  Window-/DPI-Test abgedeckt, nicht separat visuell abgenommen.
- Dedicated Server ohne DISPLAY und WAYLAND_DISPLAY gestartet, 27 Sections,
  erfolgreicher Headless-Smoke-Test.

Ein erster nativer Lauf fand `white-space` als nicht unterstuetzte TiltUI-CSS-
Eigenschaft; entfernt und die nativen Menue-/Lichtlaeufe wiederholt. Ausserdem
deckte die erste 90-Prozent-Aufnahme den bisherigen wirkungslosen absoluten
Fit-Cap auf; die relative Anpassung oben behebt ihn. Finale Laufzeitlogs ohne
CSS-/Shaderfehler. Logs `terra-phase41-*.log` liegen neben diesem Bericht,
Aufnahmen unter `lighting/`, `mushroom/`, `menu/` und `menu-small/`.

Keine FPS-/RAM-/VRAM-Vorher-/Nachhermessung in dieser Etappe. Insbesondere
groessere Pilzdetailreichweite ist kein kostenloser Effekt. Langzeit-/Mehr-GPU-
Streamingmessungen bleiben im TODO-Backlog. Seed, Welt-/Block-IDs,
Generatorversion und Benutzereinstellungen unveraendert.
