# Regionales Wachstum, Tierklima und saisonales Eis (F14)

F14 verbindet bestehende Bodenumwandlung und Tierbedürfnisse mit dem lokalen
Klima und ergänzt eine saisonale Eiskruste auf natürlichen Wasseroberflächen.
Die Änderungen laufen in der aktiven Simulation. Dafür wird keine neue
Generatorrevision benötigt und kein gespeichertes Gelände neu erzeugt.

## Bedienung und Konfiguration

Das Verhalten startet automatisch in geladenen Weltbereichen. Es benötigt keine
zusätzliche Taste und keinen neuen Spielmodus. Für einen vollständigen
Jahreszeitenzyklus gelten die vorhandenen Einstellungen in
`configs/world.toml`:

```toml
water = true

[time]
day_length_seconds = 1440.0
start_hour = 10.0
paused = false

[seasons]
enabled = true
spring_days = 7
summer_days = 7
autumn_days = 7
winter_days = 7
start_season = "spring"
transition_days = 2.0
```

Bei diesen Werten dauert ein Welttag regulär 24 echte Minuten. Gespeicherte
Welten verwenden ihre gespeicherten Welt- und Kalendereinstellungen; eine
Änderung der Vorlage setzt ihren Kalender nicht zurück. Ohne Jahreszeiten
bleiben regionale Temperatur, Höhenlage und Feuchtigkeit wirksam, der saisonale
Temperaturaufschlag entfällt. `water = false` deaktiviert die saisonale
Eissimulation. Ein angehaltener `WorldClock` pausiert Graswachstum und Eis;
bereits gesammelte Gefrier-/Tauzeiten werden beim Pausieren verworfen.

Zum Beobachten einen geladenen natürlichen Teich oder See in einer ausreichend
kalten Region aufsuchen und die Zeit laufen lassen. Nicht jeder Winterstandort
wird kalt genug, und eine Schneewetterlage allein erzwingt kein Eis. Zum
Beobachten des Grases muss normale Erde (Material 2) an einen Grasblock
(Material 1) grenzen und nach oben geeignet frei liegen. Ein beliebig
platzierter Eisblock rutscht ebenfalls, taut aber nicht automatisch.

## Lokales Klima und Graswachstum

Die Klimareaktion verwendet vorbereitete Oberflächendaten, Höhe, vorhandene
Wetterdaten und die weich überblendete Jahreszeit. Die bestehenden saisonalen
Temperaturverschiebungen sind Frühling 0 °C, Sommer +6 °C, Herbst −2 °C und
Winter −16 °C. Höhe, Ozeaneinfluss und die vorhandene Hochlandregel verändern
die tatsächliche lokale Temperatur zusätzlich.

Feuchtigkeit ist ein Wert von 0 bis 1. Sie berücksichtigt regionale Feuchte und
Trockenheit, Wetternässe, Regen und beim Gras auch die gespeicherte Nässe der
Bodenoberfläche. Die Wachstumsrate ist das Produkt dreier stetiger Faktoren:

| Einfluss | Regel |
| --- | --- |
| Kälte | Kein Wachstum bis einschließlich 1 °C; Anstieg bis zum vollen Faktor bei 10 °C |
| Hitze | Voller Faktor bis 27 °C; Abnahme bis zum Stillstand bei 42 °C |
| Feuchtigkeit | Kein Wachstum bis 0,12; voller Faktor ab 0,60 |

Normale Erde breitet Gras weiterhin nur in der Nähe eines vorhandenen
Grasblocks aus, einschließlich diagonaler Nachbarn im Abstand von einer
Zelle. Feuer, stark verkohlter Boden, hinreichende Schneebedeckung, Wasser oder
eine ungeeignete Belegung darüber verhindern die bestehende Umwandlung.
Fehlende vorbereitete Klimadaten lassen den Kandidaten warten.

Nach der Aufnahme in die Warteschlange beträgt die erste Verzögerung
positionsabhängig 8–16 Sekunden. Ein sichtbarer Grastrieb durchläuft anschließend
vier Größenstufen; weitere Schritte benötigen `8 / max(Wachstumsrate, 0,05)`
Sekunden. Bei optimalen Bedingungen sind das ungefähr 40–48 Sekunden bis zur
Umwandlung, zuzüglich Erkennung und Budgetwartezeit. Bei Rate null wird alle
acht Sekunden erneut geprüft. Der gespeicherte Trieb wird bei Frost oder
Trockenheit nicht gelöscht; eine bauliche Blockierung kann ihn weiterhin
entfernen. Es gibt keine allgemeine Umwandlung der besonderen Spirit-/Blood-
Bodenmaterialien in normales Gras.

Die vorhandene Grasverarbeitung läuft höchstens einmal pro Sekunde. Sie
untersucht bis zu zwei geladene Sections zur Entdeckung, entnimmt höchstens
128 Kandidaten und bearbeitet höchstens acht fällige Wachstumsstellen.
Warteschlange und aktive Stellen sind auf 4096 beziehungsweise 256 begrenzt.
Die neuen Klimaabfragen lösen keine zusätzliche Terrain-Generierung aus.

## Tierverhalten

Die autoritative Bewohnersimulation verwendet dieselben lokalen Klimaquellen:

- Kühe, Schweine, Schafe, Pferde und Bären verlieren oberhalb von 20 °C
  zusätzliche Wasserreserven, bei Trockenheit stärker. Unter 5 °C steigt ihr
  zusätzlicher Nahrungsverbrauch. Die bestehenden Such-, Trink- und Fresswege
  reagieren auf diese tatsächlichen Bedürfnisse.
- Gut versorgte Tiere dieser Arten können bei mehr als 32 °C und Feuchtigkeit
  unter 0,25 eine Mittagsruhe einlegen. Das Zeitfenster ist Weltphase
  0,42–0,64, ungefähr 10:05–15:22 Uhr. Es gilt nur für ruhende, wandernde oder
  schlafende Tiere mit mehr als 85 % Nahrungs- und Wasserreserven.
- Bienen verwenden bei weniger als 8 °C oder Niederschlagsintensität über 0,6
  den vorhandenen Ruhe-/Nest-Rückkehrpfad. Ein passendes vorhandenes Nest und
  erreichbare Wege bleiben notwendig; es wird kein Nest erzeugt.

Hunger, Durst, Flucht und bestehende Befehlszweige behalten ihre Bedeutung.
Villager, aquatische Tiere, tote Tiere sowie Tiere mehr als vier Meter unter
der örtlichen Geländeoberfläche erhalten diese neue Klimareaktion nicht.
Es entsteht weder eine globale Wanderungs-KI noch ein neues NPC-Kältesystem.
Die Klimaauswertung selbst fordert keine zusätzlichen Wege oder Chunks an.

## Gefrieren und Schmelzen

| Vorgang | Lokale Temperatur | Erforderliche beobachtete Zeit |
| --- | --- | --- |
| Wasser → Eisblock 49 | höchstens −2 °C | mindestens 45 Sekunden |
| Markiertes Natureis → Wasser | mindestens +2 °C | mindestens 30 Sekunden |

Zwischen −2 °C und +2 °C bleibt der Zustand erhalten. Die getrennten Grenzen
verhindern ein schnelles Hin- und Herschalten. Ein Temperaturwechsel außerhalb
der jeweiligen Bedingung verwirft deren Fortschritt. Zeitangaben sind laufende
Simulationssekunden, bei normalem Betrieb echte Sekunden; Erkennung,
Spaltenumlauf und Edit-Budget können die sichtbare Reaktion verzögern.

Gefrieren kann ausschließlich die oberste Zelle einer vorbereiteten natürlichen
Wassersäule. Ihr tatsächlicher Wasserzustand muss dem natürlichen Reservoir
entsprechen: Quellwasser mit Menge 1–8 und Strömung `[0, 0]`. Entnommenes Wasser,
endliche Fließzellen und vom Spieler gefüllte Tanks werden nicht in Natureis
umgewandelt. Über der Zelle müssen mindestens 16 geladene Zellen frei sein;
unverändertes natürliches Laub ist dabei erlaubt. Die Prüfung reicht bei Bedarf
bis über die lokale Geländehöhe, höchstens 64 Zellen. Unbekannte Bereiche und
Spielerdächer verhindern neues Eis. Ein später über Natureis gebautes Dach
verhindert dessen reguläres Auftauen nicht.

Die Simulation bearbeitet höchstens eine vorbereitete 16×16-Spalte alle
0,5 Sekunden: maximal 256 Oberflächenprüfungen und vier erfolgreiche Blockedits
pro Durchlauf. Es werden keine Chunks nachgeladen. Für den Umlauf werden die
Metadaten der bereits geladenen Spalten gesammelt. Gleichzeitig werden
höchstens 16.384 Fortschrittsstellen gehalten; nach 600 Sekunden ohne erneute
Beobachtung wird ein Eintrag verworfen. Ein einzelner Beobachtungsabstand zählt
höchstens 120 Sekunden. Dadurch kann ein lange ungeladener Bereich nicht beim
Wiederkommen unbegrenzt Zeit nachholen.

`terra_world::weather::ice::IceStatus` enthält aktuelle Prüf-/Editzahlen,
verfolgte Stellen sowie kumulierte Gefrier-/Tauereignisse. Das ist eine
Diagnoseressource für Tests und Werkzeuge, keine zusätzliche HUD-Anzeige.

## Wasserherkunft, Speicherung und Multiplayer

Natureis erhält einen materialgebundenen `BlockObjectState`. Für Material 49
speichert dessen `facing`-Wert die ursprüngliche Wassermenge 1–8; es ist hier
keine Ausrichtung. `peer` bleibt leer und `secondary` falsch. Damit ist
saisonales Eis von einem normalen, vom Spieler gesetzten Eisblock unterscheidbar.

Beim Auftauen oder manuellen Abbau dieses markierten Eises zu Luft stellt das
Edit-Journal die ursprüngliche Quellwassermenge mit ruhender Strömung wieder
her. Das gilt auch für teilweise gefüllte Oberflächenzellen. Wird die Stelle
stattdessen absichtlich mit einem festen Block überbaut, bleibt dieser Block
maßgeblich. Saisonale Updates ersetzen keine Spielerbauten und erfinden keine
zusätzlichen Wasserquellen.

Blockänderung, Herkunftsmarker und späterer Wasserzustand verwenden die
bestehenden Section-Revisionen, Region-Dateien und `SectionDelta`-Übertragung.
Die Live-Edits aktualisieren die bestehenden Mesh-, Wasser- und Kollisionspfade.
Vor dem Gefrieren wird Revisionsspielraum für Block- und Metadatenänderung
geprüft. Die gemeinsame Datenstruktur erhält keine neuen serialisierten Felder;
die neue Bedeutung von Eis-Metadaten wird zusammen mit den neuen Block-IDs
durch Protokoll **145** gegen ältere Clients abgegrenzt.

Nur eine Welt mit autoritativem `WorldAccess` führt Wachstum und saisonale
Eisedits aus. Replika-Clients übernehmen bestätigte Deltas und erzeugen beim
Import kein zweites Quellwasser. Die lokale Rutschbewegung ist davon getrennt:
Client-Prediction, Replay und Server verwenden denselben Character-Motor und
dieselben bestätigten Block-/Journalabfragen.

## Rutschen und Grenzen

Fünf Bodenproben unter der Spielerkapsel bestimmen den Eisanteil. Auf Eis wird
die horizontale Geschwindigkeit weich an den Bewegungswunsch angenähert;
Loslassen und Richtungswechsel behalten zunächst Schwung. Mischflächen bremsen
stärker. Reine Nichteisflächen verwenden das bisherige direkte Ansprechverhalten.
Die Anpassung ist exponentiell zeitabhängig und verändert weder Sprunghöhe noch
vertikale Geschwindigkeit. Sie greift nur am Boden, außerhalb des Schwimmens
und bei erlaubter Bewegung. Jede Eisfläche mit Material 49 rutscht, auch manuell
gebautes Eis ohne Herkunftsmarker.

Die Traktion gilt bisher für den gemeinsamen Spieler-Character-Motor. Sie
führt keine allgemeine Eisphysik für NPC-Navigation, Fahrzeuge oder beliebige
Avian-Körper ein. Bereits gespeicherte unmarkierte Eisblöcke bleiben bestehen
und werden nicht rückwirkend als Natureis interpretiert. Es wird nur eine
Oberflächenkruste simuliert, kein metertiefes Durchfrieren und kein Aufbrechen
durch Gewicht. Gefrier-/Taufortschritt ist flüchtig und beginnt nach einem
Neustart erneut; fertiges Natureis samt Wassermenge und sichtbare Grasstufen
werden gespeichert. Ungeladene Regionen werden nicht offline simuliert.

## Prüfumfang

Die ergänzten Regressionen decken regionale Grasraten, Winterpause und
Sommerfortsetzung, Tierbedürfnisse und Bienenruhe, Eisbudgets und Autorität,
Quellmengen nach Tau-/Abbauvorgängen, echte Region-Dateien und Section-Deltas
sowie den gemeinsamen Motor einschließlich realer Avian-Kapselbewegung ab.
Ihre Ausführung wird in der gemeinsamen Projektvalidierung unter
`tools/benchmarks/results/reference-biomes-f14-2026-11-14/` festgehalten.
Dieses Dokument erklärt den implementierten Ablauf und behauptet keine noch
nicht abgeschlossenen Cargo- oder nativen Prüfergebnisse.
