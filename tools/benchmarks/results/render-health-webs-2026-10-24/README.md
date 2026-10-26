# Blockoberflaechen, Mob-Balken, Netze Und Blumen (Phase 118)

## Umsetzung

### Blockoberflaechen

Die auffaellige Rasterwirkung entstand nicht nur im Fragmentshader: Nahe
Grasoberseiten bestanden aus wiederholten eingesenkten Plateau-/Rillenkacheln.
Sie behalten jetzt eine durchgehende Oberseite und dieselben Blockkanten wie
entferntere LODs. Seitliche Gras-/Erdstruktur und echte Vegetationsprops bleiben
erhalten. Der Shader legt keine prozeduralen Steinziegel mehr ueber die Textur;
weltverankerte Mineralvariationen und feinere, schwaechere Normalen ersetzen sie.

**Graphics > Block Surface Shader** verwendet den vorhandenen Schalter
`[blocks] material_detail` in `graphics.toml`. Aus bedeutet keine zusaetzliche
prozedurale Material-/Normalenprofilierung; handgemalte Texturen, vorhandene
Geometrie, Wetness und lokale Schneeschichten bleiben erhalten. Es werden keine
neuen Materialhandles oder Shader-Varianten je Umschaltung erzeugt.

### Mob-Lebensbalken

**Gameplay > Mob Health Bars**, gespeichert als `mob_health_bars`:

| Auswahl | Verhalten |
|---|---|
| Looking At, Standard | Naechstes Tier unter dem wirklichen Kamerastrahl, freie Sicht erforderlich |
| All In View | Tiere im Bildschirmbereich mit freier Terrain-/Modellsicht |
| All Time | Alle projizierbaren nahen Tiere, absichtlich auch durch Waende |
| None | Keine Balken |

Freundliche Tiere und gezaehmte Mobs sind gruen, feindliche rot. Titel, aktuelles
Leben und Maximum stehen ueber dem Tier. Die UI benutzt die vorhandenen
replizierten CreatureTracks, keine neuen Mob-Entities. Maximal 64 Balken bis
64 m, acht Sichtpruefungen je UI-Tick und eine auf aktuelle Kandidaten begrenzte
Ablage. Die aeltesten Pruefungen kommen zuerst, damit beim Laufen entfernte
Balken nicht dauerhaft von nahen Kandidaten verdraengt werden. Inventare bleiben
ueber der Anzeige; Tod, Verbindungsverlust und None leeren sie.
Unsichtbare Bienen im Nest bekommen keinen Balken. Das eigene Reitpferd wird
waehrend des Reitens ausgelassen und nach dem Absteigen wieder angezeigt.
Remote-Pferde verwenden die bestehende CreatureTrack-Interpolation; eine
separate gemeinsame Renderpose fuer deren Reitdarstellung ist nicht eingefuehrt.

### Stalker

Das vermeintliche Texturflackern waren koplanar ueberlappende Moos-, Kopf- und
Koerperquader. Konstruktive Fugen trennen die Detailkacheln; der geschlossene
Kern und die geteilten Modellteile bleiben erhalten. Eine echte geometrische
Ueberlappungsregression enthaelt eine negative Kontrolle der alten Situation.

### Raeumliche Netzkonstruktionen

Vier neue Vorlagen, Varianten 10..13, verwenden die vorhandenen SpiderWeb-
WorldProps als logische Konstruktionen, nicht eine zweite Registry. Drei
raeumlich versetzte Fangflaechen, Fangfaeden und 6..10 Halteseile bilden je eine
gebuendelte Geometrie. Unterschiedliche Ankerabstaende ergeben rund 7..20 m
freie Spannweite. Der Tippfehler "66 - 20" wurde als etwa 6..20 Bloecke gelesen.

Die KI sucht echte obere Wand-/Gegenanker. Geladene trockene Seilverlaeufe und
mindestens 2 m Durchgangsraum werden validiert. Die vorhandenen Netz-/Sektions-
Limits bleiben bestehen. Auswahl und Blockaenderungen finden grosse Netze auch
bis zwei Owner-Sections entfernt. Entfernen/Gegenankerabbau bleibt nach Reload
und Replikation erhalten. Eine Konstruktion wird als Einheit abgebaut.

Die Vorlagen besitzen je LOD ein Materialbatch und weniger als 8000 Vertices.
Section-Batching, gemeinsame Vorlagen und reduzierte entfernte Fadendetails
bleiben aktiv. Es gibt keine Entity, Materialinstanz oder Lichtquelle je Faden.
Alte Varianten 0..9 und deren gespeicherte Identitaeten bleiben gueltig.

### Hoehlenabstand

Neue Hoehlenstrukturrevision **7** begrenzt das vorhandene breite Netz mit
einem ruhigen, kontinuierlichen Praesenzfeld auf zwei raeumlichen Skalen
(56/128 m). Es wird je horizontaler Spalte vorbereitet, nicht pro Voxel neu
berechnet. Kammerdeskriptoren nutzen dieselben Regionen. Innerhalb aktiver
Cluster bleiben die Gangbreite aus Revision 6 und die ausgewaehlten
Kammergroessen erhalten; massive Zonen ersetzen dauerhaft dichte Gangnetze.
Unterdrueckte Kammern bleiben geschlossen und behalten die bisherigen
Boden-/Deckenschutzbereiche. Ihr Wegfall kann somit nicht versehentlich
weitere Tunnel oeffnen.

Reproduzierbare Samples der Regression, Revision 6 gegen 7, gleicher Seed und
identische unterirdische Koordinaten (keine FPS-Messung):

| Seed | Offene Samples vorher | Nachher | Kammer-Spalten vorher | Nachher |
|---|---:|---:|---:|---:|
| 7 | 7062 | 4128 | 593 | 374 |
| 42042 | 7169 | 4010 | 373 | 224 |
| 98173 | 8090 | 4813 | 421 | 267 |

Separate Connector-Samples behalten 56.1/57.8/58.1 Prozent der zuvor offenen
Gangpunkte. Benachbarte Praesenzsamples im Abstand von 4 m stimmen zu
92.7..93.2 Prozent ueberein: ruhige Regionen statt zufaellig geschlossener
Einzelvoxel. Getestete zusammenhaengende massive Abschnitte erreichen
124/160/188 m. Dies sind Stichproben, keine garantierten Regionsdurchmesser.

Bestehende Welten bleiben bei ihrer gespeicherten Revision 0..6, fehlende
historische Felder bedeuten weiterhin Revision 0. Keine stille Regeneration von
Spielerbauten und keine zweite Hoehlenengine. Das umfassendere organische
Raumform-Redesign F36 bleibt offen.

### Zehn Sekunden Verwelken

Nach Ueberschreiten ihres stabilen saisonalen Rangs wird eine Blume innerhalb
von zehn echten Sekunden braun, schrumpft an ihrer Wurzel und verschwindet.
Nach spaetestens 4.5 Sekunden ersetzt ein warmer Braunton die urspruenglichen
Bluetenfarben vollstaendig, statt blaue Blueten nur schwarzbraun zu multiplizieren.
Die Animation laeuft auch bei pausierter Spielzeit weiter. Bereits dormant
geladene Pflanzen blitzen nicht erneut auf; Fruehling bzw. Saisonrueckspruenge
reaktivieren die vorhandenen Pflanzen ohne Aenderung ihrer IDs/Tombstones.

Ein fester transienter Block fuer 256 Raenge im bestehenden EditJournal steuert
Shader, Picking und Bienen. Keine Ablage pro Pflanze, kein permanentes Wachstum
eines Caches und kein saisonaler Mesh-Rebuild. Der gemeinsame Wetterbuffer hat
1120 Byte (sechs Vec4 plus 64 Vec4 Zeitwerte); alle WGSL-Kopien teilen das Layout.
Root-Anker verwenden die vorhandene Growth-Pipeline. Saisonale Props werden
nicht dauerhaft aus gespeicherten Welten geloescht.

## Kompatibilitaet

Client und Dedicated Server gemeinsam auf **Protokoll 69** aktualisieren. Der
Bump schuetzt die neue Netzvariantensemantik vor alten Clients. Keine neuen
Dependencies, Block-/Welt-IDs oder Saveformatversionen. Alte Weltrevisionen und
gespeicherte Spielerentfernungen bleiben erhalten.

## Pruefumfang

Regressionen umfassen reale Kamera-Projektion, Modi, Occlusion/Farben und faire
Sichtbudgets bei kontinuierlicher Bewegung; wiederholte Shaderumschaltung ohne
Materialvermehrung; Stalker-Flaechengeometrie; Netzbau, Anker, freie Durchgaenge,
ferne Auswahl, Speicherung und Replica-Reload; echte Hoehlenluft-/Kammer-Samples
ueber mehrere Seeds und negative Grenzen; Root-Schrumpfung, Braun-/Sichtzustand,
Zeitspruenge und Shaderlayout der Blumen.

Native Kurzfixtures sind `--smoke-test-mob-health`,
`--smoke-test-flower-wither` und `--smoke-test-web-constructions`.
Sie verwenden echte Render-/Prop-/UI-Pipelines und pruefen auch Framebufferpixel.
Sie sind keine Netzwerk-Lasttests oder vollstaendige Art-Abnahme. Die
Stalker-/Weltlogik wird zusaetzlich mit vorhandenen Regressionen geprueft.
Keine 10-/15-/30-Minuten-Spieltests und keine erfundenen FPS-Messwerte.

## Tatsaechliche Validierung Am 24.10.2026

Alle Cargo-Kommandos mit `--locked`, die speicherintensiven Schritte einzeln
mit `CARGO_BUILD_JOBS=1`:

| Pruefung | Ergebnis |
|---|---|
| `cargo fmt --all --check` | Bestanden |
| `cargo check --workspace --all-targets` | Bestanden |
| `cargo test --workspace --no-fail-fast` | 2192 bestanden, 0 fehlgeschlagen, 3 bestehende ignoriert |
| `cargo clippy --workspace --all-targets -- -D warnings` | Bestanden |
| `cargo build --workspace` | Client und Dedicated Server bestanden |
| `terra-server --no-save --smoke-test` | Headless bestanden, 27 Sections |

Native Kurzlaeufe auf AMD Radeon RX 6950 XT, RADV Mesa 26.2.4/Vulkan.
Die zugehoerigen Aufnahmen wurden tatsaechlich angesehen:

| Szene | Ergebnis |
|---|---|
| Blockmaterialien, Shader an/aus | Je 10 Aufnahmen, 101 Blocktypen und 101 GPU-Previews bereit, kein Shaderfehler |
| Stalker | 5 Ansichten einschliesslich Greifen/Laufen; strenger Geometrietest ohne koplanare Aussenflaechen-Ueberlappung |
| Mob-HP | 5 Aufnahmen: Looking At, In View, echte Wandverdeckung, Always, None |
| Blumen | 3 Aufnahmen nach 0/5/10 s; unveraenderte Meshhandles/-daten, keine persistenten Entfernungen |
| Netze | 6 isolierte Ansichten, 7/12/20 m, je ein echter Section-Propbatch und vollstaendige AABB ueber Section-Grenzen |

HP-Farbpruefung in den tatsaechlichen UI-Fuellflaechen: gruen 2042 von 2090
Pixeln, rot 1904 von 2090 Pixeln. Der Test verlangt zusaetzlich passende
HUD-Daten, projizierte Nodegroessen und gerenderte Creature-Avatare.

Die Blumen-Pixelmaske misst 98058 lebende Pflanzenpixel, nach 5 s noch 20910
vollstaendig braune und nach 10 s null. Die Netzpruefung misst je Ansicht
18852..31919 kontrastierende helle Seilkantenpixel. Diese Werte belegen die
jeweiligen isolierten Renderzustande, nicht Framerate oder einen allgemeinen
Performancegewinn. Rohprotokolle liegen in `logs/`, die 39 Aufnahmen in `native/`.

Die nativen Erstversuche deckten auch Fehler in den neuen Pruefszenen auf:
unbeantwortete Prediction ohne Server, eine nur im Journal eingetragene
Testwand, falsche Beleuchtungs-/Farbannahmen und eine Atmosphaerenprobe an einer
deaktivierten Testkamera. Die Fixtures wurden korrigiert; die strengen
Sicht-/Pixelpruefungen blieben erhalten. Der Blumen-Braunton wurde anhand der
echten Aufnahme im Produktionsshader nachgebessert und danach erneut gemessen.

### Grenzen

Die Netzgalerie verwendet eine neutrale Inspektionskamera, keine vollstaendige
prozedurale Hoehlenszene. Das tatsaechliche Erreichen oberer Anker und Bauen wird
separat fuer alle drei Spinnengroessen ueber echte Simulationsschritte getestet.
Eine neue grosse Hoehlen-Art-Abnahme oder ein allgemeiner FPS-/Langzeitnachweis
ist nicht Bestandteil dieser Kurzlaeufe. Bestehende Welten behalten bewusst
ihre bisherige Hoehlengenerierung. Das komplette Raumform-Redesign F36 bleibt
offen.
