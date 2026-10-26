# Doerfer, Pet-HUD, Spinnennester Und Baumabbau

Stand: 2026-10-24, Phase 122. Direkte Erweiterung bestehender Systeme; keine
neuen Dependencies, keine zweite Welt-, Inventar- oder KI-Engine.

## Baum Und Spinnen

Ganzbaumabbau benoetigt jetzt **0,1 Sekunden pro tatsaechlichem natuerlichem
Holzblock**. Derselbe Plan liefert Timer, Serverpruefung und Fallanimation.
Wurzeln, Stammsegmente und Aeste zaehlen mit; editiertes Spielerholz und Holz
benachbarter Baeume nicht. Tests vergleichen die vollstaendige vorbereitete
Form mit dem wirklichen Raster und den einem Baum zugeordneten Plan-Zellen.

Autonome Spinnen bauen zuerst eine wirkliche grosse Netz-Konstruktion; ein
kleines dekoratives Netz genuegt nicht. Danach folgen deterministisch zufaellige
**30 bis 90 Sekunden** Nestpause, eine begrenzte Jagd-/Patrouillenphase und
Rueckkehr. Jagd bleibt auf bestehende Beute-, Sicht-, Kollisions- und 20-m-Regeln
begrenzt. Ohne Beute patrouilliert die Spinne statt bewegungslos auf einen
Hungerwert zu warten. Follow/Rest und Beruhigung haben weiterhin Vorrang.

Die begrenzten Runtime-Zyklusuhren verwenden aktive Simulationszeit, nicht das
bei blockiertem Nachwuchs angehaltene biologische Wachstum. Gespeicherte grosse
Nester bleiben nach Neustart bestehen; die autonome Pause beginnt dann neu.
Rueckkehr ist begrenzt, ohne Teleportation durch inzwischen gebaute Waende.

## F9 Chunk-Grenzen

F9 schaltet ein Gitter mit vertikalen Chunk-Spalten und horizontalen 16-m-
Section-Ebenen ein. Die aktuelle Section wird goldfarben hervorgehoben.
Das Gitter umfasst eine feste 3x3-Nachbarschaft, 276 Linien und die bestehenden
Welthoehen. Es wird nur bei Sectionwechsel neu aufgebaut, nicht aus der Welt
ausgelesen, und erzeugt keine Entity je Linie. Negative Koordinaten verwenden
`div_euclid`. Die Taste ist ueber das bestehende Controls-System umbelegbar;
alte benutzerdefinierte F9-Belegungen werden bei Migration nicht gestohlen.

## F09 Strukturen

Das vorhandene PreparedColumn-/Section-System stempelt deterministische
regionale Bauplaene nach natuerlichem Terrain/Baeumen und vor Spieler-Overrides.
Regionen sind 384 m gross, Plaene seedbasiert, unveraenderlich und zwischen
Threads geteilt. Der Planner laeuft nicht pro Voxel im Section-Hotpath.
Cache: 64 Eintraege; ein Plan hat maximal 256 Sections und 300000 Zellen.
Chunk-Halos, direkte Blockabfragen und normale Generierung nutzen dieselben
Plaene und dieselbe Baumfreiraeumung.

Gebaeude halten einen gemeinsamen 8-m-Wurzelabstand, damit benachbarte breite
Kronen nicht durch Fassaden wachsen. Dieselbe Reichweite gilt fuer direkte
Abfragen und vorbereitete Section-Halos. Brueckengelaender stehen quer zur
jeweiligen Laufrichtung seitlich; unter der dreibloeckigen Fahrbahn bleibt der
wirkliche Wasserquerschnitt frei, statt einen Holzdamm einzustempeln.

Drei Dorfstile orientieren sich an der Referenz:

- Wueste: helle Sandsteinwaende, Terrakottatoene, Flachdaecher/Kuppeln.
- Plains/Forest: Stein, weisse Gefache und Holz mit Giebeldach.
- Highlands: Fichte/Stein, steile Daecher; Schnee ueber das vorhandene Wetter-
  und Oberflaechensystem statt permanent eingestempelter Schneebloecke.

Doerfer besitzen begehbare Haeuser, Kirche, Schmiede, Wachturm, Medic-Gebaeude,
Felder, Wege und bei geeigneter Wasserquerung Bruecken. Gebaeude werden einzeln
an geeignete Hoehen angepasst; ungeeignete steile oder ueberflutete Standorte
werden abgelehnt, nicht grossflaechig flachgebuegelt. Dazu kommen beschaedigte
Ruinen und abgestuetzte absteigende Minen. Die tatsaechliche Dichte haengt von
Terrain und Eignung ab. Bewohner, Berufe/Handel und Rollen-Gameplay sind **F10**,
nicht durch dekorative Gebaeudenamen vorgetaeuscht.

Tueren, Fenster, Betten, Treppen, Truhen und Oefen verwenden echte vorhandene
Objektzustaende. Schmiede-Oefen erhalten den vorhandenen Betriebszustand.
Rollenspezifische Truheninhalte werden autoritativ einmal angelegt; auch eine
leere oder abgebaute Truhe sowie geerntete Felder werden nicht nachgefuellt.
Objekt-/Crop-/Loot-Daten bleiben in den vorhandenen Journal- und Regionformaten.

## Neue Bloecke

Alle IDs sind hinten angehaengt; bestehende IDs bleiben unveraendert:

| ID | Block | Darstellung / Mechanik |
|---|---|---|
| 102 | `teramic::path` | Ockerfarbener verdichteter Weg, kleine helle Steine, erdige Seiten, Hoehe 15/16. |
| 103 | `teramic::wool_block` | Weisse gewebte Wolle, gemeinsame Schleifentextur und gebuendelte nahe Oberflaechengeometrie. Tierwolle behaelt `teramic::wool`. |
| 104 | `teramic::wool_stairs` | Drei Stufen, dieselben 24 Orientierungen und Eckverbindungen wie andere Treppen. |
| 105 | `teramic::wool_slab` | Horizontale/vertikale Haelften und Kombination mit anderen Slab-Materialien. |

Die alten Mischslab-Codes bleiben unveraendert. Codes 58 bis 63 ergaenzen nur
Wolle als zweite Haelfte. Serverplatzierung, Teilabbau, Vorschau, Kollision und
Speicherung lesen dieselben Profile. Alle vier Bloecke werden in Doerfern
verwendet und besitzen echte Datenrezepte im bestehenden Too-Many-Items-Katalog:
Weg aus Dirt+Gravel, Wollblock aus vier Wollmaterialien, Treppen/Slabs aus Wolle.
Keine neue Entity und kein eigenes GLB pro Standardblock.

## Pet-HUD

Die bestehende Bedienung verwendet nun den vollstaendigen dunkel-goldenen Ring
mit erhoehtem Follow-Segment, runden Medaillons, aeusseren Beschreibungstafeln,
dem wirklichen Tiermodell im Zentrum und Namens-/Herzplakette darunter.
Follow/Rest/None verwenden weiterhin die autoritativen Auftraege; Hover veraendert
keinen Serverauftrag. Gemeinsame Framebilder werden einmal angelegt, nicht pro
Hover/Frame. Physische Fensterabmessungen und UI-Skalierung werden getrennt
beruecksichtigt. Schmale Fenster erhalten eine angepasste Anordnung ohne verdeckte
Bedienflaechen. Es wird kein neues Fox-Mob erfunden, um das Referenzportraet
nachzustellen.

Eine ruhige dunkle Mitte verdeckt das zusaetzliche grosse Tier in der Spielwelt
hinter dem separaten Modellportraet. Das weicht absichtlich von der vollstaendig
transparenten Referenz ab. Das vorhandene Icon-Set liefert fuer None ein
Kreis-X statt des dort gezeigten Verbotssymbols. Aufbau und neue Blockformen
folgen den Bildern; eine pixelidentische Nachbildung wird nicht behauptet.

## Kompatibilitaet

Neue Welten waehlen `structures_revision = 1`. Bestehende Weltmanifeste behalten
ihren gespeicherten Vertrag; ein fehlendes Feld bedeutet 0 und **keine Retrogen**.
Welt-ID, Generator-ID und existierende Spielerbauten bleiben erhalten.
Client und Server muessen gemeinsam aktualisiert werden: **Protokoll 72**.

## Reproduzierbare Orte

`cargo run --locked -p terra-world --example structure_sites` sucht in einem
begrenzten Regionsraster dieselben Plaene, die normale Chunks erhalten. Der
wirklich ausgefuehrte Lauf fand bei Seeds **42042, 1337 und 99173** jeweils alle
drei Dorfstile sowie Ruinen und Minen. Vollstaendige Ausgabe:
[structure-sites.log](structure-sites.log).

Beispiele mit unveraenderten Standard-Terrainparametern und Seed 42042:

| Ansicht | Region X,Z | Mittelpunkt X,Y,Z |
|---|---|---|
| Highlands-Dorf | -1,-1 | -185,90,-152 |
| Normales Dorf | 0,-2 | 186,89,-589 |
| Wuestendorf | -6,-5 | -2120,31,-1707 |
| Ruine | 1,-1 | 617,108,-185 |
| Mine | 0,1 | 166,105,589 |

Die gesamte Standortsuche benoetigte pro Seed 435,78 / 138,61 / 655,71 ms. Das
sind gemessene **Suchzeiten**, keine Framezeiten und kein FPS-Vergleich.
Ein frischer oder bereits warmer Plan-Cache beeinflusst diese Werte.

Native opt-in Fixtures: `--smoke-test-villages` mit
`TERRA_STRUCTURE_SCENE=temperate|desert|highland|ruin|mine` und optional
`TERRA_STRUCTURE_REGION=X,Z`, `--smoke-test-pet-commands` fuer das echte
Netzwerk-Pet-Menue sowie `--smoke-test-materials` fuer neue Bloecke und F9.
Die separaten Configs in diesem Verzeichnis veraendern keine Spielerwelt.

## Validierung

Tatsaechlich ausgefuehrt, ohne neue Dependencies oder herabgesetzte Spielqualitaet:

| Pruefung | Ergebnis |
|---|---|
| `cargo fmt --all --check` | Erfolgreich. |
| `cargo check --locked --workspace` | Erfolgreich, siehe [check.log](check.log). |
| `cargo test --locked --workspace --no-fail-fast` | **2329 bestanden**, 0 fehlgeschlagen, 3 bestehende Opt-in-Tests ignoriert; [tests.log](tests.log). |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Erfolgreich ohne Warnungen; [clippy.log](clippy.log). |
| `cargo build --locked --workspace` | Client und Dedicated Server erfolgreich; [build.log](build.log). |
| Abschliessende Clienttests nach Screenshot-Auslaufanpassung | 107 bestanden, 0 fehlgeschlagen, 1 bestehender Opt-in-Test ignoriert; [client-final.log](client-final.log). |
| `git diff --check` | Erfolgreich. |
| Dedicated Smoke ohne `DISPLAY`/`WAYLAND_DISPLAY`, ohne Save, mit freiem Port | Erfolgreich, 27 Sections; [headless.log](headless.log). |

Die Regressionen decken deterministische Seeds, negative Koordinaten, normale
und gepolsterte Section-Generierung, Reihenfolgeunabhaengigkeit, begrenzte
Caches, Baumkronenfreiraum, beide Brueckenrichtungen mit echtem Wasser darunter,
Objektzustands-/Truhenpersistenz, einmalige Initialisierung, gemischte Wollslabs,
F9-Eingabesperren und die eindeutige Unterscheidung von Tierwolle/Wollblock ab.
Spinnen durchlaufen echte Server-Ticks und UDP-Replikation samt Nestbau,
30-90-s-Simulationspause und anschliessendem Fressen. Der Baumtest prueft
wirkliche Dauer, Abbruch, Fallereignis und begrenzte Drops. Seine Zielsuche nutzt
nun denselben Picker inklusive Props und die bestaetigte Spielerposition;
kurze Fortschrittsereignisse werden nach jedem Update statt in blinden
80-ms-Bloecken beobachtet. Timeouts und fachliche Erwartungen bleiben erhalten.

## Native Bilder

Alle folgenden Fixtures wurden auf der vorhandenen **RX 6950 XT / RADV / Vulkan**
ausgefuehrt. PNGs wurden wirklich gespeichert und visuell angesehen; die
Fixtures pruefen zusaetzlich GPU-Readback und nichtleere Bilder. Die Dorfviews
verwenden dieselben echten regionalen Plaene und pruefen geladene Truhen samt
autoritativen Inventaren, keine separat nachgebauten Kulissen.

| Ansicht | Aufnahme |
|---|---|
| Plains/Forest | [Uebersicht](native/temperate/temperate-overview.png), [Fassade](native/temperate/temperate-close.png). |
| Wueste | [Uebersicht](native/desert/desert-overview.png), [Fassade](native/desert/desert-close.png). |
| Highlands mit beginnendem Schneefall | [Uebersicht](native/highland/highland-overview.png), [Fassade](native/highland/highland-close.png). |
| Ruine | [Uebersicht](native/ruin/ruin-overview.png), [Nahansicht](native/ruin/ruin-close.png). |
| Mine | [Eingang](native/mine/mine-overview.png), [absteigender Stollen](native/mine/mine-close.png). |
| Pet-HUD | [Follow](native/pet/pet-follow.png), [Rest](native/pet/pet-rest.png), [None](native/pet/pet-none.png), [kompaktes Fenster](native/pet/pet-compact.png). |
| Neue Bloecke | [Weg-/Wollmaterialien und Formen](native/materials-village-building-blocks.png). |
| F9 | [Vertikale und horizontale Grenzen](native/materials-chunk-borders.png). |

Der Pet-Test verwendet echte TiltUI-Buttons und Besitzerkommandos ueber den
integrierten UDP-Server. Er prueft Follow-Bewegung, Rest, None, Escape, reale
Modellvorschau, physische Fenster-/DPI-Grenzen, tatsaechliche Glyphenpositionen
und Textpixel in den Tafeln, nicht bloss die Layout-Boxen. Der Materials-Test
hat alle 105 Bloecke und GPU-Inventarvorschauen sowie 13 Ansichten geprueft.

## Grenzen

Keine 10-, 15- oder 30-Minuten-In-Game-Tests und kein erfundener FPS-Vorher-/
Nachher-Vergleich. Plan-, Geometrie-, Entity- und Cache-Aufwand sind begrenzt;
die Standortzeiten oben ersetzen keine Framezeitmessung.

Highlands-Daecher werden nicht pauschal mit permanentem Schnee bebaut. Der
bestehende Wetterzustand lagert Schnee im laufenden Spiel ab; die kurze
Highlands-Aufnahme zeigt beginnenden Schneefall, keine voll verschneite Stadt.

Im Wuesten-Screenshot-Smoke meldet Bevy beim automatischen Beenden gelegentlich
`Failed to send screenshot: sending on a closed channel`, trotz erfolgter
PNG-Speicherung, erfolgreichem Readback und Exitcode 0. Eine 500-ms-Auslaufphase
reduziert dies, beseitigt es auf diesem Backend aber nicht vollstaendig. Das
ist als verbleibende Testmodus-/Shutdown-Grenze dokumentiert, nicht als
fehlerfreier Loglauf ausgegeben. Keine solche Meldung wird als FPS-Fix gewertet.
