# Phase 90: Kollisionsrevisionen, Rauch Und Ofendetails

Stand 2026-10-20. Implementiert, Workspace-Tests und native Laeufe bestanden.

## Ursache Des Zurueckziehens

`EditJournal::set_object_state` erhoehte beim Oeffnen/Schliessen die Meshrevision,
gab deren betroffene Sections aber bisher nicht weiter. `server_furniture::toggle`
veraenderte somit nur das Journal. Anders als bei normalen Blockedits fehlte
die Queue-Invalidierung. `TerrainCollisionState::ready_for_aabb` verweigerte
daraufhin weitere Bewegung, weil die Server-Section ihre alte Revision behielt.
Die Clientkorrektur setzte den lokal weiterlaufenden Koerper wieder zurueck.

Der Journalmutator gibt jetzt seine tatsaechlich betroffenen Halo-Sections
zurueck. Das gemeinsame `streaming::commit_object_state` plant deren vorhandene
asynchrone Neubauten ein. Sowohl Toggle als auch andere Objekttransaktionen
verwenden diesen Weg. Identische Posen invalidieren nichts. Keine Abschaltung
von Kollision, Revisionstests oder Serverkorrektur; kurze Wartezeit waehrend
eines noch laufenden Kollisionsneubaus bleibt absichtlich erhalten.

Der neue Streamingtest schaltet alle sechs Holzbauteile wiederholt ueber eine
negative Section-Ecke und vertikale Tuergrenze. Der native Test prueft nach
Tuer- und Fensteroeffnung die anschliessende Bewegung sowohl des lokalen
Koerpers als auch der bestaetigten Serverposition, noch im Survivalmodus.
Die bisherigen statischen Kapseltests hatten diese Queue-Luecke nicht erfasst.

## Rauch, Licht Und Modelle

- Offene Fenster mit echter Aussenverbindung entfernen jetzt 0,5 Rauchzellen/s.
  Tueren bleiben bei 0,75 je offener Zelle. Mehrere Oefen teilen dieselbe
  Raum-Entlueftungsleistung; der Auslass multipliziert sie nicht je Ofen.
- Sichtbarer Austritt besitzt fuenf Staerkezonen: 0, 1..7, 8..31, 32..95 und
  mindestens 96 angesammelte Rauchzellen. Zugehoerige Quellen am selben
  Auslass werden zusammengefasst. Opazitaet, Breite und Steighoehe werden
  weich angenaehert, statt einen dichten Plume auf einmal einzublenden.
  Es bleiben maximal acht lokale Auslaesse mit je zwoelf Wisps, vier Quellen
  pro Suche und 2-Hz-Suchcache. Kein neuer Draw Call pro Wisp.
- Drei unterschiedliche aufgenommene Husten-Takes, normalisiert und gefiltert,
  spielen leiser, ohne direkte Wiederholung und mit 8,5..12,5 Sekunden Abstand.
  Die bestehende Koerper-Rauchkontaktpruefung und Unterwasser-/Lautstaerkeregeln
  bleiben erhalten. Kein Husten allein durch die Kameraposition.
- Ofenfeuer besitzt hoeheren, gedeckelten Mischpegel und eine auf -24 LUFS
  normalisierte bestehende Aufnahme. Quellen und Bearbeitung stehen in
  `assets/audio/CREDITS.md` und `tools/assets/prepare90-furnace-polish-audio.sh`.
- Fackeln verwenden 1812,5 statt 1450 Lichtintensitaet: +25%. Reichweite,
  Soft-Core-Nahdaempfung, Hoehlenfaktor und Lichtbudgets bleiben erhalten.
- Tueren besitzen dieselben Planken-, Schnitz- und Beschlagdetails auf beiden
  Seiten. Ihre Kollision und gespeicherten Posen werden dadurch nicht vergroessert.
- Gerade Rohrdurchfuehrungen und der obere Ofenblock benoetigen keine komplette
  vierseitige Umrandung mehr. Jeder vorhandene opake Nachbar bildet seine eigene
  Fuellung bis zur aeusseren Rohrwand; fehlende/glaeserne/unbekannte Nachbarn
  erzeugen keine Fuellung. Eckbesitz verhindert ueberlappende Fuellelemente.
  Auswahl, Kollision und Section-Batches verwenden dieselbe Geometrie.

## Validierung

| Pruefung | Ergebnis / Nachweis |
| --- | --- |
| `cargo fmt --all --check` | bestanden, `fmt.log` |
| `cargo check --locked --workspace` | bestanden, `check.log` |
| `cargo test --locked --workspace --no-fail-fast` | 1453 bestanden, 0 fehlgeschlagen, 1 ignoriert, `tests.log` |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | bestanden, `clippy.log` |
| `cargo build --locked --workspace` | bestanden, `build.log` |
| Nativer Integrated Server mit TiltUI | 13 Aufnahmen, `native-gameplay.log` |
| Native Vulkan-Modell-/Shaderfixture | 14 Aufnahmen, `native-art.log` |
| Dedicated Server ohne DISPLAY/WAYLAND_DISPLAY | 27 Sections, sauber beendet, `headless.log` |

Nativer Adapter: AMD Radeon RX 6950 XT, RADV/Vulkan. Beide Clientlaeufe und der
Headless-Lauf enthalten keine ERROR-, Panic- oder ValidationError-Meldungen.
Die Audiobank dekodiert nun 91 Clips erfolgreich. Eine subjektive Hoerabnahme
hat damit nicht stattgefunden. Der vorhandene dreiminuetige UDP-Soak bleibt
standardmaessig ignoriert und wurde hier nicht erneut ausgefuehrt.

Der Survival-Gameplaylauf besteht inklusive **lokaler und bestaetigter
Serverbewegung nach jedem Oeffnen**, unveraenderter Mausbindung, Ofenablauf,
kompaktem Layout, Creative-R und Save/Reload. Der Shader-/Modelllauf umfasst
zwei neue tatsaechlich als Bilder kontrollierte Ansichten:
[Rueckseiten aller Holzarten](captures/furnaces-12.png),
[Rohr mit einem Nachbarblock](captures/furnaces-13.png).
Die automatischen Aufnahmen pruefen ausserdem nicht-leere Pixelinhalte.

Die Unit-/Integrationstests erfassen alle sechs Oeffnungsbloecke, wiederholte
Pose-Commits und fertige Halo-Neubauten an negativen/vertikalen Sectionkanten,
alle 15 nichtleeren Nachbarkombinationen der Rohrfuellung auf drei Achsen,
ueberlappungsfreie Ecken und freies Rohrinneres, geteilten Rauchabzug mit mehreren
Oefen, gespeicherte Bruchteilsbilanz, monotone begrenzte Austrittsstaerke und
drei Husten-Takes ohne direkte Wiederholung. Der Tageszeit-/Hoehlenlichttest
prueft den neuen Sollwert von 8700 statt 6960 bei unveraenderter Lichtkurve.

Der erste Gesamtlauf (`tests-initial.log`) fand zwei Testanpassungen:
Der neue negative Streamingtest hatte seinen Testort ausserhalb des bisherigen
Fokusbereichs gewaehlt; er setzt seinen Fokus jetzt explizit an die gepruefte
Sectionkante. Der bestehende Lichttest enthielt noch den alten harten
1450-Intensitaets-Sollwert. Beide sind im erfolgreichen Gesamtlauf korrigiert.

## Geometrie Und Kosten

Die echten Modellzaehler zeigen fuer die Eichentuer 2712 -> 5112 Vertices,
1356 -> 2556 Dreiecke und unveraendert neun Materialbatches (Phase-89- und
Phase-90-`native-art.log`). Die detaillierte Rueckseite kostet also Geometrie;
eine kostenlose Verdoppelung oder FPS-Paritaet wird nicht behauptet. Die
Kollisionsboxen bleiben unveraendert. Rauchpartikel-/Lichtbudgets steigen nicht.
Es wurden keine neuen FPS-/GPU-Frame-/RSS-Benchmarks durchgefuehrt.

Die vorhandenen Fixtures lassen sich mit `target/debug/terra-client
--smoke-test-furnace-gameplay` beziehungsweise `--smoke-test-furnaces` starten.
`TERRA_CAPTURE_DIR` bestimmt die Aufnahmen. Fuer den Gameplaylauf muss
`TERRA_FURNACE_GAMEPLAY_DIR` auf einen frischen Testpfad zeigen; er verwendet
seine eigene Welt, Identitaet und Konfiguration statt produktiver Spielstaende.

## Grenzen

Gerade Rohrdurchfuehrungen sind automatisch gefuellt, gebogene/schraege
Wanddurchfuehrungen benoetigen weiterhin passende Anschluesse. Rauch bleibt
das vorhandene budgetierte Zellmodell, keine CFD-Simulation. Austrittsstaerken
orientieren sich an der gespeicherten Rauchmenge, nicht an simuliertem Gasdruck.
Husten wird lokal aus repliziertem Rauchzustand ausgeloest, nicht als eigener
entfernter Sprach-/Audioevent repliziert. Eine menschliche Hoerabnahme und
lange Lasttests mit vielen Oefen werden nicht durch technische Audiodekodierung
oder kurze native Laeufe ersetzt. Keine neue FPS-/Speicherparitaet behauptet.

Bestehende Block-/Welt-IDs, Generatorversion 5, Protokoll 51, RGA0/PLY4 und
Dependencies bleiben unveraendert. Client und Dedicated Server mit dem neuen
Code starten; kein manuelles Aendern gespeicherter Welten erforderlich.
