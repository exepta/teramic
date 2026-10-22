# Oefen, Place-Essen Und Regen: Phase 87

Stand: 2026-10-20. Dieser Bericht beschreibt die Implementierung und ihre
aktuellen Grenzen. Formatpruefung, Cargo Check, Clippy, Client-Build, 1406 Tests,
Headless-Server sowie die nativen Ofen-, Wetter- und Essens-Fixtures sind
erfolgreich abgeschlossen. Ein bestehender UDP-Langzeittest ist explizit
ignoriert. Nach einer abschliessenden rein stilistischen Fixture-Korrektur
sind Build, Formatpruefung, Cargo Check, Clippy und alle Tests erneut
erfolgreich durchgelaufen.

## Bedienung Und Regen

- Essen folgt `GameplayInput::consume_pressed()`: der gespeicherten Aktion
  **Place Block / Eat** oder der bisherigen **Use**-Aktion. Defaults bleiben
  `B` und `R`; auch umgebundene Mausbuttons und Tastenkombinationen funktionieren
  ueber dieselbe Eingabeauswertung.
- Extra Shift mit Place erlaubt weiterhin das Pflanzen essbarer Pilze.
  Ist Shift bereits Bestandteil der Place-Belegung, blockiert es Essen nicht.
  Stationsinteraktion und gezieltes Fuettern haben bei Place Vorrang.
- Essen sendet die bestehenden Halteimpulse an den Server. Verbrauch und Werte
  bleiben autoritativ; Loslassen, Slotwechsel, Menue und Schaden brechen ab.
  Ego-/Avataranimation und Kau-Audio verwenden dieselbe Eingabeentscheidung.
- Die Niederschlagsintensitaet verwendete zuvor denselben Zufallswert wie die
  vertikale Fallphase. Besonders leichter Regen konnte deshalb als dichtes
  Hoehenband mit anschliessender Luecke erscheinen. Ein unabhaengiger Hash in
  der ungenutzten Normalenkomponente des unbeleuchteten Niederschlagsmeshes
  trennt jetzt Dichteauswahl und Fallphase. Auch die Auswahl einer angebrochenen
  Schneeschicht nutzt diesen Wert. Keine zusaetzlichen Partikel oder Meshes.

## Bloecke Und Rezepte

Der Ofen ist ein zweiblockhohes Objekt mit Primaerzelle unten und zugehoeriger
Oberzelle. Platzierung verlangt beide geladenen freien Zellen und tragenden
Boden. Beide Teile oeffnen dasselbe Inventar. Abbau eines Teils entfernt das
Paar und verwendet die bestehenden Inventar-/Drop-Pruefungen.

Die IDs werden nach Pink Salt Stone 58 angehaengt:

| ID | Objekt | Logische Rohranschluesse |
| --- | --- | --- |
| 59 | Furnace, beide Objektteile | Abzug an Primaerposition + Y2 |
| 60 | Chimney Straight | Unten / oben |
| 61 | Chimney Elbow | Unten / vorne |
| 62 | Chimney Horizontal | Hinten / vorne |
| 63 | Chimney Bend Up | Unten / vorne |
| 64 | Chimney Bend Down | Oben / vorne |

Facing 0 zeigt nach +Z, 1 nach +X, 2 nach -Z und 3 nach -X. Rohrverbindungen
muessen an beiden Zellen passende Anschluesse besitzen. Varianten 61 und 63
unterscheiden sich visuell, haben aber dieselben logischen Ports.
Modelle und Ofenoberflaeche sind referenzinspiriert, keine pixelgenaue Kopie
der bereitgestellten Bilder.

`assets/crafting/` enthaelt je ein Rezept fuer Ofen, Rohre und Bellows.
Der Ofen benoetigt acht Steine im 3x3-Ring an der Werkbank.
Bellows bestehen aus einem Leder, zwei Holz und einem Stein beziehungsweise
Eisenbarren, Goldbarren oder Ruby:

| Bellows | Verarbeitungstempo | Doppelter Rezeptausgang |
| --- | --- | --- |
| Stone | +10 % | Nein |
| Iron | +20 % | Nein |
| Gold | +10 % | 5 % |
| Ruby | +15 % | 7,5 % |

Die Chance ist deterministisch aus Welt-/Positionsseed und gespeichertem
Abschlusszaehler abgeleitet. Speichern und Neuladen rollen einen Abschluss
nicht erneut. Es handelt sich um eine Chance je Abschluss, nicht um eine
garantierte Anzahl doppelter Ausgaben in kleinen Stichproben.

Jede Schmelzregel steht in einer eigenen Datei unter `assets/smelting/`;
Brennstoffe und ihre Brennsekunden stehen einzeln unter `assets/fuels/`.
Der vorhandene Asset-Override wird mit eingebetteten Fallbacks unterstuetzt.
Enthalten sind Rind-, Schweine-, Schaf- und Fischgerichte, Clownfisch, Eisen,
Gold und Smooth Stone; Brennstoffe sind Kohle, Stock sowie die vorhandenen
drei Holz- und Bretterarten. Die bisherigen Handwerksrezepte zum Raffinieren
von Eisen und Gold wurden durch Ofenschmelzen ersetzt.

## Inventar Und Autoritaet

Die vier Plaetze sind Eingang, Brennstoff, Ausgabe und Bellows. Der Server
prueft Rezeptzutaten und Brennstoffe, erlaubt aus dem Ausgabefach nur Entnahme
und begrenzt den Bellows-Platz auf ein passendes Exemplar. Die Filter gelten
auch fuer Ziehen, Mausrad und Schnellverschieben. Ein voller oder inkompatibler
Ausgang startet keinen neuen Verbrauch von Zutaten oder Brennstoff;
bereits entzuendeter Brennstoff laeuft weiter ab.

Too Many Items zeigt Schmelzrezepte mit Ofenstation, Eingang, Ergebnis und
Laufzeit. Bei geoeffnetem Ofen kann das Rezept Zutaten autoritativ in dessen
Eingang laden. Dies erzeugt kein sofortiges Schmelzergebnis. Der normale
Handwerksbereich wird waehrend des Ofenfensters nicht als Ersatzofen benutzt.

`terra-core::furnace` berechnet Rezepte, Brennzeit, Fortschritt und Bellows.
`terra-network::server_furnaces` verarbeitet dieselben Zustaende fuer Integrated
und Dedicated. Containerinhalte laufen ueber die bestehenden privaten
Container-Snapshots; Brennzustand und begrenzte Rauchdarstellung werden separat
repliziert. Fortschrittsaenderungen loesen keinen Terrain-Mesh-Neubau aus.

## Verarbeitung Und Rauchgrenzen

- Verarbeitung etwa alle 0,1 Sekunden; maximal 64 aktive Oefen je Durchlauf
  in sortierter Round-Robin-Reihenfolge. Verstrichene aktive Zeit wird pro Ofen
  weitergetragen, mit maximal acht Sekunden Nacharbeit pro Ofen und Durchlauf.
- Ohne synchronisierten Spieler oder ohne geladenes, gueltiges Ofenpaar
  pausiert die Verarbeitung. Entladene und offline vergangene Zeit wird nicht
  beim Wiederladen als Brennzeit nachgeholt.
- Rauch wird in ganzen Sekundenschritten weitergeschaltet, mit gespeichertem
  Sekundenrest. Ein brennender Ofen fuellt einen als geschlossen erkannten
  Raum um eine eindeutige Zelle pro Sekunde, maximal 256 Zellen pro Ofen.
  Hoehere Zellen haben Vorrang, danach die Entfernung zum Austritt.
- Rohre beginnen an Primaerposition + Y2. Maximal 64 Rohrzellen werden verfolgt.
  Zyklen, falsche Anschluesse oder feste Hindernisse bewirken Rueckstau zur
  Vorderseite auf Y1; auch dort muss eine durchlaessige Zelle vorhanden sein.
- Die Raumpruefung besucht maximal 4096 verbundene durchlaessige Zellen.
  Eine freie Saeule aus 32 geladenen Zellen darueber gilt als Aussenluft.
  Wiederholte Zellabfragen werden innerhalb der Pruefung zwischengespeichert.
  Wasser und feste Bloecke sind nicht durchlaessig.
- Unbekannte Sections, ungeklaerte Abzuege und ueberschrittene Raumsuchbudgets
  pausieren weiteres Fuellen. Sie werden nicht als leere Welt interpretiert
  und erzeugen keine Chunks. Sehr grosse oder teilweise entladene Innenraeume
  koennen deshalb ohne weitere Rauchausbreitung bleiben.
- Beim Lueften oder Erloeschen verschwinden zwei Rauchzellen pro Sekunde.
  Geaenderte Waende und erreichbare Zellen werden in der naechsten
  Rauchpruefung neu bewertet.
- Schnitt des Koerpervolumens mit mindestens einer Rauchzelle verursacht
  zwei Schaden pro Sekunde. Ueberlappende Oefen vervielfachen den Schaden nicht.
  Survival-Spieler verwenden die vorhandenen Schadens-, Essabbruch- und
  Todespfade; Creative, Spectator und bestehender Respawn-Schutz bleiben immun.
  Lebende geladene Mobs verwenden Gesundheit, Todeszustand und vorhandene
  Drop-Grenzen, keine separate Rauch-Mob-Simulation.

Dies ist eine begrenzte Blockzustandsdarstellung geschlossener Raeume, keine
Fluid-/CFD-Simulation. Die 32-Zellen-Himmelspruefung ist eine Heuristik; ein
entsprechend hoher Innenraum kann als belueftet gelten. Rauchmengen je Ofen
sind logisch vollstaendig gespeichert, auch wenn die Darstellung weniger zeigt.

## Darstellung Und Audio

Pro Spieler werden hoechstens 64 Ofen-Datensaetze ausgewaehlt. Ihre Prioritaet
ist das Minimum aus Ofenankerabstand und Abstand zur naechsten Rauchzelle;
dieses Minimum muss innerhalb von 192 Metern liegen. Ein entfernter geladener
Ofen kann deshalb seinen nahe am Spieler liegenden Rauch weiterhin uebertragen.
Aus den hoechstens 16.384 Zellkandidaten dieser Quellen werden global die
naechsten 256 eindeutigen Rauchzellen innerhalb von 192 Metern ausgewaehlt.
Ueberlappungen verbrauchen nur einen Platz und werden einer Quelle zugeordnet.
Ein naher Ofen kann das Budget somit nicht mit entfernteren Zellen belegen,
waehrend naehere Zellen einer anderen ausgewaehlten Quelle fehlen.
Der Kanal sendet maximal viermal pro Sekunde und unterdrueckt unveraenderte
Snapshots. Diese Grenzen betreffen Darstellung und Bandbreite, nicht die
vollstaendige, nicht stapelnde Rauchschadenspruefung.

Der Renderer verwendet zwei wiederverwendete Meshes fuer Feuer und Rauch,
maximal 32 nahe Feuer innerhalb von 96 Metern und global die naechsten
256 eindeutigen Rauchzellen innerhalb von 64 Metern. Die Rauchauswahl ist
unabhaengig von Ankerentfernung, geladenem Anker, Brennzustand und Feuerbudget;
die einzelne Rauchzelle muss aber geladen sein. Renderer und Simulation
verwenden dasselbe `smoke_passable`-Praedikat: Luft und nichtfeste Zellen wie
Fackeln oder Pflanzen koennen Rauch zeigen, feste Bloecke und Wasser nicht.
Leere Geometrie wird ausgeblendet, nicht als leerer Vertex-/Indexpuffer
hochgeladen. Maximal acht schattenlose Feuerlichter innerhalb von 28 Metern
besitzen je acht Meter Reichweite. Es gibt nicht pro Rauchblock eine neue
Entity oder Lichtquelle.

Brennende Oefen werden den bestehenden `TorchSources` hinzugefuegt und nutzen
den vorhandenen leisen Fire-Loop. Es wurden keine neuen Feueraufnahmen
hinzugefuegt. Die breite und kompakte Inventaransicht sowie die Weltaufnahme
wurden visuell geprueft: Header-Ebenen und der Status `Heating` sind korrekt.
Diese Pruefung bedeutet keine pixelgenaue Uebereinstimmung mit der Referenz.

## Speicherung Und Kompatibilitaet

- Netzwerkprotokoll 50: Client und Server muessen gemeinsam aktualisiert werden.
- Neue Regionen verwenden den tatsaechlichen vier Byte langen Header `RGA0`
  und enthalten Ofenfortschritt, Restbrennzeit, Abschlusszaehler, Abzug und Rauch.
  Inventare bleiben im bestehenden Containerdatensatz.
- `REG9` und die vorhandenen aelteren Decoder bleiben erhalten. `PLY4`,
  Generatorversion, Welt-IDs und bestehende Block-/Item-IDs werden nicht
  umnummeriert. Die neuen Block-IDs 59 bis 64 sind eine Erweiterung.
- Keine neuen Abhaengigkeiten, keine zweite Terrain-, Inventar- oder
  Speichersimulation.

## Bestaetigte Validierung

| Pruefung | Status und Nachweis |
| --- | --- |
| Abschliessende Formatpruefung | Erfolgreich; [fmt-accepted-final.log](fmt-accepted-final.log) ist beim sauberen Stand leer |
| Cargo Check | Erfolgreich, 1,88 s; [check-accepted-final.log](check-accepted-final.log) |
| Client-Build | Erfolgreich, 29,63 s; [build-accepted-final.log](build-accepted-final.log) |
| Vollstaendiger Cargo-Testlauf | Erneut 1406 bestanden, 0 fehlgeschlagen, 1 bestehender UDP-Soak ignoriert; [tests-accepted-final.log](tests-accepted-final.log) |
| Clippy | Abschliessender Nachlauf erfolgreich, 13,75 s; [clippy-accepted-final.log](clippy-accepted-final.log) |
| Headless-Server | Erfolgreich initialisiert und 27 Sections verarbeitet; [headless-final.log](headless-final.log) |
| Native Ofen-Referenzfixture | Erfolgreich, Ofen, fuenf Rohrformen und vier Bellows, vier Aufnahmen; [native-furnace-art-final.log](native-furnace-art-final.log) |
| Finale native Ofen-Gameplay-Fixture | Erfolgreich mit frischer Fixturewelt `native-6`, sechs Aufnahmen; [native-furnace-gameplay-final-pass.log](native-furnace-gameplay-final-pass.log) |
| Native Wetter-Fixture | Erfolgreich bis Phase 54; [native-weather-final.log](native-weather-final.log) |
| Native Essens-Fixture | Erfolgreich, vier Aufnahmen; [native-food.log](native-food.log) |

Die Tests umfassen umgebundene Place-Eingaben, Essabbruch und
Interaktionsvorrang, Regendichte ueber Fallphasen, Schmelzen und Filter,
Bellows, Rohrrotationen, Raum-/Unbekannt-Grenzen, Rauchschaden, faire
Verarbeitung, Netzwerklimits und Speicherung. Geprueft werden auch kurze
gueltige Rezepte/Brennstoffe mit unterschiedlichen Zeitschritten sowie die
global naechste, eindeutige Rauchauswahl bei entfernten Ofenankern.
Der ignorierte Test `sustained_two_profile_udp_session_keeps_identity_and_replication`
ist der bereits vorhandene 180-Sekunden-UDP-Soak und wurde in diesem Lauf
nicht ausgefuehrt.

Die finale native Ofen-Fixture prueft serverseitige Zweiblock-Platzierung,
Oeffnen mit umgebundenem Place bei gehaltenem Lebensmittel ohne Essbeginn,
Too-Many-Items-Rezept samt Zutatenuebernahme, echte Inventar-Gesten,
laufenden Fortschritt, Entnahme des fertigen Ergebnisses und
Speichern/Neuladen. Die sechs Aufnahmen sind
[Platzierung](captures/furnace-placed.png),
[Schmelzrezept](captures/furnace-smelting-recipe.png),
[Breite Ansicht](captures/furnace-running-wide.png),
[Kompakte Ansicht](captures/furnace-running-compact.png),
[Ergebnis](captures/furnace-result.png) und
[Brennender Ofen in der Welt](captures/furnace-burning-world.png).
Die separate Referenzfixture dokumentiert die Modelle unter
[Ansicht 0](captures/furnaces-0.png), [Ansicht 1](captures/furnaces-1.png),
[Ansicht 2](captures/furnaces-2.png) und [Ansicht 3](captures/furnaces-3.png);
sie ersetzt nicht die autoritative Gameplay-Pruefung.
Nach diesem erfolgreichen nativen Lauf wurden ausschliesslich zwei
gleichwertige Fixture-Bedingungen in Match-Guards verschoben, um
`clippy::collapsible_match` zu erfuellen; kein Gameplay-Verhalten wurde dabei
geaendert. Clippy und der vollstaendige Testlauf wurden danach erneut ausgefuehrt.

Der Wetterlauf bestaetigt Regen, Schneequalitaeten, Wolkenstufen, Tag/Nacht,
Gewitter, Regen-/Wasserausschluss und die bestehenden Wettereffekte bis zur
abschliessenden Erfolgsmeldung. Die kurzen protokollierten FPS-Fenster sind
kein Vorher-/Nachher-Paritaetsbenchmark. Beispielaufnahmen:
[Regen](captures/rain.png), [Regen Low](captures/rain-low.png) und
[Regen bei Nacht](captures/rain-night.png).

Der native Essenslauf prueft umgebundene Place-Taste und rechte Maustaste,
ausbleibenden vorzeitigen Verbrauch, Loslass-/Menueabbruch, genau ein
verbrauchtes Item, Regeneration und Speichern/Neuladen. Die Erfolgsmeldung
weist vier Aufnahmen aus:
[Ruhe](captures/food-idle.png), [Ego-Bissen](captures/food-first-person-bite.png),
[Third-Person-Bissen](captures/food-third-person-bite.png) und
[Menueabbruch](captures/food-menu-cancelled.png).

## Fruehere Fixture-Befunde

Fruehe Laeufe werden nicht nachtraeglich als bestanden gewertet:

- [Erster Gameplay-Lauf](native-furnace-gameplay.log): Fixture griff vor der
  Plugin-Initialisierung auf `ControlsConfig` zu. Die Initialisierung wartet
  jetzt auf vorhandene Controls-/Menu-Ressourcen.
- [Frueher Stage-0-Lauf](native-furnace-gameplay-final.log): Timeout ohne
  abgeschlossene Fixture. Explizite Bereitschaftspruefungen und Diagnostik
  warten nun auf Sitzung, Bodenstand und die benoetigten Item-Vorschaubilder.
- [Zwischenlauf](native-furnace-gameplay-verified.log): blieb beim
  Schnellverschieben in Stage 8 stehen. Shift bleibt in der korrigierten
  Eingabesequenz bis zur bestaetigten Brennstoff-/Bellows-Uebernahme gehalten;
  ein unerwarteter Cursorstapel wird als Fehler erkannt.
- [Spaeterer Zwischenlauf](native-furnace-gameplay-complete.log): brach wegen
  einer zu fruehen Zielpruefung in Stage 3 ab. Die Fixture richtet erneut aus
  und wartet begrenzt auf das echte Ofenziel, bevor sie Place ausloest.
- Die abschliessende UI-Pruefung korrigierte Header-Layering und den
  Heizstatus. Die aktuellen breiten, kompakten und Weltaufnahmen wurden
  danach erneut aufgenommen und visuell geprueft.

Der oben verlinkte Lauf `native-furnace-gameplay-final-pass.log` ist der
massgebliche Gameplay-Erfolgsnachweis. `TERRA_FURNACE_GAMEPLAY_DIR` bezeichnet
eine eigene frische Testwelt, `TERRA_CAPTURE_DIR` das Aufnahmeverzeichnis.
Die nativen Laeufe belegen funktionierende Ablaeufe und Darstellung, keine
Performance-Paritaet oder garantierte FPS auf anderen Systemen.
