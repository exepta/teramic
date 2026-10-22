# Phase 61: Fackeln Und Gemeinsame Inventare

## Implementierung

- Bestehende GLTF-Fackel, Section-Batches und Flame-Vertexshader erweitert:
  kleine getrennte Zungen, verwurzelte UV1-Gewichte, animierte Hoehe/Biegung
  und leichte Farbvariation. Kein eigener Renderer und keine Entity je Flamme.
- `MaterialId::Torch.is_solid()` ist false. Authored ShapeBoxes bleiben fuer
  exaktes Picking erhalten, werden aber nicht in Bewegungscollider eingebaut.
- Licht-Edits umgehen die 100-ms-Auswahlpause. Erstentdeckung ist auf 128
  unveraenderte Sections je Auswahl begrenzt; bereits gecachte und editierte
  Sections reagieren ohne Warten auf asynchrone Meshes.
- Kein fester 48-/64-m-Abstand mehr. Geladene Sections, Sichtlinien ausserhalb
  der direkten Lichtreichweite und das bestehende 4/8/12/16-Lichtbudget bleiben.
  Das ist keine unlimitierte Lichtgarantie bei hunderten benachbarten Fackeln.
- 20000 Basis-Lumen, 18 m Radius, staerkerer sanfter Flicker/Positionsversatz,
  kleiner schattenloser Traeger-Fill. Belichtungskompensation stellt aehnliche
  Spielhelligkeit innen/aussen her, statt reale gleiche Lumen trotz verschiedener
  Kamera-EV als spielerisch gleich hell auszugeben. Licht faellt zum Rand ab.
- Eigene aktive Fackel und bis drei nahe entfernte getragene Fackeln erzeugen
  bewegte Lichter. Fremde aktive Slots sind autoritativ als Bool repliziert;
  kein privates Inventar wird dazu verbreitet. Nur die eigene getragene Lampe
  nutzt zusaetzliche Schattenmaps, entfernte sind schattenlos budgetiert.
- Eigenes leises Feuer-Foley im vorhandenen koerperbasierten Mixer mit
  Abstand/Pan/Verdeckung/Unterwasserdumpfung und maximal drei nahen Quellen.
  Quelle und reproduzierbarer Generator stehen in `assets/audio/CREDITS.md`.

## Population

`natural_mob_limit` bleibt derselbe gespeicherte Wert/Slider/Serveroverride,
zaehlt aber lebende unbesessene Tiere innerhalb 150 m um den gerade beprobten
Spieler. Ein fern gespeicherter Tierbestand blockiert neue Gegenden nicht mehr.
Nahe Spieler teilen dieselben Tiere, es wird kein eigener Bestand je Client
erzeugt. Ein kleineres Limit oder zusammenlaufende Herden loeschen keine Tiere.
Globale 16384er-Sicherheitsgrenze, persistente Todes-Tombstones und begrenzte
KI-/Clientbudgets bleiben. Despawn/Respawn, echte Zaehmung und Paging sind
weiterhin offen. Bienen akzeptieren Forest, Plains und Highland, brauchen
weiter einen echten Baum und behalten Nestdichte, IDs und Speicherung.

## Multiplayer Und UI

`configs/multiplayer.toml`, beide Anzeigen default true, Farbe default None
(stabile gesaettigte UUID-Farbe). Eigene Farben sind validierte RGB-Werte,
keine Graustufen. Bestehendes Apply/Reset/Discard und Display-Rollback erweitert;
kein sofortiges Uebernehmen ungespeicherter Optionen.

Namensschilder sind projizierte TiltUI-Elemente mit weissem Text, Schatten und
transparentem Hintergrund, keine Bevy-UI-Ersatzkomponenten. Geteilte Cursor
verwenden gepackte Slot-/Innerslot-Koordinaten und denselben 3D-Itempreview-Pool.
Dadurch bleibt auch zwischen sechsspaltiger Werkbankansicht und neunspaltiger
Truhe bei anderer DPI-/UI-Skalierung der wirklich gemeinte Slot erhalten.
Server akzeptiert nur die wirklich geoeffnete Truhe; Name und Cursorstack
werden aus der Sitzung gelesen. Empfaenger muessen dieselbe Truhe offen haben.
Maximal 10 Hz, ruhend 400-ms-Lebenszeichen, 800-ms-Serverablauf; bis 32 fremde
Cursor pro Betrachter. Namensroster nur bei Aenderungen, kein staendiger
Cursorverkehr ohne geteilten Cursor. Noch keine Cursorinterpolation oder
gesonderte Namensschild-Verdeckungspruefung.

Chat ist waehrend Inventarzugriff passiv sichtbar, ohne dem Suchfeld Befehls-
eingaben wegzunehmen. Lebensanzeigen bleiben sichtbar, Debug liegt darueber.
Inventar zeigt vor materialisierten Slots und zwei stabilen Layoutframes keine
unangeordnete Flaeche. Truhe/Werkbank warten ausserdem auf ihre autoritative
Oeffnungsantwort; davor erscheint nicht kurz das normale Spielerinventar.
Der aktive Slot wird auch ohne Abbau-Intent direkt uebertragen, damit getragene
Lichter auf fremden Clients ohne einen Abbauklick wechseln. Ein Render-Guard
entfernt Bevy-0.19-Hintergrund-Motion-Pipelines nach Abschalten des Motion-
Prepasses; sonst konnte der echte TAA->Low-Wechsel einen unpassenden MRT-Pass
binden und den Client beenden. Weder TAA noch die Schattenoption werden pauschal
abgeschaltet. Der Farbwaehler benutzt oeffentliche TiltUI-ControlParts, dunkle
Styles und einen Clip-Override fuer seine Palette.

Netzwerkprotokoll 35 erfordert gemeinsamen Client-/Server-Neustart. Welt-/Block-
IDs, Seed, Generator und REG9-/PLY3-Formate bleiben unveraendert.

## Abnahme

Erfolgreich auf dem finalen Code:

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo test --workspace --no-fail-fast`: **964 bestanden**, ein bestehender
  dreiminuetiger Soak ignoriert; dieser wurde in Phase 61 nicht erneut ausgefuehrt.
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --workspace`

Neue Regressionstests pruefen unter anderem lokale Zaehler mit fernen/besessenen/
toten Tieren, zulaessige Bienenbiome, physisch durchlaessige aber anwaehlbare
Fackeln, Belichtungsfaktor, Konfigurations-Apply/Reset/Reload, passiven Chat und
Lebensanzeigen im Inventar, Layout-/Serverantwort-Gates, Spalten-/DPI-unabhaengige
Cursorzuordnung und den TAA-Motion-Pipelineabbau.

Ein echter Zwei-Client-UDP-Test prueft Slotwechsel ohne Abbau, den replizierten
Held-Torch-Status, Namensroster, Cursorfreigabe nur fuer dieselbe offene Truhe,
autoritative Cursoritems, Schliessen und gefaelschte Anker. Die Erweiterung
deckte die bisher an Abbau-Intents gekoppelte Slotuebertragung auf; behoben.

Native Vulkan/Wayland-Pruefung auf RX 6950 XT, RADV/Mesa 26.2.4:

- Zwoelf erfolgreiche Hoehlen-/Fackelansichten in `torches/`: alle vier
  Hoehlenqualitaeten, Boden-/Wandfackel, Nacht aussen mit EV 10.7 und Caveblend 0,
  Erhalt der geladenen Lichtquelle jenseits 50 m sowie replizierte getragene
  Fackel. Testgrund und Caches werden vor Generierung konsistent konfiguriert;
  nachtraegliches Austauschen nur des Testgenerators wurde verworfen.
- Vollstaendiger Menue-Smoke erfolgreich, inklusive Multiplayer-Optionen,
  realem TiltUI-Colorpicker, TOML-Reload und Display-Timeout. PNGs in `menu/`;
  `multiplayer-color-palette-open.png` zeigt die vollstaendige dunkle Palette.
  Der native TAA->Low-Absturz wurde dabei gefunden und gezielt behoben.
- Zwei native Clients mit verschiedenen echten lokalen Profilen an einem
  eigenen Dedicated Server erfolgreich. `player-names.png` zeigt einen echten
  Remoteavatar mit serverseitigem Namen; Bewegung, Replikation und Bilder
  bestanden. Flache Testwelt, Seed 42042, keine Produktionswelt veraendert.
- Dedicated ohne DISPLAY/WAYLAND_DISPLAY gestartet, 27 Sections im kurzen
  Headless-Test; eigener zusaetzlicher Zwei-Client-Testserver danach beendet.
- Inventar-Startansicht aufgenommen und angesehen (`inventory/inventory-open.png`).
  Der vollstaendige native uinput-Hoverlauf ist **nicht bestanden/abgenommen**:
  dem Fenster fehlte die native Zeigerposition (`pointer=None`). Interaktions-
  und Layouttests sind gruen, ersetzen diesen Desktop-Maustest aber nicht.

Bilder wurden stichprobenartig angesehen. Logs sind als `terra61-*.log` hier
abgelegt. Die Fehlversuche vor den Korrekturen bleiben im temporaeren Logsatz;
die `*-verified.log`-Dateien enthalten die erfolgreichen finalen Laeufe.
Kein Audioausgabegeraet vorhanden: Dekodierung getestet, keine behauptete
menschliche Klangabnahme.

## Performancegrenzen

Kein neues kontrolliertes Vorher-/Nachher-FPS- oder GPU-Messpaar. Die kurzen
Showcase-Framesamples sind Funktionsproben, teils parallel zu Kompilierung,
keine Performanceabnahme. CPU-/GPU-/Speicherparitaet wird nicht behauptet.
Lange bewegte Mehrspielerlaeufe mit vielen Fackeln, ueberlappenden Populationen,
Cursorlast, weiteren GPUs und echtem Desktop-Picking bleiben offen in TODO.md.
