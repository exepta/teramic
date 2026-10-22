# Phase 62: Navigation, Audio Und Inventar

## Umfang

Bestehende autoritative Tier-KI, Water-/Section-Caches, Itemregister,
Preview-/Drop-Batches, TiltUI und Audio-Mixer erweitert. Keine Dependency-
Upgrades und kein eigener Renderer oder zweites Speicher-/Prop-System.
Architektur und konkrete Grenzen stehen im Phase-62-Abschnitt der Root-README.

## Audioquelle

Feuer: PagDev, Fireplace Sound loop, CC0,
https://opengameart.org/content/fireplace-sound-loop

Original SHA256: `85ca0cc60d0c037fff8b185e31ad1fcdbda6ce45eee17c3ee1318d1b8f59e330`.
Verarbeiteter Clip: `cf16ddea1f3a450a52720aebcfc2843c4774bf57ca6d952621789c2827014a5e`.
Filter und Reproduktion: `tools/assets/build62-audio.mjs`, Lizenznachweis in
`assets/audio/CREDITS.md`. Atem-Foley ist eigene Synthese, keine Tieraufnahme.

## Abnahme

Erfolgreich auf dem finalen Stand:

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo test --workspace --no-fail-fast`: **972 bestanden**, ein bestehender
  dreiminuetiger Soak ignoriert. Dieser wurde hier nicht erneut ausgefuehrt.
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --workspace`
- Isoliert `cargo build -p terra-server`: bestanden, auch ohne Feature-Unifikation
  mit dem Client. Dessen eigener Featurebaum enthaelt kein `bevy_render`,
  `bevy_pbr`, `bevy_winit` oder `bevy_audio`.
- Der so gebaute Server startet mit entferntem `DISPLAY`/`WAYLAND_DISPLAY` und
  beendet den Headless-Smoke erfolgreich nach 27 geladenen Sections.

Zusaetzliche Regressionen pruefen Bienenumwege mit dem produktiven Routencache,
unbeladene Barrieren, Abstandshaltung, verschiedene Schlafzeiten, Fluidparitaet
mit negativen Koordinaten und Live-Edits, reine Erzreflexion, alle Produktmeshes,
Cursorinterpolation, gueltige Tilt-Glyphen, Namensreichweiten-Apply/Reload und
Erhalt der unsichtbaren Bilddeckkraft vor stabiler Inventaranordnung.
Reale UDP-Tests pruefen unter anderem 256 Tiere ohne Suchbudget-Verhungern,
maximales Clientinteresse, gemeinsame Truhencursor, autoritative Items und
Schliessen/Verbindungswechsel. Eine seltene TCP-/UDP-Portkollision beim lokalen
Serverstart wurde dabei entdeckt: TCP wird jetzt vorab reserviert und derselbe
Port auf UDP-Verfuegbarkeit geprueft. Bestehende Ports werden nicht uebernommen.

Native Vulkan/Wayland-Pruefung auf RX 6950 XT, RADV/Mesa 26.2.4:

- Zwei echte Profile an eigenem Dedicated Server, erfolgreiche Bewegung und
  Replikation, jeweils 128 replizierte Tiere, kontinuierliche Eingabe-ACKs.
  `network/observer.png` zeigt den serverseitigen Namen am echten Remoteavatar.
  Die beiden selbst beendenden Clients wurden nacheinander gestartet; ihre
  Messintervalle ueberlappen nur teilweise.
- Zwoelf Fackel-/Hoehlenansichten, vier Qualitaeten, Boden-/Wandmontage,
  naechtliche Aussenansicht, Lichtquellenerhalt jenseits 50 m, getragene Lampe.
  `torches/night-before-support-fill.png` dokumentiert den zu dunklen Schatten
  unter dem Halter vor der letzten Nahlichtkorrektur. `cave-9-Medium.png` zeigt
  dieselbe Ansicht danach. Keine Abschaltung der Terrain-/Wandschatten.
- 22 echte GPU-Itemvorschauen in `items/` (13 Tierprodukte und neun Werkzeuge),
  inklusive erhaltener Eisenspitzhacke. `items-contact-sheet.png` ist nur eine
  beschriftete Kontaktkopie dieser Readbacks, kein UI-Mockup oder neues Asset.
  Pro Produkt ein gemeinsamer farbiger Meshbatch, keine Entity je Detailvoxel.
- Vollstaendiger Menue-Smoke, Multiplayer-Icon und 40-m-Regler sichtbar,
  Settings-Speicherung, Reset, Discard, Display-Rollback und Colorpicker.
- **Echter OS-Mauslauf bestanden**, neun Inventaraufnahmen: Creative-Suche,
  Hover/Tooltips, Rechtsklick, Shift/Stacktransfer, MouseTweaks im Craftinggrid,
  gehaltene Vorschau, ausserhalb abgelegter kompletter Stack und Scrollen.
  Der Desktop war entsperrt; das vorhandene temporaere uinput-/KWin-Werkzeug
  bediente nur das eigene Testfenster und wurde danach beendet. Damit ist die
  Phase-61-Einschraenkung `pointer=None` fuer diesen Einzelclientlauf behoben.

Aufnahmen wurden stichprobenartig angesehen. Ein gleichzeitiger nativer
Zwei-Betrachter-Truhencursortest ist trotzdem noch offen; echte UDP-Transport-
tests und der Einzelclient-Mauslauf ersetzen ihn nicht. Keine menschliche
Klangabnahme: weiterhin kein Audioausgabegeraet vorhanden. Dekodierung,
Sampleenergie, Begrenzung und sanftes Abschalten des Feuerbetts sind getestet.
Die erfolgreichen finalen `terra62-*.log`-Dateien liegen neben diesem Bericht.
Fruehe Fehlversuche wurden nicht als bestandene Abnahme gewertet; ihre Logs
bleiben im temporaeren Arbeitsverzeichnis.

## Messungen Und Grenzen

Die Fluidabfrage vergleicht im selben Testprozess 3840 identische Proben in
drei vorbereiteten Spalten, darunter negative Koordinaten. Beide Wege liefern
bitgleich denselben Fuellstand; danach werden Live-Override und feste Barriere
geprueft. Vorbereitung ist nicht in dieser reinen Abfragezeit enthalten.

| CPU-Abfrage | Bisherige Noise-Abfrage | Vorbereitete Spalte |
|---|---:|---:|
| 3840 Fluidzellen | 3.187 ms | 0.127 ms |

Das ist kein Chunkgenerierungs-, GPU- oder allgemeiner FPS-Nachweis. Zusaetzlich
wird dieselbe Zelle innerhalb einer Koerperpruefung nicht mehr zwoelfmal gelesen.
Beide Verbesserungen verwenden bestehende Daten, keinen weiteren Weltcache.

Kurze native Frameproben, **1920 x 1080 tatsaechliche Fensterpixel**, flache
Testwelt mit Gras/Props, Seed 42042, jeweils 128 sichtbare Tiere:

| Client | Samples | FPS | p95 | p99 |
|---|---:|---:|---:|---:|
| Beobachter | 4565 | 162.99 | 8.556 ms | 9.676 ms |
| Begleiter | 7258 | 190.99 | 8.349 ms | 9.391 ms |

Die Intervalle enthalten auch Zeit mit nur einem Client: der Begleiter startet
spaeter und laeuft laenger als der Beobachter. Sie sind **kein reines
Zwei-Client-Benchmarkpaar** und kein kontrollierter Vorher-/Nachher-Vergleich.
Die CLI-Fixtures verwenden ihre Grafikressource; nicht die kleinere
1280-x-720-Angabe der kopierten Konfiguration als tatsaechliche Aufloesung
ausgeben. Waehrend dieser Netzwerkproben lief keine Cargo-Kompilierung.
Hoehlen-Showcase-Samples entstanden dagegen teilweise neben dem Testbuild und
werden nicht als Performanceabnahme verwendet.

Keine neue Aussage zu FPS-/Speicherparitaet, GPU-Zeit, VRAM oder Subdrawzahlen.
Lange dichte Landschafts-/Streaminglaeufe, 4K, mehrere GPUs und hoehere
Spielerzahlen bleiben offen. Mehr als 512 gleichzeitig faellige KI-Updates pro
Takt sind nicht als zeitparitaetisch abgenommen. Lokale Voxel-A* ersetzt keine
weltweite Wegeplanung. Art-Direction und die physikalische PBR-Lichtabnahme
bleiben gemeinsam abzustimmen; 24 m Cullingradius bedeuten nicht 24 m gleich
helle Beleuchtung. Ofen und Honigentnahme bleiben echte Backend-Aufgaben.
