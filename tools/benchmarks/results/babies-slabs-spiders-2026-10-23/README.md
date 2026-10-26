# Nachwuchs, Slabs, Spinnen Und Baukonturen

Phase 115 erweitert die bestehenden Systeme, ohne neue Dependencies oder
Block-/Welt-/Generator-IDs. Client und Server verwenden gemeinsam Protokoll 66.

## Baukontur

Die echte Modellkontur ist nun zwei physische Viewport-Pixel breit statt einer
einzelnen Rasterlinie. Ein gemeinsam genutztes Material und ein behaltenes
Kantenmesh reichen weiterhin aus. Der neue kleine WGSL-Vertexshader erweitert
entartete Kanten-Quads ausschliesslich senkrecht zur projizierten Kante; es
entstehen keine gefuellten Modellflaechen. Kamera-/Viewportaenderungen benoetigen
keine neuen Meshes. Hinter der Kamera liegende Endpunkte werden vor der
perspektivischen Division abgeschnitten.

Die Farbe ist heller: sRGB `(1.0, 0.68, 0.2)`. Die Deckkraft pulsiert weiterhin
zwischen 0,4 und 0,95 mit 1,6 Sekunden Periode, ohne Geometrie-Rebuilds.
Normale Vollbloecke erhalten weiterhin keine Platzierungsvorschau.

## Slabs

- Boden-/Deckenmitte: vertikale Haelfte nach dominanter Blickrichtung.
- Die aeusseren 15 Prozent einer Boden-/Deckenflaeche bleiben horizontal.
- Vertikale Fortsetzungen, alle sechs Halbierungen und negative Koordinaten
  verwenden denselben Placement-Resolver fuer Vorschau und echten Server-Edit.
- Ein gemeinsamer Ray-/Halb-AABB-Helfer bestimmt bei gemischten Slabs das
  getroffene Material, Werkzeugniveau und die verbleibende Haelfte.
- Der Server entfernt atomar nur die getroffene Materialhaelfte und erzeugt
  genau deren Drop. Beim Wechsel der Haelfte beginnt der Abbau neu.
- Stationaere Resthaelfte und bewegte Abbauhaelfte werden separat durch den
  vorhandenen ModelComposer dargestellt; Partikel verwenden das Zielmaterial.
- Das Hover-HUD verwendet denselben Ray fuer Materialtitel, Identifier und
  Werkzeuganzeige, auch ohne laufenden Abbau und in Third-Person.
- Identische Slab-Paare behalten gemeinsamen Abbau mit zwei Items.
- Historische Doppelslabs mit Facing 9 enthalten keine gespeicherte Split-Ebene;
  sie bleiben lesbar und werden ausdruecklich horizontal interpretiert.

## Spinnen

Konkrete Ursachen waren dauerhafte Tagesruhe unter einem Hoehlendach,
ausschliesslich ebene Netzsuche, verlorene Ziele beim Wechsel zwischen Boden
und Wand sowie der pauschale Wartepfad fuer bereits gezaehmte Spinnen.

Die Netzsuche prueft nun begrenzt auch benachbarte hoeher-/tieferliegende
Verankerungen. Ein begonnenes Ziel bleibt beim Klettern hoechstens vier Sekunden
erhalten; die aktuelle Eignung wird weiter geprueft. Geschuetzte mittelgrosse
Spinnen wechseln zwischen kurzen Ruhephasen und Erkundung statt dauerhaft AFK
zu bleiben. Nach dem Netzbau suchen hungrige Spinnen im vorhandenen 20-m-Bereich
tatsaechliche Nahrung. Hunger bleibt 50, Durst 0.

Gezaehmte Spinnen erhalten ebenfalls eigenstaendigen Netzbau und Nahrungssuche,
aber keine Spielerziele. Das pausierte Folgen und der abgeschaltete
Leerhand-Umschalter werden nicht reaktiviert. Andere Pets warten weiterhin.

## F06: Zucht Und Wachstum

Unterstuetzt sind Schafe, Kuehe, Schweine, Pferde und alle drei Spinnenarten.
Die Groessenarten der Spinnen bleiben verschiedene Arten, keine Wachstumsstufen.

Zwei bereits gezaehmte, erwachsene Tiere gleicher Art und desselben Besitzers
werden jeweils einmal zusaetzlich mit ihrem bisherigen F05-Futter gefuettert:

| Art | Futter |
|---|---|
| Schaf, Kuh | Weizen |
| Pferd | Weizen, Mais oder Salat |
| Schwein | Mais, Salat oder Pilz |
| Spinne | Rohes Rind-/Schweine-/Schaffleisch, Fisch oder Clownfisch |

Das verbraucht je ein echtes gehaltenes Item. Die abschliessende Zaehmungsfuetterung
allein loest keine Zucht aus. Die bestehenden Aktions-/Fuetterungsabstaende bleiben
450 ms beziehungsweise 1,25 s. Eltern muessen wach, lebendig und mindestens zur
Haelfte gesund/gesaettigt sein. Die Bereitschaft gilt 60 aktive Sekunden.

Partner benoetigen einen geladenen begehbaren Weg, hoechstens 5 m Abstand und
1 m Hoehenunterschied. Eine Geburt braucht trockenen Untergrund, freien
Terrainraum fuer die ausgewachsene Groesse und Abstand zu lebenden Tieren. Pro Paar entsteht ein
Jungtier; beide Eltern haben anschliessend 300 aktive Sekunden Abklingzeit.

Jungtiere wachsen ueber 1200 aktive Simulationssekunden weich von Faktor 0,45 auf
1,0, mit jugendlichen Kopfproportionen und schnellerem Gang. Rendering,
Zielvolumen, Abstaende, Bewegung und Unterstuetzung beachten ihre Groesse.
Neue Hindernisse pausieren unmoegliches Wachstum, nicht den Futter-/Heilabstand:
eine getrennte gespeicherte physiologische Uhr laeuft fuer Nachwuchs auch dann
weiter und bleibt nach dem Erwachsenwerden massgeblich fuer Fuetterungen.

Besitzer und volles Vertrauen werden uebernommen; die Variante stammt
deterministisch von einem Elternteil. Alte natuerliche/Command-Tiere mit Alter 0
bleiben erwachsen. Neue Geburts-IDs ersetzen keine bestehenden Tier-IDs.
Ein gespeicherter High-Water-Zaehler verhindert Wiederverwendung nach Loeschung
oder Reload. Elternreferenzen, Uhren und Abklingzeiten werden repliziert/gespeichert.

Die Zucht arbeitet einmal pro Sekunde mit maximal zwei Geburten, einem
rotierenden 64-Tiere-Arbeitsfenster und raeumlicher Partnersuche. Hohe IDs werden
nicht dauerhaft ausgeschlossen. Grenzen: 16384 gespeicherte Tiere einschliesslich
Tombstones, 128 lebende eigene Tiere je Besitzer, 32 lebende Tiere innerhalb von
32 m einer Geburt. Eigener Nachwuchs zaehlt nicht zum Wildtier-Spawnlimit.
Weitere Generationen erfordern erneutes ausdrueckliches Fuettern.

## Speicherung Und Grenzen

Neue Familienregionen verwenden RGA4, ein benoetigter Geburtszaehler IDX7.
RGA3/IDX6 und alle bisherigen Leserpfade bleiben erhalten; Regionen ohne neue
Familien beziehungsweise Welten ohne Geburt brauchen diese Erweiterung nicht.
Die binaere Creature-Struktur bleibt unveraendert. Weltformat 2, PLY7 und
Generatorversion 5 bleiben gleich. Client und Dedicated Server gemeinsam bauen.

Keine Pet-Befehls-UI, keine Bienen-/Fischzucht und keine Offline-Vermehrung in
dieser Phase. Bienen-/Honigfortpflanzung bleibt F12. Es wurden keine neuen
10-/15-/30-Minuten- oder mehrstuendigen In-Game-Tests angesetzt.

## Validierung

Mit bestehender Cargo.lock und einem Cargo-Buildjob ausgefuehrt:

| Pruefung | Ergebnis |
|---|---|
| `cargo fmt --all --check` | Erfolgreich |
| `cargo check --locked --workspace --all-targets` | Erfolgreich |
| `cargo test --locked --workspace --no-fail-fast` | 2091 erfolgreich, keine Fehler |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Erfolgreich |
| `cargo build --locked --workspace` | Client und Dedicated Server erfolgreich |
| `terra-server --smoke-test --no-save` | Headless erfolgreich, 27 Sections |
| Vollblock-Fixture | Exit 0, keine Vorschau auch mit F1+B/FOV-/Fensterwechsel, echter Abbau/Setzen |
| Slab-Fixture | Exit 0, 12 Kanten mit 2 px, Alpha 0,647 bis 0,853 ohne neuen Mesh-Handle, echte vertikale Platzierung Facing 5 |
| Zucht-Fixture | Exit 0, zwei echte Fuetterungen/Items, genau eine Geburt, Eltern/UUID/Abklingzeit korrekt, fuenf Aufnahmen |

Die neuen Spinnenregressionen verwenden zwei echte lokale UDP-Clients: Netzbau
wird vor dem Verzehr naher Nahrung autoritativ committed und an beide Clients
repliziert, sowohl bei wilden als auch gezaehmten Spinnen. Dazu kommen
Hoehlenstufen, Tagesruhe, Kletterziele und Jungtier-Reichweite.

Slab-Regressionen decken alle sechs Halbierungen, negative Koordinaten,
Material-/Drop-/Restzustaende, Abbauwechsel und reine Praesentationsdaten ab.
Ein echter UDP-Lauf prueft zwei Clients und einen spaeteren Beitritt. Die
Werkzeugunterschiede werden mit ausdruecklich konfigurierten Testeigenschaften
geprueft; ausgelieferte Werkzeuganforderungen wurden nicht umgeschrieben.

F06 prueft alle sieben zaehmbaren CreatureKinds, deterministische Paarung,
Besitzer-/Art-/Reichweitenregeln, freie Geburt, begrenzte Population, Kollision
beim Wachstum, weiterlaufende Futterabstaende, faire Kandidatenrotation,
Familien-/Uhren-Roundtrip und ID-Schutz nach Loeschung/Reload.

### Native Bilder

RX 6950 XT, RADV/Mesa 26.2.4, Vulkan, isolierte Testkonfiguration. Die folgenden
Aufnahmen wurden tatsaechlich angesehen, nicht nur auf Dateiexistenz geprueft:

- [Helle ungefuellte Kontur](native/slab/furniture-82-placement.png)
- [Tatsaechlich hochkant gesetzter Bodenslab](native/slab/slab-floor-center-committed.png)
- [Echt geborener Nachwuchs zwischen den Eltern](native/breeding/breeding-newborn.png)
- [Erwachsenes vorbereitetes Jungtier](native/breeding/breeding-adult.png)

Im projizierten Slab-Modellbereich liegen 2959 orange Framebufferpixel
(260x322 Pixel ab 790/368 im 1920x1200-Bild). Das ist ein Nachweis sichtbarer
Kanten, kein isolierter Vorher-/Nachher-Dickenbenchmark bei identischer Pose.
Der neue WGSL-Shader wurde im laufenden Client kompiliert und gerendert.

Die Zuchtszene verwendet einen frischen Save `target/phase115/breeding2`.
Der neue Nachwuchs entsteht aus echten Inventaraktionen mit Weizen 6 auf 4.
Ein zusaetzlich vorbereitetes Jungtier startet 30 aktive Sekunden vor der
1200-Sekunden-Schwelle und ueberschreitet sie im Lauf tatsaechlich. Es wird
nicht behauptet, dass das Neugeborene in diesem Kurztest 20 Minuten durchlief.

Der finale Bau-/Zucht-/Vollblock-/Headless-Lauf sowie Build und Clippy enthalten
keine Warnungen oder Fehler. `logs/intermediate-render-check.log` dokumentiert
einen nicht abschliessenden Lauf waehrend noch geaenderter Quellen; der
abschliessende Gesamt-Teststand steht in `logs/tests.log`.

Beim nativen Testen wurden reine Fixture-Probleme korrigiert: ein irrtuemlicher
Rand-Fallback im Bodenmitten-Test, vorzeitig wechselnde Blickziele zwischen
Aktionsqueue und Bestaetigung sowie der erforderliche Abstand zwischen zwei
Fuetterungsaktionen. Strenge echte Platzierungs-, Inventar-, Besitz-, Eltern-
und Wachstumspruefungen wurden beibehalten und um Regressionen ergaenzt.

Logs liegen in `logs/`, Aufnahmen in `native/`. Gemischte Slabs und Spinnen-AI
sind durch echte UDP-/Renderregressionen geprueft, nicht als neuer nativer
Rundgang fuer jede Materialkombination beziehungsweise jede Hoehle behauptet.
Keine allgemeine FPS-/Langzeitabnahme wird aus diesen Kurztests abgeleitet.
