# Phase 112: Bauen, Ackerboden, Hoehlen Und Tierbewegung

## Umfang Und Ursachen

- Ackerboden und Kulturen zeigen Bodenfeuchte und Wasserbedarf. Der Wert stammt
  aus dem zu genau dieser Pflanze gehoerenden autoritativen Fortschritt, mit
  validiertem Journal-Fallback. Fremde/veraltete Pflanzenwerte werden nicht
  uebernommen. Unterhalb der tatsaechlichen Wachstumsgrenze von 5 Prozent muss
  gegossen werden; bis 25 Prozent erscheint die Vorwarnung. Reife Pflanzen
  bleiben als erntereif gekennzeichnet. Unbepflanzter Boden zeigt weiterhin
  fehlende Samen; eine persistente Feuchtigkeit leerer Ackerzellen wurde nicht
  erfunden.
- Die alte Pflanzen-ID verknuepfte Art und X-Koordinate per XOR. Dadurch hatten
  zum Beispiel Weizen auf `[4,15,22]`, Mais auf `[5,15,22]` und Salat auf
  `[6,15,22]` dieselbe ID `ce25aa44e32dac62`. Die korrekte serverseitige
  Duplikatpruefung wies weitere Aussaat dauerhaft ab. Neue IDs hashen das
  vollstaendige festbreite Tupel. Gespeicherte alte IDs bleiben unveraendert;
  erneute Aussaat erhaelt die neue stabile Identitaet. Revisions-, Reichweiten-
  und Inventarpruefungen bleiben aktiv.
- Bei hohen Hoehlenkammern umfasste die vertikale Streaming-Interessenzone
  nicht immer Dach und Boden. Die bereits vorbereiteten Raumgrenzen erweitern
  jetzt gezielt die angeforderten Sections, auch wenn die vorbereitete Spalte
  erst nachtraeglich ankommt und der Spieler stillsteht. Keine zweite
  Hoehlen-Engine und kein pauschales Laden der gesamten festen Weltspalte.
- Hoehlen-EV wird von 6.621488 auf 6.451563 gesenkt: 12,5 Prozent mehr
  Hintergrundhelligkeit gegenueber Phase 111. Die Fackel-Softcore-Streuung
  erhoeht die Umgebungswirkung ab vier Metern um 30 Prozent, ohne den
  Nahbereich bis einen Meter heller zu machen. Keine neue Lichtquelle je
  Kristall und keine Aenderung der gespeicherten Fackel-Definition.

## Umsetzung

### Hoehlen Und Mineralien

Neue Welten verwenden `cave_structure_revision = 5`. Die bestehenden Kammern
erhalten nicht-radiale Konturen, unregelmaessige Terrassen, trockene
Felsvorspruenge und begrenzte Wasserkanal-Auslaeufe. 25 Prozent geeigneter
normaler, Salz- und Lush-Raeume bekommen solche Auslaeufe; Cascade behaelt
die vorhandenen echten Wasserfaelle. Die vorhandene Fluessigkeitslogik
verarbeitet die trockenen Auslaufzellen mit ihrem bestehenden Budget. Keine
neue Simulation, keine zusaetzlichen Quellen zur Wasservermehrung.

Revisionen 0 bis 4 bleiben in gespeicherten Welten erhalten. Ein expliziter
Regressionstest schuetzt auch das bisherige Salzterrassenprofil der Revision 4.
Die neue Formgebung gilt deshalb nicht rueckwirkend fuer bestehende Welten.
Streaming-, Licht-, Kristallform- und Interaktionskorrekturen gelten hingegen
auch dort. Generatorversion 5 und Weltformat 2 werden nicht veraendert.

Salz-, Dripstone- und Eiskristallsegmente verwenden blockige Querschnitte statt
abgerundeter Formen. Einsturz prueft die wirkliche verankerte Segmentkette.
Abbau eines nur benachbarten, nicht tragenden Blocks loest keinen Einsturz aus;
Entfernen eines tatsaechlichen Traegers beziehungsweise Mittelsegments schon.

### Tiere Und Spinnen

Landbewegung prueft den Weg zwischen alter und neuer Position auf Waende,
Luecken und ungeladene Zellen. Slabs und Treppen liefern ihre echte
Unterstuetzungshoehe. Material- und Formabfrage lesen beide denselben
Journal-/Chunk-Zustand: Ein bereits editierter Slab wurde zuvor zwar als
fest erkannt, bei noch nicht angewendetem Chunk-Update aber als ganzer
Wuerfel aufgeloest. Nach entferntem Boden fallen Tiere; beim Eintritt in
verifiziertes Wasser uebernimmt der vorhandene Auftrieb. Veraltete Routen
fordern eine erneute Planung innerhalb des vorhandenen 96-Knoten-Budgets an.

Spinnen koennen unter anderem beobachten, anschleichen, Ecken patrouillieren,
Netze anlegen und bei niedrigen Lebenspunkten fliehen. Jagd und Flucht sind
physisch 60 Prozent schneller, nicht nur schneller animiert. Braune, dunklere
Baender ersetzen den roetlichen Ton.

`F1 + E` ergaenzt die bestehende Inspektion (`F3 + E` bleibt erhalten). Das
Debug-HUD zeigt Absicht, tatsaechliches Ziel und bis zu acht vorhandene
Routenpunkte. Der Server uebertraegt diese Daten nur fuer das gezielt
inspizierte Tier des jeweiligen Spielers, nicht fuer alle Tiere in jedem
Snapshot. Direktes Steuern bekommt keinen erfundenen Navmesh-Pfad. Dies ist
weiterhin lokale, budgetierte Navigation auf geladenem Terrain, kein globales
Navmesh.

### Platzieren, Truhen Und Laternen

Die Platzierungssilhouette verwendet denselben Layout-Resolver wie die
Serverplatzierung und dieselbe Modellgeometrie wie das spaetere Objekt.
Ausrichtungen, gemischte Slabs, mehrteilige Maschinen/Betten und Nachbarzustand
fliessen in die Vorschau ein. Ein einziges wiederverwendetes, veraenderbares
Mesh verhindert eine Asset-Vermehrung bei jedem Maus- oder Kameraschritt.
In einer echten Sitzung zaehlt ausschliesslich der aktive Inventargegenstand:
Leere Haende duerfen nicht auf die alte Offline-Baupalette zurueckfallen.
Dies verhindert Phantomvorschauen und unerwuenschtes Platzieren aus anderen
Inventarplaetzen. Der explizite Offline-Palettenmodus bleibt erhalten.

Truhenmodelle werden in gemeinsamen Korpus und Deckel geteilt. Berechtigte
Betrachter oeffnen den Deckel in 0,42 Sekunden mit weicher Umkehrung beim
Schliessen. Dies gilt auch fuer Doppeltruhen und nicht selbst zugreifende
Beobachter. Der Sichtbarkeitstest beruecksichtigt beide Haelften ueber
Section-Grenzen. Die Darstellung durchsucht nur geladene Object-Indizes,
nicht den gesamten jemals besuchten Weltzustand. Modelle bleiben auf der
3D-Karte sichtbar. Der statische geschlossene Auswahlrahmen wird waehrend
offener beziehungsweise bewegter Deckel ausgeblendet, statt durch den
geoeffneten Deckel zu ragen. Neue Truhen-Oeffnungs-/Schliessgeraeusche sind nicht Teil
dieser Aenderung.

Crusher- und Vogelscheuchenlaternen verwenden den bestehenden gemeinsamen
Lichtpool mit den unveraenderten Qualitaetsbudgets 4/8/12/16. Positionen folgen
den tatsaechlichen Modellankern, Drehung und Skalierung. Prop-Lichtkandidaten
werden bei Section-/Prop-Aenderungen erneuert; es gibt keinen neuen
Vollwelt-Prop-Scan pro Frame.

## Kompatibilitaet

Netzwerkprotokoll **64**: Client und Server gemeinsam aktualisieren.
PLY7, RGA2 mit alten Lesern, Weltformat 2, Generatorversion 5 und bestehende
Block-/Item-/Welt-IDs bleiben erhalten. Der Dedicated Server bleibt headless.
Keine Dependency-Upgrades und keine Migration gespeicherter Hoehlenrevisionen.

## Pruefungen

`cargo test --locked --workspace`: **2019 bestanden, 3 bestehende ignoriert**.
Die Tests umfassen die alte ID-Kollision, Nachpflanzen mit alten gespeicherten
IDs, echte Feuchtegrenzen, reale Jagd-/Fluchtstrecken, langsames Anschleichen,
Unterstuetzung nach Journal-Edits, Wasserauftrieb, budgetierte Neuplanung,
gezielte UDP-Inspektion, Doppeltruhen-Betrachter, Vorschau-Geometrie und deren
einzelnes Mesh-Asset, Boden-/Decken-Mineralketten an negativen Section-Grenzen,
geladene Hoehlenschalen, Revision-4-Kompatibilitaet, Revision-5-Geometrie und
echte Wasserfluss-/Reload-Regeln. Rohprotokolle liegen in [logs](logs).

`cargo build --locked --workspace` baut Client und Dedicated Server.
Nach der letzten Leere-Hand-/Sichtbarkeitskorrektur ebenfalls bestanden:

- `cargo fmt --all --check`
- `cargo check --locked --workspace --all-targets`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `git diff --check`

Die abschliessenden nativen Logs enthalten keine WARN-/ERROR-Eintraege.

### Native Ergebnisse

Tatsaechlich ausgefuehrt auf **RX 6950 XT / RADV / Vulkan**. Alle folgenden
Pruefungen endeten mit Exit 0. Repraesentative PNGs aus allen fuenf
Clientgruppen wurden zusaetzlich angesehen; die Modellgalerie ist keine
Pruefung autonomer KI und die Inspektionslampen keine Produktionsbeleuchtung.

| Pruefung | Ergebnis |
|---|---|
| Farming | 11 Aufnahmen, 29 gespeicherte Kulturen nach Save/Reload; Weizen/Corn/Lettuce auf den zuvor kollidierenden benachbarten Positionen jeweils `Applied` |
| Truhe | 9 Aufnahmen; echte sichtbare Vorschau mit 1536 Vertices, Platzieren, autoritatives Oeffnen/Schliessen, leere Hand ohne Phantomvorschau, vier Abbaustufen und Abbruch |
| Natuerliche Hoehlenschale | 2 Aufnahmen, 800 geladene Sections; reale geschlossene Dach-/Bodenbloecke in der kompletten Mittelspalte, 13 Wassertiles |
| Hoehlenlicht | 4 Aufnahmen der geschlossenen Kammer mit laufender Uhr, einschliesslich Unterwasser; EV 6.451563 und finale Fackelbelichtung bestaetigt |
| Spinnen | 9 Aufnahmen der drei Groessen, Seiten-/Frontansichten, Wand-/Decken-/Abseilposen und zehn Netzmodelle |
| Headless Server | 27 Sections, kein Fenster-/GPU-Start, `--no-save` |

Beispielaufnahmen: [Wasser-HUD](native/farming/farming-growth-hud.png),
[Bauvorschau](native/chest/furniture-29-placement.png),
[offene Truhe](native/chest/chest-lid-open.png),
[geschlossene Truhe](native/chest/chest-lid-closed.png),
[Hoehlenboden](native/shell/cave-natural-shell-0.png),
[Hoehlendach](native/shell/cave-natural-shell-1.png),
[Fackel](native/light/cave-15-Medium.png),
[braune Spinne](native/spiders/spiders-4.png).

Die erste Truhenaufnahme war durch einen Baum schlecht eingerahmt. Der
verstaerkte Test deckte anschliessend eine zu strenge Testplatzsuche auf,
die Gras vor einem gueltigen Boden-Hit ablehnte. Die korrigierte Fixture
prueft sichtbare Modellkoerper-/Dachpunkte, ViewVisibility, echte CPU-Vertices
und Bildgrenzen; die erfolgreiche letzte Aufnahme ersetzt die erste.
Ein eigener Regressionstest akzeptiert flachen Boden und tiefe Zierleisten,
weist aber einen tatsaechlich verdeckenden Baumstamm weiterhin ab. Creative
und Survival besitzen zusaetzlich eine Leere-Hand-Regression mit Stein im
Hauptinventar und leerem Hotbar-Slot; die Offline-Palette bleibt separat
benutzbar.

### Begrenzte Laufzeitmessung

Die vorhandene Lichtfixture protokolliert folgende kurze Messfenster.
Das sind **keine** Vorher-/Nachher-Vergleiche, keine 12-/20-Chunk-FPS-Abnahme
und kein Nachweis langfristiger Speicherstabilitaet.

| Lichtfixture-Stufe | FPS | p95 ms | p99 ms |
|---|---:|---:|---:|
| 14, geschlossen | 123.78 | 9.00 | 9.93 |
| 15, geschlossen | 109.02 | 11.06 | 12.04 |
| 16, unter Wasser | 147.62 | 8.78 | 12.65 |
| 17, unter Wasser | 151.37 | 7.25 | 8.98 |

Qualitaet Medium, tatsaechliche Aufnahme 1920 x 1200; keine eigenen Cargo-Builds
parallel zum Client. GPU-Einzelzeit,
Draw Calls und RAM-Vorher-/Nachher wurden hier nicht gemessen. Die
Ueberbelichtungspruefung erfasste in diesen Aufnahmen 0.012 bis 0.125 Prozent
sehr helle Pixel einschliesslich der absichtlich hellen Flamme. Der
normierte Streulicht-Regressionstest prueft den 30-Prozent-Zuschlag getrennt
von Tonemapping und Flicker.

### Kurze Native Reproduktion

Alle Befehle laufen im Repository-Hauptverzeichnis mit demselben Build.
`TERRA_CONFIG_DIR` und `TERRA_MENU_CONFIG_DIR` zeigen auf das bestehende lokale
Testprofil. Die Aussaat-Pruefung benoetigt einen noch nicht vorhandenen Pfad
fuer `TERRA_FARMING_SMOKE_DIR`, damit kein eigener Spielstand veraendert wird.

```bash
TERRA_FARMING_SMOKE_DIR=target/phase112/farm-run1 TERRA_FARM_GROWTH_INSPECT=1 TERRA_CAPTURE_DIR=tools/benchmarks/results/building-farming-caves-2026-10-23/native/farming target/debug/terra-client --smoke-test-farming
TERRA_BUILD_CREATIVE=1 TERRA_FURNITURE_MINING_SMOKE=chest TERRA_CAPTURE_DIR=tools/benchmarks/results/building-farming-caves-2026-10-23/native/chest target/debug/terra-client --smoke-test-building
TERRA_CAVE_SHELL_INSPECT=1 TERRA_CAPTURE_DIR=tools/benchmarks/results/building-farming-caves-2026-10-23/native/shell target/debug/terra-client --smoke-test-cave-structures
TERRA_CAVE_LIGHT_INSPECT=1 TERRA_CAPTURE_DIR=tools/benchmarks/results/building-farming-caves-2026-10-23/native/light target/debug/terra-client --smoke-test-caves
TERRA_CAPTURE_DIR=tools/benchmarks/results/building-farming-caves-2026-10-23/native/spiders target/debug/terra-client --smoke-test-spiders
```

Die Hoehlenschalen-Pruefung benutzt deterministisch Seed 42042/Revision 5
und prueft reale geladene Boden-/Dachbloecke. Ihre zwei expliziten
Inspektionslampen dienen ausschliesslich der Geometrie-Abnahme und sind kein
Nachweis der natuerlichen Hoehlenhelligkeit. Die Spinnengalerie prueft Modelle
und Posen, nicht autonomes Verhalten. Navigation, Wasser, Geschwindigkeit und
Netzwerkberechtigung brauchen die gesonderten Regressionstests.

Keine neuen 10-/15-/30-Minuten-Tests. Es werden keine FPS-Steigerung und keine
Vorher-/Nachher-Langzeitstabilitaet ohne entsprechende Messungen behauptet.
