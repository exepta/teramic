# Phase 107: Handkurbel, Ruecken-Capes Und Inventar-Korrekturen

## Umsetzung

- Cape-Stoff nur auf dem Ruecken, kein Frontschal und keine Frontschuerze.
  Zwei verbundene Gelenke reagieren geglaettet auf Laufgeschwindigkeit und
  den vorhandenen Wetterwind. Vorwaertslaufen zieht den Saum deutlich nach
  hinten; Seitenwind wirkt auch im Stand. Kein Cloth-Solver und keine neuen
  Meshes pro Frame, weiterhin drei gebuendelte Teile pro Cape-Variante.
- Crusher-Modell mit kleineren Trichtern, warmdunklem geschmiedetem Metall,
  gemalter Detailtextur, Zahnwalzen, Holzmaserung, segmentierten Flaechen und
  Nieten. Reproduzierbar mit `tools/assets/build_crusher.mjs`.
- Handkurbel als partieller Strukturblock 100, eigenes Rezept und Modell,
  rechts am Zahnrad montiert. Beide angeklickten Crusherhaelften werden auf
  denselben primaeren Anker aufgeloest; falsche Seiten und Treffer ausserhalb
  der Anschlussstelle werden abgewiesen. Keine Bodenstuetze erforderlich.
- Items zuerst in den Crusher legen, Inventar schliessen, dann an der Kurbel
  Platzieren/Benutzen halten. Loslassen, Wegsehen, Menues, Tod, Teleport,
  Spectator und fehlende/entfernte Kurbel stoppen den Antrieb. Shift behaelt
  normale angrenzende Platzierung bei. Kurbeln verriegelt weder Kamera noch
  Bewegung und platziert/isst nicht nebenbei das gehaltene Item.
- Leeres Kurbeln dreht die Mechanik und spielt den mechanischen Anteil;
  nur verarbeitbare Eingaben erzeugen Knirschen und fallende Spaene. Blockierte
  Ausgabe verbraucht keine Zutaten. Alle bisherigen 16 Rezepte und TMI-Plus-
  Transfers bleiben erhalten, aber es gibt keinen automatischen Fortschritt.
- Player-Inventar hoeher, zusaetzliche kompakte Hoehenregeln. Slot-Tooltips
  bleiben persistent und sind bei leerem Slot oder gehaltenem Cursoritem
  deaktiviert. Tooltip-Hover wird aus dem aktuellen Picking-Zustand
  synchronisiert, statt ein neues Over-Ereignis oder einen Klick zu verlangen.
  Monitor-DPI und UiScale werden getrennt behandelt; beide Follow-Placer
  verwenden identische Werte und bleiben im Idle unveraendert. Portalisierte
  Tooltips aktualisieren geerbte Opazitaet aus ihrem echten Elternteil:
  Hintergrund/Text behalten keine Alpha 0 des anfangs versteckten Inventars.
  Urspruengliche CSS-Farben und lokale Alpha-Werte bleiben erhalten.
- Ruestungsrahmen maskiert die fremden HP-Rahmenpixel seines Atlas-Ausschnitts.
  Shield Power liegt als schmales blaues Inlay innerhalb der Ruestungsleiste,
  ohne ueberstehenden zweiten Rahmen; bei null Punkten bleibt nur die Effektcard.
  Gift besitzt bewegliche Rinnsale und zeitversetzte, fallende Pixeltropfen
  im vorhandenen HUD-Shader; weiterhin nur das erste Effektsymbol auf Herz/HP.

## Budgets Und Kompatibilitaet

Ein gemeinsamer Stereo-Audiostream mischt maximal acht nahe Maschinen bis
24 m mit Entfernung/Panorama und Master-/Ambience-/Mute-Einstellungen.
Keine Audio-Entity pro Span oder Maschine. Neun gemeinsame Mechanikmeshes
fuer Crusher und Kurbel; Partikel bleiben auf acht Spaene an maximal 16
arbeitenden Maschinen begrenzt.
Die Schallquelle folgt der ausgerichteten Maschinenmitte. Mute erreicht
innerhalb von maximal 75 ms plus einem 256-Sample-Steuerintervall echte
Nullsamples, statt einen unbegrenzt auslaufenden exponentiellen Rest zu behalten.

Der Client sendet gehaltene Eingabe mit 10 Hz und eine sofortige Freigabe.
Der Server prueft echten Sichtstrahl/Reichweite/Berechtigung und begrenzt
Impulse auf 20 Hz. Eine 350-ms-Lease laeuft auch bei fehlendem Release ab.
Mehrere Spieler beschleunigen dieselbe Maschine nicht mehrfach. Bestehende
32-Stationen-/125-ms-Arbeitsbudgets und begrenzte Nachholzeit bleiben bestehen.

Protokoll 59 erfordert gemeinsames Aktualisieren von Client und Server.
Die Kurbelbindung benutzt vorhandene Objektmetadaten mit materialabhaengiger
Validierung. PLY7, RGA1, Generator 5, Weltformat 2 und bestehende IDs bleiben
unveraendert. Gespeicherte Crusher bekommen keine automatisch montierte
Kurbel; sie warten auf Montage und manuelle Bedienung. Drive-Leases und
Antriebszustand werden weder gespeichert noch als Terrain-Edits repliziert.
Liegt eine montierte Kurbel in einer noch ungeladenen Nachbarsection, wird
Crusher-Abbau vor jeder Mutation abgewiesen. Nach Laden wird sie atomar mit
dem Crusher abgebaut; Inhalte und Kurbel droppen genau einmal.

## Pruefungen

- `cargo fmt --all --check`, `cargo check --locked --workspace --all-targets`,
  `cargo clippy --locked --workspace --all-targets -- -D warnings` und
  `cargo build --locked --workspace` bestanden.
- `cargo test --locked --workspace --no-fail-fast`: 1873 bestanden,
  zwei bestehende Tests ignoriert. Kurbelbindung/Abbau, ungeladene Anhaenge,
  Persistenz, echte UDP-Bedienung, Stillstand ohne Fahrer, Release-Pause,
  Eingabebindung, Cape-Geometrie/Wind und begrenzte Caches sind abgedeckt.
- `cargo test --locked -p tilt-ui-runtime --lib`: 315 bestanden, einschliesslich
  bereits stationaerem Hover, Disabled-/Reenable-Verhalten, DPI/UiScale,
  portaliertem Alpha-0-Fehler und unveraendertem Idle nach der Reparatur.
- Headless Dedicated-Smoke mit 27 Sections bestanden.

Zwei kurze echte Vulkan-/UDP-Funktionslaeufe auf der RX 6950 XT:

- `--smoke-test-equipment`: 16 bestaetigte Aufnahmen, alle fuenf Sets,
  weisse/gefaerbte Capes und Ablegen. Zehn Plaetze, Erhaltung von 36 Items
  und maximal 2947 Mesh-Assets im gesamten Fixture-Client.
  Kein Scrollbedarf der Spieler-Inventarflaeche bei den geprueften Groessen,
  auch kompakte Hoehe/Breite und UiScale 0.8/1.5. Tooltippruefung sendet
  echte Picking-Bewegung ohne Klick oder Betriebssystem-Cursorwarp und
  kontrolliert Itemname/ID, gerenderte Glyphen, Textalpha, geerbte Sichtbarkeit,
  exakten Abstand, Bildschirmgrenzen und kontrastreiche Screenshotpixel.
  Leerer Cape-Slot bleibt ohne Popup. Die Bildschirmaufnahmen wurden angesehen.
- `--smoke-test-crusher`: echte Survival-Montage, normale konfigurierte
  Platzieren-Eingabe statt direktem Setzen des Antriebszustands. Leeres Kurbeln,
  Rotation, Audio-Sink/RMS, Mute/Unmute, Teilfortschritt/Loslassen/Pause,
  TMI-Plus-Transfer und alle 16 Rezeptausgaben mit Itemerhaltung bestanden.
  Fuenf bestaetigte Aufnahmen, neun gemeinsame Mechanikmeshes, aktive fallende
  Partikel. Zahnradseite und zusaetzliche Schraeg-Frontansicht wurden angesehen.

Beide finalen nativen Logs enthalten keine Warnungen, Renderfehler oder Panics.
Die Fixtures verwenden isolierte Konfigurationspfade und eigene temporaere
Welten; sie veraendern keine Spielstaende. Das sind Funktionspruefungen in
einer flachen Welt ohne natuerliche Mobs, keine FPS-/Streaming-Messpaare.
Die Speicher-/Netzwerktests pruefen Persistenz separat; die GPU-Laeufe sind
kein Neustart-/Reconnect-Test.

Ansichten: [Ruecken-Cape und HUD](native/equipment/armor-ruby-world.png),
[Tooltip bei 150 Prozent UI](native/equipment/armor-iron-inventory.png),
[gefaerbtes Cape im Inventar](native/equipment/armor-ruby-blue-inventory.png),
[Kurbel im Betrieb](native/crusher/crusher-running.png),
[Crusher-Front](native/crusher/crusher-results-world.png).
Unveraenderte Pruefprotokolle liegen unter [logs](logs/).

## Grenzen

Die Modelle verwenden die stilisierte Teramic-Geometrie und bestehende PBR-
Beleuchtung. Eine geometrisch/pixelidentische Kopie der gerenderten Vorlage
wird nicht behauptet. Der Klang ist ein prozeduraler mechanischer Mix, keine
neue Mikrofonaufnahme. Signal-/Mute-Tests ersetzen keine menschliche
Hoerabnahme. Kein Stromnetz, Offlinefortschritt oder Stoffkollisionssystem.
Keine neuen langen Soak-Tests und keine allgemeine FPS-/VRAM-Paritaetsaussage.
