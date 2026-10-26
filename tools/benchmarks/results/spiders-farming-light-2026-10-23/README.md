# Spiders Farming And Cave Lighting

Phase 111 korrigiert Aussaat, Pflanzenanzeige, Kartenhoehen und Hoehlenlicht.
Spinnen, verankerte Netze und Schwimmspuren erweitern die vorhandenen Systeme.
Protokoll 63 erfordert gemeinsam aktualisierte Clients und Server. Weltformat
2, Generator 5, PLY7, RGA2 und vorhandene Block-/Prop-/Mob-Tags bleiben erhalten;
neue Tags sind angehaengt. Keine Dependency-Upgrades.

## Pflanzen Und Karten

Bereits vorhandener Ackerboden wird bei der ersten Aussaat korrekt im
Pflanzjournal verankert. Wiederholtes Pflanzen erzeugt keine zusaetzlichen
Terrain-Revisionen. Ueberhaengende Kulturpflanzen versperren die Bodenauswahl
nicht mehr; Waende und andere Dekorationen bleiben Hindernisse. Inventarquelle,
Reichweite, freie Hoehe, belegter Boden und Epochen werden weiterhin geprueft.
Gedruecktes Abbauen verwendet auch mit Samen in der Hand die normale
Pflanzenauswahl, statt durch die Ernte hindurch auf den Boden zu zielen.

Die Ackerboden-HUD zeigt den Fortschritt der darauf gesaeten Kultur als echten
Balken. Unbesetzter Boden zeigt `Keine Samen erkannt`, auch wenn noch ein
veraltetes Fortschrittspaket vorliegt. Ein Pflanzen-Stufenwechsel ueberblendet
2,4 Sekunden lang alte und neue Geometrie mit komplementaerer Rasterdeckung
und wurzelfester Groesseninterpolation. Farbe und Tiefenpass verwenden denselben
Uebergang. Beide Stufen bleiben in einem Section-Batch; die alte Geometrie wird
anschliessend budgetiert entfernt. Keine transparente Entity pro Pflanze und
keine Materialkopie pro Frame. Gespeicherte Kulturen wachsen beim Reload nicht
erneut von Null an.
Die Auswahlumrandung wartet den kurzen Pflanzenuebergang ab, damit kein
schwarzes Gitter der zukuenftigen Geometrie durch die Rasterdeckung sichtbar
wird. Logische Zielauswahl und Ernte bleiben dabei unveraendert aktiv.

Die Minimap folgt der Hoehe des kartierten Bodens, nicht der Sprung-/Flughoehe
oder dem Hoehenbezug der ersten geladenen Position. Die FullMap behaelt beim
Verschieben ihren eigenen Bezug. Gecachte Marker haben einen getrennten
Hoehenbezug und bleiben zwischen den FPS-begrenzten Updates korrekt ausgerichtet.

## Spinnen

Die Koerpermasse verstehen sich ohne Beine:

| Art | Breite | Hoehe | Laenge | Verhalten | Gift |
| --- | ---: | ---: | ---: | --- | --- |
| Small Spider | 0,145 m | 0,1 m | 0,195 m | Neutral, versteckt sich an Baeumen/Boden | Stufe 2, 16 s |
| Medium Spider | 0,645 m | 0,5 m | 0,695 m | Aggressiv, nachts draussen, sonst obere Hoehlen | Stufe 1, 25 s |
| Large Spider | 1,345 m | 1,25 m | 1,895 m | Aggressiv, Hoehlen ab Y -22 abwaerts | Stufe 3, 5 s |

Die grosse Spinne verursacht 5,5 Schaden je Treffer. Schaden und Gift verwenden
die bestehende serverseitige Kampf-/Effektlogik mit Abstand, Sicht und
Abklingzeit. JSON-Definitionen liegen einzeln unter `assets/terra/mobs`.
Zum gezielten Testen dienen `/spawn teramic::small_spider`,
`/spawn teramic::medium_spider` und `/spawn teramic::large_spider`.

Die Simulation prueft echte Boden-, Wand- und Deckenformen, geht an Kanten in
neue Anheftungen ueber und laesst Tiere nach entferntem Halt fallen. Abseilen
hat einen wirklichen Deckenanker und sichtbaren Faden. Die Modelle verwenden
acht bewegte Beine, Mandibeln, dunkle blockige Panzer mit rotorangen Baendern
und rote Augen. Augen besitzen keine eigenen PointLights. Kleine Spinnen
wehren sich erst nach Provokation. Spawnabfragen und Nachbarschaftssuchen
bleiben begrenzt und zaehlen zum bestehenden natuerlichen Mob-Limit.
Mittlere Spinnen suchen tagsueber echte Hoehlendeckung oder Schatten. Verlorene
Deckung wird neu gesucht; nachts oder bei Kampf werden alte Deckungsrouten
freigegeben. Laufender Kampf verbraucht nicht jeden Tick neue Suchkontingente.

## Netze

Der angehaengte `SpiderWeb`-Prop besitzt zehn Konstruktionen: Wandflaeche,
Ecke, Deckennetz, Kokon, Durchgang, Bodennetz, dichtes Hoehlennetz, altes Netz,
Eiersaecke und Waldnetz. Drei LODs und gemeinsame Section-Batches verwenden
solide stilisierte Seidenstraenge statt einer einzelnen flachen Textur.

Generierte Netze benoetigen passende reale Anker und sind auf zwei pro
Section begrenzt. Spinnen koennen persistente Netze bauen, mit maximal vier
lebenden Netzen pro Section. Wasser und ungeladene Anker werden abgewiesen.
Netze sind auswaehlbar/abbaubar und im Creative an Boden, Wand und Decke
platzierbar; entfernte Props bleiben beim Reload entfernt.

## Hoehlen Und Wasser

Geschlossene Hoehlen schalten direktes Sonnen-/Mondlicht ab. Tauchen stellt
nicht mehr die helle Aussen-Ambientbeleuchtung her; solare Unterwasserstreuung
und Kaustik bleiben in geschlossenen Hoehlen aus. Das Hoehlen-Hintergrundlicht
ist um 35 Prozent reduziert, bei unveraenderter Lesbarkeit lokaler Fackeln.
Eine stetige Sonnen-/Mondueberblendung verhindert die kurze Helligkeitsluecke
an Daemmerungsgrenzen.

Wasser verwendet groessere gerichtete Shader-Spuren und Schaumschleppen mit
maximal 16 aktiven Stroemungsimpulsen und 3,2 Sekunden Ablaufzeit. Die bestehende
Wassermaterial-/Simulationspipeline bleibt erhalten. Vier weichere Schwimm-
Foley-Varianten basieren auf bereits vorhandenen CC0-Wasseraufnahmen. Spinnen
erhalten Schritte, dezentes Idle und ein Fauchen fuer die grosse Art; Quellen
und Bearbeitung stehen in `assets/audio/CREDITS.md`.
Der Shader animiert Spuren mit der globalen Bevy-Uhr: reine Zeitfortschritte
erzeugen keine neuen Wassermaterial-Uploads. Zeitstempel bleiben ueber den
Uhrenumbruch hinweg korrekt.

## Offene Erweiterungen

Rohre aus Holz/Stein/Eisen, Ventile und Ein-/Ausgaenge (F27), anschliessbare
Wasserhaehne (F28) und ruhende 2x2-Becken mit Quellenbildung (F29) sind wie
gewuenscht in TODO.md eingeordnet, hier nicht als umgesetzt dargestellt.
Die Modelle folgen der Referenz mit wiederverwendbarer Voxelgeometrie;
pixelidentische Illustrationsqualitaet wird nicht behauptet. Eine menschliche
Audio-Abnahme sowie plattformuebergreifende FPS-/Speicherparitaet bleiben
separate Abnahmen. Keine neuen 10-/15-/30-Minuten-Laeufe.
Die Sonnen-/Mondabschirmung nutzt die vorhandene kameraweite Hoehlenprobe,
keinen raeumlichen Lichttransport. Dadurch kann direktes Aussenlicht auch auf
Gelaende hinter einem vollstaendig ueberdachten Eingang abgeschwaecht werden.

## Verifikation

Alle folgenden Cargo-Pruefungen wurden mit dem aktuellen Quellstand ausgefuehrt,
ohne parallele Cargo-Prozesse und mit `CARGO_BUILD_JOBS=1`:

| Pruefung | Ergebnis |
| --- | --- |
| `cargo fmt --all --check` | Erfolgreich |
| `cargo check --locked --workspace` | Erfolgreich |
| `cargo test --locked --workspace --no-fail-fast` | 1981 erfolgreich, 0 fehlgeschlagen, 3 ignoriert |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Erfolgreich, keine Warnungen |
| `cargo build --locked --workspace` | Client und Dedicated Server erfolgreich |
| `terra-server --smoke-test --no-save` | Headless erfolgreich, 27 Sections |

Logs liegen unter [logs](logs). Die Regressionen pruefen unter anderem:

- Vorbereiteten Ackerboden, negative Koordinaten, Section-Raender, wiederholte
  Aussaat ohne zusaetzliches Remeshing und autoritativen Inventarverbrauch.
- Aussaat durch ueberhaengende Blaetter, normale Ernte mit Samen in der Hand,
  leere/veraltete Pflanzenanzeigen und alle acht Stufen der drei Kulturarten.
- Wachstums-/LOD-Kontinuitaet, Ablauf alter Fade-Geometrie und keine vorzeitige
  Auswahlumrandung waehrend eines Pflanzenuebergangs.
- Kartenhoehen, Rotation und getrennte Hoehenbezuege gecachter Overlays.
- Spinnengroessen, freie Augenflaechen mittels Dreiecks-Raycasts, Anheftung,
  Abseilen, entfernten Halt, Tagesverstecke und unerreichbare Verstecke.
- Natuerliche Spawn-/Tiefengrenzen, Neutralitaet, Gift, Reichweite, Sichtlinie,
  Angriffspausen, Suchbudgets sowie persistente Posen und Netze.
- Netzanker und echten seitlichen Freiraum, Flutentfernung und Tombstones.
- Geschlossene Hoehlen ohne direktes Himmelslicht, dunkles Hoehlenwasser,
  stetige Lichtuebergabe und unveraenderte belichtungsnormierte Fackelenergie.
- Wasser-Spuruhr ueber Zeitumbrueche: 64 reine Zeitfortschritte mit aktiver
  Spur erzeugen keine `AssetEvent::Modified`-Meldung und kein Asset-Wachstum.

## Native Sichtpruefung

Tatsaechlich auf der RX 6950 XT mit RADV/Vulkan ausgefuehrt:

- `--smoke-test-farming`: erfolgreicher echter Survival-Zyklus einschliesslich
  Weizen/Mais/Salat, Ernte, Werkzeugen, Wachstum-HUD und Save/Reload.
  11 Aufnahmen, 29 gespeicherte Kulturen, 10 erkannte GPU-Wachstumsuebergaenge.
  `TERRA_FARM_GROWTH_INSPECT=1` ergaenzt einen ausdruecklich rein visuellen
  Stufen-Replay nach dem echten Zyklus; dieser veraendert die Autoritaet nicht.
- `--smoke-test-spiders`: neun erfolgreiche Modell-/Netzaufnahmen. Vorder-/
  Seitenansichten aller drei Groessen, Wand-/Decken-/Abseilposen und zehn
  Netzvarianten. Dieser Galerielauf ist kein In-Game-Nachweis der Kampf-KI;
  deren Regeln werden durch die Welt-/Netzwerktests abgesichert.
- `TERRA_CAVE_LIGHT_INSPECT=1 --smoke-test-caves`: vier erfolgreiche Aufnahmen
  der Stufen 14 bis 17, jeweils Qualitaet Medium: pausierter/laufender Tag und
  echtes Untertauchen bei Tag/Nacht. Geschlossene Hoehle jeweils Blend 1,
  Kamera-EV 6,621488, kein direktes Welt-Sonnen-/Mondlicht; Unterwasserblend 1
  in beiden Tauchaufnahmen. Isoliertes Inventar-Vorschaulicht wird korrekt
  getrennt gemessen. Lesbarkeits-/Ueberbelichtungspruefungen bestanden.
- `--smoke-test-minimap`: alle 52 Schritte erfolgreich. Echter zweiter Peer,
  Mobmarker, FPS-Presets, Iconskalierung, Hoehlenteleport, gehaltenes Ziehen,
  Zoom, Zentrierung, Waypoint-CRUD und TOML-Reload wurden geprueft. Reale
  gehaltene Flugsteuerung bewegte den Spieler 131,62 m mit 55,15 m
  Hoehenaenderung und drei nichtleeren Kartenaufnahmen. HUD-Overlays bleiben
  erhalten; Kartensymbole, Listensymbole und Welt-HUD-Symbole werden getrennt
  auf ihre richtigen Groessen geprueft.

Ausgewaehlte tatsaechlich gesichtete Aufnahmen:

- [Leerer Ackerboden](native/farming/farming-empty-farmland-hud.png)
- [Wachstum Start](native/farming/farming-grow-start.png),
  [Mitte](native/farming/farming-grow-middle.png),
  [Abschluss](native/farming/farming-grow-complete.png)
- [Mittlere Spinne](native/spiders/spiders-2.png),
  [Netzkonstruktionen](native/spiders/spiders-8.png)
- [Hoehlenwasser tagsueber](native/caves/cave-16-Medium.png),
  [nachts](native/caves/cave-17-Medium.png)
- [Minimap nach echtem Flug](native/maps/map-23.png)

Der Kartenlauf meldete vier nicht-fatale Wayland-Fehler beim kuenstlichen
Setzen der Cursorposition eines nicht gesperrten Testfensters. Die separaten
logischen Eingabe-/Drag-Pruefungen bestanden. Diese Plattformgrenze des
Testharness wird nicht als behobener Spielfehler ausgegeben. In den aktuellen
Farming-, Spinnen-, Hoehlen- und Serverlaeufen gab es keine Warnungen/Fehler;
auch kein Shaderfehler oder Slab-Allocator-Use-after-free wurde beobachtet.

Keine allgemeine FPS-Vorher-/Nachher-Aussage: Diese kurzen Funktionsszenen
ersetzen keinen repraesentativen Performance-Benchmark. Die wenigen im
Hoehlenlog enthaltenen Frame-Samples liefen teilweise waehrend Rust-Builds.
Es wurden wie gewuenscht keine neuen 10-/15-/30-Minuten-Soaktests gestartet.
