# Stalker, Kletterwege, Geschlechter Und Streaming

Phase 116. Keine neuen Dependencies, keine geaenderten Block-IDs und keine
Welt-/Generator-Neuzuordnung. Client und Dedicated Server verwenden Protokoll 67.

## Spinnen

Die bisherige Wandsteuerung pruefte bei mehreren Ausweichwinkeln dieselbe
Richtung. Dazu fehlten Uebergaenge um konvexe Kanten zur Decke. Der bestehende
Bewegungspfad verarbeitet jetzt lokale Wand-/Deckenachsen und gepruefte
Kontaktuebergaenge. Ein begrenzter Oberflaechen-Routencache benutzt dieselbe
physische Bewegung fuer seine Kanten, nicht unabhaengige teleportierbare Punkte.
Routen unterliegen dem bestehenden gemeinsamen Acht-Suchen-Budget.

Gestufte Hoehlenwaende, seitliche Hindernisse, Deckenkontakt, echte Positions-
aenderungen und Abseilen werden getrennt von blossen Verhaltenslabels geprueft.
Landungen pruefen die reale rotierte Auflageflaeche statt ausschliesslich die
Zelle unter der Koerpermitte. Ein echter Absatz darf stuetzen; entfernte
Stuetzen loesen wieder Gravitation aus. Ein neu in den Koerper gesetztes
Hindernis beendet einen blockierten Seilzustand, erlaubt aber kein Fliegen
oder kollisionsfreies Entkommen durch feste Bloecke.
Besitzerdaten und autonomer Netzbau bleiben erhalten; Folgen wird nicht aktiviert.

## Geschlechter Und Zucht

`CreatureSex::{Male,Female}` wird mit einem stabilen, gemischten 64-Bit-Hash aus
der unveraenderlichen Tier-ID bestimmt. Natuerliche Tiere, Command-Spawns,
neuer Nachwuchs und alte gespeicherte Tiere besitzen damit sofort ein
deterministisches Geschlecht. Bewegung, Alter, Variante und Reload aendern es
nicht. Das vorhandene binaere Creature-Layout bleibt unveraendert.

Neue F06-Paarungen erfordern unterschiedliche Geschlechter. Alle bisherigen
Regeln fuer Erwachsene, Besitzer, Gesundheit, Futter, Naehe, freie Geburtsstellen,
Wachstum, Cooldowns und Population gelten weiter. Das Geschlecht eines Kindes
folgt seiner eigenen Geburtsidentitaet, nicht dem Geschlecht eines Elternteils.
Bestehende historische Familien werden nicht nachtraeglich geloescht oder fuer
ungueltig erklaert. Das Ziel-HUD zeigt Maennlich/Weiblich.

## Stalker

Registrierung: `teramic::stalker`, JSON `assets/terra/mobs/stalker.json`.
Die neue Enumvariante ist angehaengt; bestehende numerische Mobvarianten bleiben
erhalten. Natuerliche Spawns sind seltene, gedeckte und begehbare Hoehlenstellen.

Die KI nutzt die vorhandene Bodenbewegung mit einer begrenzten Cave-Route-
Erweiterung. Sie sucht Spieler bis 50 m ueber geladene, gedeckte Gaenge und
verlaesst Hoehlen nicht. Unter 15 m wechseln Pausen/Blickkontakt/Annaeherung.
Gesehenwerden fuehrt zum Zurueckweichen; Verfolgen oder Schaden erzuernt ihn.
Ignorierte nahe Spieler koennen nach einer Vorlaufzeit gegriffen werden, aber
nur mit einem echten begehbaren Weiterweg. Fackeln in der Hand, platzierte
Fackeln und die vorhandenen nahen Lampen vertreiben ihn. Auch die Fackel eines
anderen Spielers sowie natuerliche leuchtende Hoehlenpilze werden mit echter
Verdeckung beruecksichtigt. Brennende Oefen pruefen ihre gedrehte Feuerseite
ausserhalb der eigenen Huelle. Der Pilztest beschraenkt sich auf den lokalen
Vier-Meter-Bereich und nutzt das vorhandene Such-/Lichtcache-Budget.

Echte Pfadkurven werden von harmlosen Grid-Zickzacks unterschieden. Vor einer
verdeckten Ecke haelt er kurz an und tritt ueber den vorhandenen gesweepten
Pfad zu einem tatsaechlich sichtbaren Punkt vor. Hoechstens acht vorhandene
Pfadpunkte, begrenzte lokale Sichtpruefungen und ein kurzlebiger Zustand je Stalker;
kein zweiter Deckungs- oder Tunnelplaner.

Der Griff publiziert nur an sein Opfer ein kurzlebiges Bewegungsziel. Server,
Client-Prediction und Reconciliation verwenden dieselbe normale Terrain-
Kapselkollision, Schrittlogik und hoechstens 2,3 m/s Zuggeschwindigkeit. Keine
Flugerlaubnis, kein frei akzeptierter Client-Grab und kein Teleport. Ungueltige,
alte oder nicht mehr erneuerte Ziele verfallen nach 350 ms. Tod, Teleport,
Fackelabwehr und nicht verletzbare Spielmodi beenden den Zug. Neue Hindernisse
und fehlender Fortschritt brechen den Griff ab. Das Entfuehrungsbudget wird aus
dem tatsaechlich zurueckgelegten Spielerweg gezaehlt und auf 40 m begrenzt.

Ein geteilter Suchhaushalt begrenzt Cave-Routenarbeit, ein begrenzter Mind-Cache
haelt Kurzzeitverhalten. Unbekannte oder ungedeckte Regionen sind keine
unsichtbaren Wege. Bestehendes Streaming wird nicht durch AI-Pfadproben erweitert.

Das Modell benutzt wiederverwendbare Rindenlagen, Moos, Zweige, lange bewegliche
Arme/Unterarme/Haende und bernsteinfarbene Augen. Keine Entity je Rindenteil.
Ruhiges/grabendes Atmen, Kratzen und Griffschrei verwenden vier begrenzte
Audioclips im vorhandenen raeumlichen Audio-/Hoehlenhallpfad.
Elf gemeinsame Gelenkmeshes enthalten insgesamt 46.560 Vertices / 23.280
Dreiecke. Das feste Budget liegt bei 64.000 Vertices; bestehende Grenzen fuer
andere Arten bleiben unveraendert. Gang, Atmung, Peek, Recoil, Griff und Angriff
sind prozedural animiert, keine handanimierten Motion-Capture-Clips.

## Wetter

`SurfaceWeatherProbe` liest Klima und Boden-/Wasserhoehe aus vorbereiteten
Spalten. Kamerahoehe wird fuer die Niederschlagsart nicht mehr verwendet.
Bei noch fehlenden vorbereiteten Daten existiert ein einzelner gecachter
16x16-Fallback statt erneuter Noise pro Frame. Darstellung und Ambiente
verwenden dieselbe Sampling-Funktion; reale Pflanzen-/Bodenberechnung bleibt
am vorhandenen hoehenabhaengigen Klimapfad.

Ozeane moderieren kalte kontinentale Grundtemperaturen vor den saisonalen
Offsets. Dadurch kann die fliegende Kamera einen Fruehlingsozean nicht in
Schneewetter verwandeln. Winter bleibt lokal kalt; Highlands behalten die
gewuenschte Schneeregel und `/weather set snow` bleibt ein echter Override.

## Performance: Vorherbefund

Optimiertes Dev-Profil, RX 6950 XT / RADV / Mesa 26.2.4 / Vulkan. Einzelne native
Laeufe ohne parallelen Cargo-Prozess, integrierter Server, reale Netzwerkeingaben,
vollstaendiges HUD und Minimap. Kein FPS-Limit und kein VSync. Je 120 Sekunden
Messfenster nach Szenenbereitschaft, Aufstieg und 600-m-Hin-/Rueckflug mit 12 m/s.
Es wurden keine neuen 10-/15-/30-Minuten-Laeufe gestartet.

| Szenario | Seed | Sichtweite | Physische Aufloesung | FPS vorher | p95 ms | p99 ms |
|---|---|---:|---|---:|---:|---:|
| Highland, Clear | 42042 | 12 | 1920x1080 | 123,61 | 12,306 | 15,460 |
| Kuesten-/Wasserregion, Rain | 2262437629133911810 | 20 | 3840x2160 | 39,18 | 43,875 | 51,011 |

Der zweite Lauf reproduziert den Einbruch: seine 30-s-Fenster messen
54,35 -> 32,99 -> 34,50 -> 34,26 FPS. Mesh-/Material-/Bildzahlen wachsen dabei
nicht unbegrenzt; der grosse Terrain-Arbeitsvorrat verursacht jedoch viel
Hauptthreadarbeit. Im zweiten Fenster braucht `terrain.dispatch` 13,149 ms je
Aufruf (13,017 s in 30 s), obwohl Vorbereitung und Auswahl nur einen kleinen
Teil davon ausmachen. In einer separaten CPU-Stichprobe liegen unter anderem
721 Samples in `interactive_edit`, 368 in der Sortierclosure und zahlreiche
weitere in ihren Hash-/Prioritaetsfunktionen. Das sind CPU-Samples, keine
addierbaren Framezeiten. Vollstaendige Caller liegen nur fuer einen Teil vor.

Konkrete Korrekturen:

- Queue-Prioritaeten werden hoechstens einmal pro Kandidat berechnet statt
  pro Sortiervergleich. Editvorrang und Distanzreihenfolge bleiben identisch.
- Wetter-/Kuestensnapshots werden erst nach Matching-/Empty-Geometriepruefung
  erstellt. Unveraenderte LOD-Daten benoetigen keine neue Aufnahme.
- Echte Blockspalten und ihr Shore-Halo invalidieren die Exposition lokal.
  Reine Gameplay-/Wetness-Aenderungen leeren nicht mehr den kompletten Cache.
- Leere Sky- und tiefe Innenraum-Publikationen werden nicht als neues Dach
  interpretiert. Echte neu geladene/entfernte Daecher bleiben erfasst.

Ein erster `before12-rain`-Versuch endete mit Timeout und wird nicht als
Benchmark gezahlt: der Highland-Standort erhielt gemaess bestehender Regel
Schnee, waehrend das Szenario Regen erwartete. Der erfolgreiche 12er-Lauf
verwendet deshalb Clear. `diagnostic20-rain` ist eine getrennte instrumentierte
CPU-Stichprobe, kein weiterer unverfaelschter FPS-Vergleich.

## Performance: Nachher

Verglichen werden jeweils derselbe Seed, dieselbe GPU, derselbe vollstaendige
GraphicsConfig-Wert, dieselbe physische Aufloesung und dieselbe echte Route.
Die beiden Zeilen untereinander sind hingegen kein isolierter Wetter- oder
Sichtweitenvergleich: Aufloesung und Seed unterscheiden sich.

| Szenario | FPS vorher / nachher | p95 ms vorher / nachher | p99 ms vorher / nachher | Initiales Laden s vorher / nachher | RSS am Ende MiB vorher / nachher |
|---|---:|---:|---:|---:|---:|
| Clear, Seed 42042, 1080p, Sichtweite 12 | 123,605 / 121,734 | 12,306 / 12,071 | 15,460 / 14,885 | 9,491 / 9,260 | 1524,95 / 1505,72 |
| Rain, Seed 2262437629133911810, 4K, Sichtweite 20 | 39,185 / 71,335 | 43,875 / 25,160 | 51,011 / 29,655 | 18,835 / 16,639 | 2146,14 / 2102,51 |

Das sind die abschliessenden Native-Smoke-Werte mit ausgesparten Screenshot-
Nachbarframes. Sie werden nicht mit Mittelwerten aus den folgenden kompletten
30-s-Fenstern vermischt. Clear liefert keinen FPS-Gewinn (-1,51 %); der zuvor
problematische Rain-Lauf gewinnt 82,05 % bei gleichzeitig kuerzeren langen
Frames. Das ist ein einzelner Vergleich je Szenario, kein Konfidenzintervall.

| Messfenster bis | Clear vorher / nachher FPS | Rain vorher / nachher FPS |
|---|---:|---:|
| 30 s | 120,58 / 113,13 | 54,35 / 67,66 |
| 60 s | 126,57 / 126,01 | 32,99 / 71,62 |
| 90 s | 125,09 / 125,55 | 34,50 / 71,47 |
| 120 s | 121,55 / 121,71 | 34,26 / 73,61 |

Im kritischen zweiten Rain-Fenster sinkt `terrain.dispatch` von 13,149 auf
1,252 ms je Aufruf. Die Gesamtzeit sinkt von 13.017 auf 2.690 ms, obwohl
990 vorher nun 2.149 Aufrufe gegenueberstehen. Aufrufe ueber 8 ms: 843 -> 0
in diesem Fenster. Der neue gemessene Prioritaetssortierpfad benoetigt dort
0,017 ms je Aufruf. `props.dispatch` sinkt gleichzeitig von 1,822 auf 0,764 ms.
Verschachtelte Scopes sind nicht additiv und werden nicht zu einer erfundenen
gesamten CPU-/GPU-Framezeit summiert.

Einzelspitzen sind nicht verschwunden: Die kompletten ersten Soak-Fenster,
die auch den anfangs angeforderten Capture enthalten, erreichen maximal
157,60 / 155,59 ms vorher / nachher. In den folgenden drei Rain-Fenstern
sinken die Maxima von 72,30 / 68,13 / 80,05 auf 43,03 / 47,96 / 40,97 ms.
Das ist keine behauptete feste FPS-Untergrenze; die Ursache jeder einzelnen
Spitze wurde nicht separat isoliert.

Mehr Geometrie wird rechtzeitig fertig: Am Ende des Rain-Laufs sind 4.335 statt
2.506 Sections dargestellt, 13.985 statt 10.059 Mesh-Assets vorhanden und
8.053 statt 5.979 Mesh-Entities sichtbar. Wasser bleibt bei 987 Tiles;
94.535 / 94.538 Props sind vorhanden. Diese unterschiedlichen Counts sind
kein Qualitaetsverlust oder Leak-Nachweis: Die Pipeline arbeitet schneller.
Die Materialzahl steigt mit dem neuen Mob von 255 auf 256, Bilder bleiben 439.
Exposure-Gesamtarbeit ist wegen der groesseren Zahl fertiger Meshjobs nicht
direkt als identischer Snapshot zu vergleichen; ihre Invalidierungsregeln
werden zusaetzlich gezielt getestet.

Beide Nachherlaeufe beenden sich mit Exit 0, ohne ERROR/WARN. Fuer Fruehling
an der Kuestenregion meldet die Darstellung wirklich Rain 0,78 und Snow 0,0.
Die native GPU-Passdiagnostik beschreibt einzelne Paesse, keine gesamte
GPU-Framezeit. Der 120-s-Vergleich ersetzt keine Langzeit-/Mehr-GPU-Abnahme;
5 / 10 / 15 / 30 Minuten wurden nicht gemessen.

Rohdaten: [vorher Clear](logs/before12-clear.log),
[nachher Clear](logs/after12-clear.log),
[vorher Rain](logs/before20-rain.log),
[nachher Rain](logs/after20-rain.log). Dazu jeweils `*.summary.json` und
`*.timeline.csv` im selben Berichtsordner. Der vorhandene Soak-Summarizer
interpoliert keine fehlenden Zeitpunkte.

Die tatsaechlich verglichenen Binaerdateien sind lokal als
`target/phase116/terra-client-before` und
`target/phase116/terra-client-after-bench` erhalten. Nach der Messung wurden
nur das Testpodest der Modellgalerie und der Lichtbeobachter-Randfall fuer
unverwundbare Spieler nachgezogen; am gemessenen Terrain-/Wetter-/Render-
Budget wurde nichts nachtraeglich veraendert. Keine Grafikoption wurde zur
Kaschierung des Problems herabgesetzt.
Die beiden tatsaechlich gemessenen Binaerdateien sind durch
[SHA-256-Pruefsummen](benchmark-binaries.sha256) identifizierbar.

## Native Bilder

Die Modellgalerie wurde mit dem tatsaechlichen Vulkan-Client ausgefuehrt.
Alle fuenf Capture-Callbacks pruefen ein nichtleeres Bild; Front, Seite,
Ruecken, Griff und Angriff wurden auch manuell betrachtet. Die neutralen
Galeriekoordinaten sind kein natuerlicher Stalker-Spawn ausserhalb einer Hoehle.
Ein anfangs koplanares Testpodest wurde um 0,05 m angehoben und die Galerie
erneut erfolgreich aufgenommen; das ist keine Aenderung von Spielbloecken.

- [Front](native/stalker/stalker-0.png)
- [Seite](native/stalker/stalker-1.png)
- [Ruecken](native/stalker/stalker-2.png)
- [Griff](native/stalker/stalker-3.png)
- [Angriff](native/stalker/stalker-4.png)

Die Galerie zeigt tatsaechliche Meshes und animierte Zustaende, erzwingt ihre
Verhaltensposen aber ausdruecklich. Sie ist kein behaupteter nativer
End-to-End-KI-Kampf. Reale Hoehlenbewegung, Ecke, Griffweg, Licht und Netzwerk-
Kollision sind durch separate echte Voxel-/Physik-/UDP-Regressionen abgesichert.
Eine menschliche Hoerabnahme und eine unbelegte 1:1-Art-Abnahme werden nicht
behauptet.

Auch die kurze F06-Szene lief erfolgreich mit der aktualisierten Paarung:
zwei echte Owner-Fuetterungen, ein Kind, zwei verbrauchte Futteritems,
Eltern-/UUID-/Cooldown-Replikation, Geschlecht und physisches Wachstum.
Fuenf Aufnahmen unter [native/breeding](native/breeding); fuer die kurze
Wachstumspruefung besitzt ein vorbereitetes aelteres Kind bereits gespeichertes
Alter. Es wurde nicht 20 reale Minuten aufgewachsen.

## Validierung

Alle folgenden Abschlussbefehle wurden auf dem endgueltigen Quellstand mit
Exit 0 ausgefuehrt; schwere Builds liefen einzeln mit `CARGO_BUILD_JOBS=1`.

| Pruefung | Ergebnis | Rohlog |
|---|---|---|
| `cargo fmt --all --check` | bestanden | [fmt](logs/fmt-final.log) |
| `cargo check --locked --workspace --all-targets` | bestanden | [check](logs/check-final.log) |
| `cargo test --locked --workspace --no-fail-fast` | 2.132 bestanden, 0 fehlgeschlagen, 3 ausdrueckliche Opt-in-Tests ignoriert | [tests](logs/tests-verified.log) |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | bestanden | [clippy](logs/clippy-final.log) |
| `cargo build --locked --workspace` | Client und Dedicated Server bestanden | [build](logs/build-final.log) |
| `git diff --check` | bestanden | keine Ausgabe |
| Nativer headless Server, `--bind 127.0.0.1:0 --no-save --smoke-test` | echter UDP-Start, 27 Sections, Exit 0 | [server](logs/server-native.log) |
| Zwei native Benchmarklaeufe | je 120 s, Exit 0 | [clear](logs/after12-clear.log), [rain](logs/after20-rain.log) |
| Native Stalker-Modellgalerie | 5 nichtleere Captures, Exit 0 | [galerie](logs/stalker-native.log) |
| Native F06-Zuchtpruefung | 5 Captures, 2 Fuetterungen, 1 Kind, Exit 0 | [zucht](logs/breeding-native.log) |

Die endgueltigen World-Tests enthalten 520 erfolgreiche Faelle, darunter
16 Stalker-Regressionen. Vier gezielte Spinnenfaelle pruefen alle drei Groessen
ueber gestufte Waende, Decken, seitliche Ausweichwege, Seilabstiege, echte
Absatzauflagen und entfernte/neue Hindernisse. Eine deklarierte Verhaltens-ID
ohne reale Positionsaenderung genuegt nicht. Die Meshqueue-Regression prueft
8.192 Kandidaten auf identische Prioritaet bei hoechstens einem teuren Lookup
je Kandidat. Exposure-Tests unterscheiden Erscheinungs-/Inventarupdates,
lokale echte Blockaenderungen, leere Sky-Publikationen und wirkliche Daecher.

Stalker-Tests pruefen echte L-Tunnel, negative Koordinaten, verschlossene Waende,
Tageslicht, die 50-m-Grenze, 15-m-Eintrittspause, Platz-/Hand-/Retterfackeln,
gedrehte brennende Oefen, natuerliche Leuchtpilze, den gemeinsamen Suchhaushalt,
LRU-Eviction, Wegziehen und Blockade-/Moduswechsel-Freigabe. Unverwundbare
Beobachter koennen leuchten, aber niemals Ziel, Opfer oder Griffempfaenger sein.

Die Netzwerk-Regression verwendet zwei reale lokale UDP-Clients. Nur der
Empfaenger wird durch normale Survival-Physik gezogen; die Kapsel stoppt vor
einer sieben Bloecke hohen Wand, bekommt keine Flug-/Sprungfreigabe und wird
beim Auswaehlen einer Fackel wieder freigegeben. Ein vertrauter Testhost
injiziert dafuer ausdruecklich den autoritativen Griff; die KI-Entscheidung
selbst wird getrennt gegen echte Voxelgeometrie geprueft.

Geschlechtstests pruefen stabile alte/neue Identitaeten, negative Koordinaten,
Verteilung, Reload und unveraendertes Geschlecht beim Bewegen/Wachsen.
Gleichgeschlechtliche Tiere verbrauchen ihre Zuchtbereitschaft nicht erfolglos.
Fruehlingskuesten-, Winter-, Highland-, Hoeheninvarianz-, Cache- und explizite
Wetterkommando-Tests decken die Niederschlagskorrektur ab.

## Grenzen

Die Hoehlennavigation verwendet geladene, tatsaechlich begehbare Zellen.
Unbekannte Dachzellen und mehr als 48 m freie Hoehe werden konservativ
abgewiesen. Alle Ziel-/Weiterweg-Suchen eines KI-Schritts teilen maximal 512
Knoten; komplexe gueltige Wege koennen deshalb spaeter erneut versucht werden.
Die 40 m sind Wegstrecke, kein garantierter Hoehenabstieg. Hoechstens 64
kurzlebige Stalker-Minds werden per LRU gehalten; es gibt keinen unbegrenzten
neuen Weltcache. Lichtbeobachter werden auf 128 Spieler begrenzt; nur Survival
bleibt angreifbar, waehrend auch unverwundbare Beobachter Fackellicht liefern.

Einzelne Streaming-Spitzen bleiben messbar. Aus zwei Minuten folgt keine
garantierte dauerhafte FPS-Untergrenze; A05 in TODO bleibt deshalb offen.
Die lokalen UDP-Tests ersetzen keine LAN-/Internet-Abnahme zwischen Rechnern.
