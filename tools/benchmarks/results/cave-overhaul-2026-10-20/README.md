# Phase 83: Terrassierte Hoehlenoekologien Und Echte Kaskaden

## Architektur

Aufbau auf Phase 82, keine zweite Hoehlen-, Wasser- oder Prop-Engine.
Der gemeinsame positionsabhaengige Sampler gilt fuer Kammern, Noise-Gaenge
und Eingangsverbinder. Gewichtete, gewarpte Regionen verwenden weiterhin
45/20/20/15 fuer Normal/Salt/Lush/Cascade und die Durchmesser-Richtwerte
100--400, 80--250, 100--300 und 120--350 m. Der Uebergangsbereich ist 24 m.
Klima, Feuchtigkeit, Tiefe und Oberflaechenbiome modulieren die Eignung;
Revision 4 beruecksichtigt ausserdem die Klimatemperatur der Regionsstandorte.
Gewichte sind keine garantierten Flaechenanteile.

Vorbereitete Spalten und begrenzte Regionscaches vermeiden Regionssuchen pro
Voxel. Connector-Materialien, Feuchte, Breiten und Dekorationen verwenden
dieselben Gewichte wie Kammern. Die vorhandenen Noise-Verbinder besitzen
keine separate Graph-ID: Debug zeigt deshalb Raum/Gang und stabile Region-ID,
nicht eine erfundene Connector-ID.

## Drei Unterschiedliche Oekologien

**Salt:** Unregelmaessige breite Terrassen, meandernde Wasserstellen,
zusammenhaengende cremeweisse und rosige Mineralbaender. Oberflaechenbelag
ist jetzt von Ressourcenerz getrennt. Neue Cube-Bloecke:

| ID | Kennung | Darstellung |
| --- | --- | --- |
| 57 | `teramic::salt_stone` | Heller, rauer mineralischer Stein |
| 58 | `teramic::pink_salt_stone` | Leicht rosiger mineralischer Stein |

Beide verwenden einzelne JSON-Definitionen, die vorhandene prozedurale
Texturerzeugung und das gemeinsame Section-Meshing, keine GLBs.
Bestehende IDs bleiben erhalten. `[cave_biomes].salt_surfaces` startet bei
`[0.55, 0.30, 0.10, 0.05]`: Gestein, Belag, Cluster, Spezialformationen.
Noise-Schwellen setzen diese visuellen Richtwerte um, nicht exakte Anteile.
Kristallreiche und grosse Formationen teilen das regionale Clusterbudget.
Raumcluster besitzen versetzte, unterschiedlich lange breite Schaefte,
Stand-/Haengevarianten und seltene zusammengewachsene Saeulen. Die gemeinsame
Segmentgeometrie ist abgestuft und stumpf bevelled, nicht nadelartig.
Near-Schaft: 56 statt 24 Dreiecke; Fernprofil weiterhin 24.
Interaktion, Supportketten, fallende Segmente und exaktes Picking bleiben
gemeinsam mit der Darstellung.

**Lush:** Unterschiedliche Insel-/Bodenterrassen, feuchte Mooswaende und
gruene Auflagen auf trockenen nach oben offenen Flaechen. Vorhandene
Gras-, Farn-, Busch-, Blumen-, Kleinpflanzen- und Pilzprops werden in
Feuchtigkeits-/Lichtzonen gebuendelt. `lush_density=0.72` ist konfigurierbar.
Laengere geschwungene Ranken mit groesseren Blaettern, dickere Wurzeln,
seltene vorhandene Hoehlenbaeume und leuchtende Hoehlenpilze ergaenzen sie.
Gaenge behalten kurze Dekorationen und freie Durchgangsbereiche.
Props behalten stabile IDs, Section-Batches und gespeicherte Entfernungen.
Es gibt keine neue Prop-Registry und keine Entity pro Blatt oder Kristall.

**Cascade:** Bis zu drei natuerlich begrenzte Beckenstufen mit 6--10 m
Hoehenunterschied, meandernden Kanaelen, abgeschraegten seitlichen Ufern
und freien Fallstrecken. Becken liegen oberhalb des echten Bodens und unter
der Decke; tiefe Weltgrenzen werden respektiert.

## Zellbasiertes Wasser

Deterministische natuerliche Becken sind die bereits vorhandenen
WaterColumns-Reservoirs. Der asynchrone Section-Generator liefert nur die
trockenen Auslaufzellen neben deren Lippen. Beim autoritativen Laden werden
diese in die bestehende Fluid-Warteschlange eingetragen. Replika-Clients
starten keine eigene Simulation. Das vorhandene Budget bleibt
256 Zellen pro 0.1 s; ungeladene Nachbarn schlafen bis zum Reload.

Wasser faellt als echte FluidCells, breitet sich am Boden aus und trifft
auf das tiefere Reservoir. Zustaende verwenden EditJournal, Wasser-Epochen,
asynchrone Wasser-Rebuilds und Regionspersistenz. Terrain-/Collider-Batches
werden nicht fuer jeden Fluidschritt erneuert. Blockierte Auslaeufe nutzen
die vorhandene Block-/Quellenpruefung. Neue Fliesszellen werden nicht
automatisch zu Quellen.

Die alten synthetischen senkrechten Curtain-Quads sind fuer diese Kaskaden
abgeschaltet. Gerendert werden Flaechen des tatsaechlichen Fluidvolumens
mit dem vorhandenen Wassershader. Echte Aufschlagskanten speisen denselben
qualitaetsabhaengigen Gischt-Batch (maximal 96 Partikel), keinen Zusatzpass.
Reservoirs sind erneuerbare Quellen/Senken; dies ist keine globale
volumenerhaltende Grundwasser- oder Flusssimulation.

## Erzadern

Revision 4 nutzt zusammenhaengende dreidimensionale, unterschiedlich grosse
Ellipsoidadern mit glatter raeumlicher Randvariation statt unabhaengiger
Rauigkeit jedes Randvoxels. Salzprospektion verwendet groebere 32-m-Zellen
und stark reduzierte Frequenz, nur mit geeigneter Cave-Salt-Gewichtung.
Ein Teil kann im Gestein liegen. Erz-Hosts umfassen normalen, moosigen und
die neuen mineralischen Steine. Wandaffinitaet prueft das gemeinsame
Kammer-/Tunnel-Dichtefeld. Kohle, Eisen, Gold, Diamant und Rubin sowie
deren Hoehenregeln bleiben erhalten.

## Weltvertraege

Neue Welten verwenden `cave_structure_revision=4`; gespeicherte Vertraege
0/1/2/3 werden nicht angehoben. Generatorversion 5, Welt-IDs und Seeds bleiben
unveraendert. Kein automatischer Retrogen und keine destruktive Migration.
Das Netzwerkprotokoll ist 48: Client und Dedicated Server gemeinsam erneuern.
Reine Segment-Art-Aenderungen gelten auch fuer dargestellte alte Segmente;
gespeicherte logische Bloecke und deren Positionen werden nicht umgeschrieben.

## Reproduktion Und Diagnose

`configs/` in diesem Ordner definiert die neue Testwelt (Seed 42042).
Der native Harness `--smoke-test-cave-structures` sucht echte prozedurale
Raeume und fotografiert je zwei Ansichten. `TERRA_CONFIG_DIR` auf diesen
Config-Ordner setzen, `TERRA_CAPTURE_DIR` auf den gewuenschten Ausgabeordner.
`TERRA_CAVE_CONNECTOR=0`, `1` oder `2` sucht stattdessen einen offenen
Salt/Lush-, Lush/Cascade- beziehungsweise Salt/Cascade-Uebergang.
Die gefundenen Koordinaten stehen im jeweiligen Log. Reproduzierbare Positionen
fuer diese Config, im Spectator-Modus pruefen:

| Region | X / Y / Z |
| --- | --- |
| Salt, Kammerzentrum | 16759.58 / -35.00 / -17942.54 |
| Lush, Kammerzentrum | -17758.80 / 1.97 / -18335.40 |
| Cascade, Kammerzentrum | -17422.20 / -14.15 / -18322.80 |
| Salt/Lush, Connector-Kamera | -1727.50 / -4.50 / -1983.50 |
| Lush/Cascade, Connector-Kamera | -1647.50 / -31.50 / -2015.50 |
| Salt/Cascade, Connector-Kamera | -1727.50 / -14.50 / -2047.50 |

Die Inspektionslichter sind ausschliesslich Testfixtures, keine automatisch
generierten Lampen. Screenshot-Helligkeit ist keine GI-Abnahme.

F1 zeigt Biom/Mischung/Feuchtigkeit, Region und Grenznaehe.
`T / P / C` bezeichnet normierte Klimatemperatur sowie Props und
Kristallsegmente in der aktuellen Section. `Fluid tick` zeigt wirklich
besuchte Zellen und CPU-Zeit des letzten autoritativen Fluid-Takts;
Replikas erhalten keine erfundenen Server-Messwerte.

## Native Ansichten

Elf PNGs unter `captures-final/`: je zwei Salt-, Dripstone-, Lush- und
Cascade-Ansichten sowie die drei gemischten Connector-Paarungen.
Native Vulkan-Laeufe erzeugten alle Aufnahmen; Salt-1, Lush-1,
Waterfall-0 und alle drei Connector-Bilder wurden direkt betrachtet.
Die Bilder zeigen Mineralbaender/Cluster, bewachsene Terrassen, echte
Fallzellen und Moos-/Bodenuebergaenge im bestehenden Renderer.

Die Referenzbilder wurden betrachtet, ihre vollstaendige Bildqualitaet
ist noch nicht erreicht: grosse Felsflaechen und Terrassen wirken teils
regelmaessiger, die Vegetationsschichtung weniger dicht als in den
Illustrationen. Wasser-Aufschlagslichter und Oberflaechenkontrast brauchen
kuenstlerische Abstimmung. Der Salt/Cascade-Testblick trifft bewusst eine
ruhige Zone, nicht einen kristallreichen Vorzeigebereich.
Diese Stichproben pruefen keine komplette Survival-Durchquerung aller
vertikalen Tunnel. Die Freiraum-/Ankertests ersetzen keine globale
Erreichbarkeitsgarantie.

## Abnahme

`cargo fmt --all --check`, `cargo check --workspace`,
`cargo test --workspace --no-fail-fast`,
`cargo clippy --workspace --all-targets -- -D warnings` und
`cargo build --workspace` sind erfolgreich. Der finale vollstaendige
Testlauf umfasst **1293 erfolgreiche Tests**, einen bereits zuvor ignorierten
Langzeit-UDP-Test und keine Fehler. Die ersten Laeufe deckten noch feste
Materialanzahlen in Audio-/Renderingtests und eine unpassende synthetische
Flaechenausrichtung im Connector-Test auf; diese wurden korrigiert.

Der Dedicated Server startete mit entfernten `DISPLAY`/`WAYLAND_DISPLAY`
erfolgreich und beendete den Headless-Smoke-Test mit Exit 0.
Die nativen Raum- und Connector-Laeufe fanden keine Shader- oder
Slab-Allocator-Fehler. Zwei erste Connector-Laeufe meldeten beim sofortigen
Testende einen geschlossenen Screenshot-Rueckkanal nach bereits gespeichertem
PNG. Ein 250-ms-Abschluss-Puffer ausschliesslich im Screenshot-Harness
behebt diesen Shutdown-Race; beide Wiederholungen in
`logs/native-connector-{0,1}-drained.log` enden ohne Fehler.

Die Tests umfassen die neun Connector-Paarungen, Negative-/Section-Grenzen,
Generierungsreihenfolge, Legacy-Vertraege, Belags-/Erztrennung,
unterschiedliche Terrassen, begrenzte Becken, Mineralgeometrie und
Prop-Anker/Freiraum. Ein neuer Test generiert echte Cascade-Sections,
simuliert 300 begrenzte Fluidschritte, prueft abwaerts fallende Nichtquellen
und den identischen Zellzustand nach Export/Import der Regionsdeltas.
Bestehende Tests pruefen Quellblockaden, Rezession, Schlaf-/Reload-Verhalten
und sectionuebergreifende Wassergeometrie.

Rohprotokolle: `logs/workspace-tests-optimized.log`, `logs/clippy-optimized.log`,
`logs/check-optimized.log`, `logs/build-optimized.log`, `logs/fmt-final.log`,
`logs/headless-final.log`.

## CPU-Messung

Identischer 3072-Section-Schnitt, Seed 42042, 256 X/Z-Spalten von -8 bis 7,
je zwoelf Sections Y=-7 bis 4, dev-Profil opt-level 1, kein Renderer.
Drei getrennte Prozesslaeufe pro Variante, keine gleichzeitig laufende
Kompilierung. Erste Messreihe:

| Vertrag | Spalten vorbereiten | Sections/Props | Gesamt |
| --- | ---: | ---: | ---: |
| Revision 3 | 0.254 s | 1.618 s | 1.872 s |
| Revision 4, erste Fassung | 0.262 s | 1.946 s | 2.208 s |
| Revision 4, Belegungs-Vorpruefung | 0.264 s | 1.764 s | 2.029 s |

Die vorhandenen Blockdaten verwerfen nun ungeeignete Prop-Standorte vor
Noise-/Dichteabfragen. Randzellen behalten die globale Nachbarpruefung;
ein Test vergleicht gepaddete Render- und ungepaddete Headless-Sections samt
Prop-IDs. Die Optimierung spart hier 8.1% gegenueber der ersten Fassung.
In dieser ersten Reihe bleiben gegenueber Revision 3 8.4% CPU-Mehrkosten.
Spaeter wurde ein parallel laufendes fremdes Spiel festgestellt; dessen
Einfluss auf diese fruehere Reihe ist nicht isoliert. Sie bleibt als
historische Messung erhalten und ist kein belastbarer Regressionsnachweis.
Pruefsummen: Revision 3 weiterhin `39148176`, Revision 4 vor/nach Vorpruefung
`39963652`. Diese Schnitt-Pruefsumme beweist nicht unveraenderten Inhalt an
allen Weltpositionen. Zusaetzlich wurde eine alte Grass->Dirt-Sonderregel
fuer neue Lush-Connector-Boeden entfernt; der direkte Sampler und Section-
Mesher stimmen dort jetzt ueberein.
Rohdaten: `logs/cpu-rev*.log`.

Finale Wiederholung nach Beenden des fremden Spiels, abwechselnd Revision
3/4, je drei frische Prozesse, keine parallele Kompilierung oder native
Teramic-Szene:

| Vertrag | Spalten vorbereiten | Sections/Props | Gesamt |
| --- | ---: | ---: | ---: |
| Revision 3 | 0.243 s | 1.588 s | 1.832 s |
| Revision 4 | 0.246 s | 1.623 s | 1.869 s |

**2.0% Mehrkosten in diesem Ausschnitt**, nicht allgemeine CPU-Paritaet.
Die Pruefsummen bleiben identisch zu den oben genannten Vertragswerten.
Rohdaten: `logs/cpu-final-rev{3,4}-{1,2,3}.log`.

## Native Messung

RX 6950 XT / RADV / Vulkan, native 1920x1080, Sichtweite 8, Wasser High,
VSync aus, dev-Profil. Je 30 s ruhende Kamera nach dem Laden, keine
gleichzeitige Kompilierung oder zweites Spiel. Desktopdienste und der bereits
laufende Benutzer-Server wurden nicht beendet. Render-/Draw-Diagnose in
beiden Builds aktiviert; beide verwenden dieselben Asset-Dateien.

Das direkte Vergleichspaar nutzt dieselbe Revision-3-Welt, Seed, Kamera,
Geometrie und Config aus dem Phase-82-Ordner:

| Messwert | Vorher | Nachher |
| --- | ---: | ---: |
| FPS | 129.29 | 133.79 |
| p95 / p99 Framezeit, ms | 10.448 / 13.179 | 9.113 / 10.434 |
| Ladezeit, s | 4.237 | 4.147 |
| RSS am Messende, MiB | 896.73 | 914.71 |
| Geladene / gerenderte Sections | 2251 / 2088 | 2251 / 2088 |
| Mesh-Assets / sichtbare Mesh-Entities | 8371 / 4391 | 8371 / 4391 |
| Repraesentierte Props / Prop-Batches | 24209 / 679 | 24209 / 679 |
| Terrain-Komposition, Worker-Summe s | 3.836 | 3.773 |
| Prop-Komposition, Worker-Summe s | 0.181 | 0.178 |
| GPU Early Prepass, Mittel ms | 0.792 | 0.779 |
| GPU Opaque Pass, Mittel ms | 1.580 | 1.550 |
| Indirekte PBR-Draw-API-Aufrufe, letzter Frame | 62 | 62 |

RSS ist kein Peak oder VRAM-Wert. Worker-Summen sind keine Mainthread-
Framezeit. GPU-Werte betreffen einzelne instrumentierte Paesse, keine
vollstaendige GPU-Framezeit. 62 API-Aufrufe sind keine GPU-Subdrawzahl;
diese bleibt nicht verfuegbar. Der kurze Lauf begruendet keine allgemeine
FPS-Verbesserung, Speicherparitaet oder Streaming-Abnahme.

Neue Revision-4-Raeume, **andere Geometrie und Kameras**, separat:

| Szene | FPS | p95 / p99 ms | RSS MiB | Ladezeit s | Sections | Props / Batches |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Salt | 146.67 | 8.304 / 9.298 | 914.52 | 4.616 | 2804 | 6197 / 1309 |
| Lush | 93.71 | 12.803 / 14.934 | 1036.19 | 5.298 | 2041 | 40444 / 1164 |
| Cascade | 104.97 | 11.336 / 12.808 | 974.23 | 5.602 | 2156 | 36205 / 1257 |

Die Props beziehen sich auf alle geladenen Batches, nicht nur den
sichtbaren Bildausschnitt. Lush ist der teuerste dieser neuen Szenen.
Rohdaten mit weiteren Mesh-, Upload- und GPU-Metriken:
`logs/native-before-final.log`, `logs/native-after-rev3-final.log` und
`logs/native-{salt,lush,cascade}-rev4-final.log`.

## Grenzen

- Kein globales unterirdisches hydrologisches Netz; begrenzte lokale Aquifere
  und zusammenhaengende Becken-/Kaskadensysteme.
- Keine neue GI, keine dreidimensionale Volumenstreuung. Vorhandenes
  Cave-Lighting, Emissionsbudget, Nebel und Billboard-Gischt werden verwendet.
- Regionengroessen und visuelle Oberflaechengewichte sind Richtwerte.
- Referenznaehe muss anhand der nativen Aufnahmen menschlich abgestimmt
  werden. Prozedurale Voxelgeometrie ist keine exakte Kopie der Illustrationen.
- Kurze lokale Tests ersetzen keine mehrstuendigen Multiplayer-/Streaming-
  Tests, weitere GPUs oder eine Messung interner GPU-Subdraws.
