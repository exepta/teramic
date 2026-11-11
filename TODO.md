# TERAMIC: Offene Aufgaben

Stand: 11.11.2026. Nur offene Features, Korrekturen und Abnahmen.
Erledigte Aufgaben werden entfernt; die Umsetzungshistorie steht in
[PATCH.md](PATCH.md). Bestehende IDs bleiben erhalten, neue IDs werden fortlaufend angehaengt.
`F` = Feature, `O` = Optimierung/Fehlerkorrektur, `A` = Abnahme.

Die weiteren Schwerpunkte bauen auf den vorhandenen Kingdom-Rechten, Baeumen
und Setzlingen auf: produktive NPC-Arbeit sowie Dampfantrieb und
Fahrzeugautomatisierung. Vorhandene Registries, Navigation, Inventare,
Wassertechnik sowie Speicher- und Netzwerkpfade werden erweitert.
Neue Systeme brauchen passende Save/Reload-, Mehrspieler- und Besitzpruefungen;
Wachstum, NPC-Arbeit und bewegte Fahrzeuge muessen begrenzt simuliert werden.
F58/F61 sind umgesetzt; Bedienung und Grenzen stehen in [docs/npc-work.md](docs/npc-work.md).
F72 mit Deutsch/Englisch steht in [docs/localization.md](docs/localization.md).

## 1. NPCs, Siedlungen Und Waldwirtschaft

- [ ] **F78 Neutrale Nomaden:** Gelegentlich kleine NPC-Nomadengruppen in geladenen neutralen Gebieten erscheinen und umherziehen lassen. Kingdom-/Dorfgrenzen, erreichbare Wege, Tagesrhythmus und begrenzte Spawn-/Simulationsbudgets beachten; kein Erscheinen in Spielerbauten oder beanspruchten Gebieten.

- [ ] **F79 Produktive Doerfer Und Bot-Doerfer:** Dorfaufbau um Berufsstationen, zugewiesene Arbeitsgebiete und echte Vorratstruhen erweitern. Holzfaeller und weitere unterstuetzte Arbeiter sollen ihre Arbeitsplaetze erreichen, Werkzeuge/Vorraete beziehen und Ertraege anliefern. Dorf-/Bot-Besitz und Rechte, begrenzte Simulation sowie Save/Reload erhalten.

- [ ] **F80 Kingdom-Startbevoelkerung:** Bei erfolgreicher Kingdom-Gruendung genau einmal drei Schwertkaempfer, zwei Bogenschuetzen, drei Lanzentraeger und zehn zuweisbare Arbeitslose an sicheren erreichbaren Stellen erscheinen lassen. Alle NPCs dem Gruender/Kingdom zuordnen; atomare Gruendung, Spawn-Limits, Speicherstand und Schutz vor erneuter Vergabe beim Laden beachten.

- [ ] **F49 Verbleibende Mob-/Villager-Navigation:** Reproduzierbare Restfaelle bei Hindernissen, Tueren, Hoehenwechseln, Ausweichbewegungen und festgefahrenen Zielen beheben. Bestehende begrenzte Navigation um verlaessliche Ankunft, Zielwechsel und Abbruchgruende erweitern. Berufs-, Pflanz- und Transportziele anbinden; sichtbare Aktionen an tatsaechlichen Fortschritt koppeln.

## 2. Zuege, Strassen Und Pferdekutschen

### Fahrbetrieb Und Automatisierung



- [ ] **F65 NPC-Lokfuehrer:** Einen zuweisbaren Zugfuehrer-Beruf ergaenzen. Fuehrerstand erreichen, zugewiesenen Zug fahren, Weichen/Signale und Haltepunkte beachten sowie bei Hindernissen sicher bremsen. Wasser-/Brennstoffmangel, blockierte Strecke und fehlende Berechtigung anzeigen; Dienstzeit, Spieleruebernahme und Wiederaufnahme nach Laden regeln.

- [ ] **F66 Bahnhoefe, Fahrplaene, Signale Und Fracht:** Benannte Stationen/Haltepunkte mit Fahrplaenen, Wartezeiten und Zielwahl ergaenzen. Blockabschnitte/Signale fuer mehrere Zuege vorsehen. Fahrgastwechsel und begrenztes Be-/Entladen zwischen Wagen und Lagern ermoeglichen; Mengen, Filter und volle Zielinventare ueber vorhandene Inventar-/Logistiktransaktionen behandeln.

### Strassen Und Pferdekutschen

- [ ] **F68 Teer-/Asphalt-Strassensystem:** Teer beziehungsweise Bitumen als Bindemittel mit Rezeptkette ergaenzen; Zuschlagmaterial und Bindemittel zu Asphalt verarbeiten. Items, Materialien, Rezepte und TMI-Anzeigen registrieren. Asphaltbloecke, duenne Strassendecken, Steigungen/Uebergaenge und Randsteine bereitstellen; Texturanschluesse, Kollisionen, Abbau und Materialrueckgabe definieren.

- [ ] **F69 Gemeinsame Wege Und Transportrouten:** Strassen, Arbeitsplaetze, Lager, Bahnhoefe und Kutschenhalte an die bestehende Navigation anbinden. NPCs/Pferde sollen geeignete Wege bevorzugen. Fahrzeugbreite, Steigung, Durchfahrtshoehe und Claim-Rechte beachten; Umleitungen und unpassierbare Verbindungen anzeigen. Wiederverwendbare Suchergebnisse und begrenzte Aktualisierungen fuer viele Auftraege nutzen.

- [ ] **F67 Pferdekutschen:** Kutsche mit Raedern, Deichsel, Geschirr und An-/Abkuppeln an vorhandene gezaehmte Pferde ergaenzen. Fahrersitz, Mitfahrer und echtes Ladungsinventar unterstuetzen. Lenkung, Rueckwaertsfahrt, Bremsen, Gespannkollisionen, Steigungen und Pferdeausdauer einbeziehen. Manuelle Fahrt und NPC-Routen an F69 anbinden; sichere Halte-/Anbindestellen sowie konsistente Speicherung von Gespann, Sitzen und Ladung vorsehen.

## 3. Spieler, Inventar Und Handwerk

- [x] **F77 Charaktermodelle Und Kleidermaterialien:** Getrennte Auswahl von Leinen, Segeltuch, Leder und Wolle fuer Oberteil, Hose, Stiefel und Umhang, mit tatsaechlicher Gewebe-/Oberflaechendarstellung und lokalen sowie Serverprofilen. Umhang, Stiefeldetails und Spieler-/NPC-Gesichter an den Referenzen ueberarbeitet; 12 maennliche/14 weibliche Frisuren mit Strähnen, getrennte Augenfarben, korrigierte Haende und detailliertere Koerper, vier Editor-Tabs und drehbare Inventarfigur; alte Profile bleiben lesbar. Die menschliche gestalterische Abnahme ist unter A01 gesammelt; die Modelle sind nachmodelliert, keine importierten Original-3D-Assets. Siehe `docs/characters.md`.

- [x] **F44 Inventar-Sortierbuttons:** Rucksack, Truhen/Doppeltruhen, verbundene Werkbank-Lager und Zuglager per Trichter-Dropup nach A-Z, ID oder Menge sortieren und gleiche Stapel zusammenfassen. Hotbar, reservierte Crafting-Slots und funktionale Maschinenplaetze bleiben erhalten. Serverautoritative atomare Transaktionen mit Revisionen, Rechten und leerem Cursor; auch Rucksacksortierung im Zugdialog. Siehe `docs/inventory-sorting.md`.

- [ ] **F48 Tragbare Taschen Und Rucksaecke:** Taschen/Rucksaecke mit eigenem gespeichertem Inventar und passender Ausruestungs-/Taschenansicht ergaenzen. Slotbesitz, Zugriff, Verlust/Drop, Mehrspielertransaktionen und begrenzte Verschachtelung definieren; Inhalte beim Tragen, Ablegen und Zerstoeren erhalten.

- [ ] **F45 TMI-Prozessketten Wie Bei Create:** Vorhandene Stationsrezepte um zusammenhaengende Verarbeitungsketten erweitern. Benoetigte Antriebe, Fluessigkeiten, Transportverbindungen und Zwischenprodukte aus echten Rezept-/Maschinendefinitionen zeigen; zwischen verknuepften Schritten navigieren koennen.

- [ ] **F47 Chemie Und Experimentieren:** Experimentelle Mischungen im vorhandenen Trank-/Kesselsystem ergaenzen. Zutaten, Mengen, Wasser, Erhitzen und Werkzeuge beeinflussen Ergebnisse; Kombinationen, Fehlschlaege und Entdeckungen speichern und erklaeren. Bestehende Effekt-/Rezeptregistries verwenden.

- [ ] **F32 Kaelte Und Hitze:** Umgebungstemperatur aus Tageszeit, Jahreszeit, Biom, Hoehe, Wasser/Nassheit und Schutz ermitteln, einschliesslich kalter Wuestennaechte/heisser Sommer. Frieren, Resistenz, Kleidung und Lagerfeuerwaerme einbeziehen; Hitze als eigenen Status ergaenzen. Wirkung langsam auf-/abbauen; Werte, Schutz und HP-/Stamina-Rueckmeldung autoritativ speichern/replizieren.

## 4. Weitere Natur-, Welt- Und Darstellungsaufgaben

### Natur Und Kreaturen

- [ ] **F14 Verbleibendes Klimaabhaengiges Wachstum:** Vorhandene Dirt->Grass-Umwandlung und Tierverhalten an regionale Feuchtigkeit/Jahreszeiten anbinden. Kontrolliertes Gefrieren/Schmelzen von Eisblock 49 und dessen Rutschphysik ergaenzen; Spielerbauten und Wasserquellen erhalten.

- [ ] **F35 Spirit Forest:** Wasserfallreiches Biom mit gruen-tuerkisem Laub, blaulich schimmernden leicht transparenten Spirit Mushrooms, seltenem Spirit Crystal und violetten Haengeranken vervollstaendigen. Lokale Nebelstellen aus erdaehnlichen Nebelbloecken unter Gras mit pulsierenden blauen Steinen ergaenzen; vorhandene Darstellungsbudgets verwenden.

- [ ] **F37 Schmetterlinge:** Freundliche Tiere mit sieben Varianten in Cherry Forest, Forest und Plains ergaenzen. An bestehende Spawn-/Tier-Systeme anbinden und fuer Spinnen als echte Beute verwenden.

- [ ] **F38 Spirit Gloom:** Zwei freundliche fliegende Varianten ausschliesslich im Spirit Forest ergaenzen. In der Naehe Regeneration I ueber das bestehende Effektsystem mit begrenzter Reichweite, Updatefrequenz und Aura-Stapelung vergeben.

- [ ] **F39 Skelettkrieger:** Nachts im Spirit Forest ungeruestete Schwertkaempfer, Kaempfer mit Eisenruestung/Eisenschwert und unbewaffnete Varianten ergaenzen. Bei Tagesanbruch sichtbar eingraben und danach despawnen; bestehende KI, Navigation, Ausruestung und Spawn-Budgets verwenden.

- [ ] **F70 Weitere Unterwasserpflanzen:** Zusaetzliche Wand-/Deckenpflanzen fuer geeignete Unterwasserbereiche ergaenzen. Bestehende Prop-/Habitatregeln fuer Halt, Wasser, Sichtbarkeit, Entnahme und Darstellung erweitern.

- [ ] **F71 Hoehlenoekologie Und Unterirdische Wasserlaeufe:** Hoehlenbegruenung mit Wachstumszyklen sowie zusammenhaengende unterirdische Wasserlaeufe ueber einzelne lokale Reservoirs hinaus ergaenzen. Vorhandene Kammern, Feuchtigkeit, Hoehenprofile und Fluidpersistenz verbinden; Generierungsaenderungen versionieren.

### Welt Und Darstellung

- [ ] **F36 Natuerliche Hoehlenraeume:** Bestehende Kammer-/Connector-Engine um unregelmaessige, geologisch lesbare Raeume, Terrassen und wechselnde Querschnitte erweitern. Natuerliche Uebergaenge statt auffaelliger Kugelraeume erzeugen; historische Welten durch versionierte Generierung erhalten.

- [ ] **F15 Schaeden An Weltobjekten:** Brand-/Lebenspunktschaeden fuer geeignete Objekte ergaenzen. Treffer, Feuer, Zustand, Zerstoerung und Drops an vorhandene autoritative Welt-/Inventarpfade anbinden.

- [ ] **F24 Entfernte 3D-Karte:** Budgetierte entfernte 3D-Rekonstruktion mit getrennten Boden-/Kronenhoehen oder echten Mesh-LODs ergaenzen. Vorhandene gespeicherte Kartendaten nutzen; Hoehlenkarten bleiben flach.

- [ ] **F20 Verbleibende Rendering-Erweiterungen:** Budgetierte Offscreen-Reflexionen, RGB-/Skylight-Ausbreitung und indirekte Beleuchtung ergaenzen. Neue Lava/leuchtende Pflanzen an Emissionsmetadaten anbinden; Lava-Gameplay gesondert definieren. Statischen Custom-Mesh-Import bei benoetigten Spezialassets um Animation/Skinning erweitern.

- [ ] **F21 Fehlende Tierlaute Und Drops:** Artspezifische Baeren-/Pferdelaute, weitere Kau-/Schuettel-/Umgebungsaufnahmen und passende Drops ergaenzen. Lautstaerke, Entfernung und Wiederholungen in Audiobudgets integrieren; Hoerabnahme ueber A02 durchfuehren.

- [ ] **F22 Ressourcen-Stapelphysik:** Gegenseitige Kollision und Stapelbildung loser Items bei tatsaechlichem Gameplaybedarf ergaenzen; bewegte Drops und Einsammeln weiterhin budgetieren.

## 5. Multiplayer, Befehle Und Erweiterbarkeit

- [ ] **F50 Heal-Befehl:** Autoritativen `/heal`-Befehl fuer den eigenen sowie explizit benannte andere Spieler ergaenzen. Berechtigungen, Zielspieler, maximale HP, Rueckmeldungen und Replikation pruefen; bestehende Effekte und weitere Beduerfnisse erhalten.

- [ ] **F51 Give-Befehl Mit Optionaler Farbe:** Autoritativen `/give`-Befehl fuer Itemvergabe mit Mengen-/Zielspielerangabe und optionalen RGB-Werten ergaenzen. Item-Metadaten, Rechtepruefung, Inventartransaktionen und Save-/Netzwerkvalidierung verwenden; Farbangaben fuer unfaerbbare Items klar ablehnen.

- [ ] **F16 Weitere Mod-Inhalte Und Inhaltsabstimmung:** Neue Schmelz-, Brennstoff- und Nahrungsdefinitionen ueber bestehende Inhaltsabstimmung replizieren. Validierte JSON-Kataloge und spaeter Live-Reload ergaenzen; vorhandene Item-/Rezeptregistries erweitern.

- [ ] **F17 Verbleibende Multiplayer-Aktionen:** Allgemeine autoritative Schlagereignisse fuer Luft/Mobs und expliziten Schwimmantrieb replizieren. Remote-Bewegung aus passenden Eingaben darstellen; Husten-/Tierlaute als budgetierte gemeinsame Audioereignisse ergaenzen.

- [ ] **F18 Spectator-Vervollstaendigung:** Spieler-Kamerabesitz sowie vollstaendige Abkopplung von Populationsfokus, Wassereffekten und fremden Platzierungskoerpern ergaenzen. Vorhandenen kollisionslosen Flug und gesperrte Weltinteraktionen erhalten.

- [ ] **F19 Dimensionen Und Gemeinsame Karten:** Identitaeten und Speicherpfade festlegen; Spieler, Container, Karten und Weltzustand pro Dimension trennen. Gemeinsame/serverseitige Wegpunkte darauf aufbauen.

- [ ] **F23 Rollen Und Online-Accounts:** Chat-Moderation, Serverrollen und verifizierten Login-/Sessiondienst mit API, Vertrauensschluesseln und Gateway-Pruefung ergaenzen. Lokale Profile und verifizierte Accounts unterscheiden; oeffentlichen Zugang an einen Verifier binden.

## 6. Offene Abnahmen

- [ ] **A01 Grafik-Abnahme:** Blockformen/-oberflaechen, Kronen, Gras, Blumen, Pilze, Mineralien, Hoehlen, Ufer, Korallen, Anemonen und Tiere gemeinsam an nativen Referenzansichten menschlich beurteilen. Avatar-Gesichter, Griffpunkte, Zug-Montageansicht/Inventare und menschliche Animationen einschliessen.

- [ ] **A02 Audio-Abnahme:** Tier-/Schritt-/Ess-/Ofen-/Zug-/Oeffnungs-/Husten-/Wasser-/Wetterklang mit Kopfhoerern und Lautsprechern menschlich pruefen. Dekodiertests durch tatsaechliche Hoerabnahme ergaenzen.

- [ ] **A03 Weitere Plattformen:** Serverkonsole unter Windows/macOS, PowerShell/Legacy-Terminals, Signalabbruch und Pipe-/Zeileneingabe nativ pruefen.

- [ ] **A04 Mehrere Rechner:** Reale LAN-/Internet-Sitzungen mit zweitem Rechner, Firewall/Router, Reconnect und hoher Latenz pruefen. Zwei weit getrennte Streaming-Foci mit getrenntem Verlassen und Fluid-Pause/Reload-Wakeup als kombinierten Lauf abnehmen.

- [ ] **A05 Weitere GPUs Und Langzeitlast:** Speicher-/Framezeit-/VRAM-Verlauf bei dichten 32er-Streaming-, Wasser-, Wetter-, Tier- und Edit-Laeufen auf weiterer Hardware pruefen. Windlaub-/Blend-Overdraw sowie Near-/Middle-Pilzpark- und Riffkosten mit identischer Geometrie messen. Viele Avatare, Herden und Itemdrops einschliessen. Keine neuen 10-/15-/30-Minuten- oder mehrstuendigen Laeufe ohne ausdrueckliche Freigabe.

- [ ] **A06 Weltgenerierungs-Abnahme:** Viele neue Seeds, negative Koordinaten, extreme Kammern/Regionsecken, Seen/Fluesse, Schluchten und Oasenraender pruefen. Y-Grenzen, Grundstein, Erzhoehen und Salzbiom-Regeln erhalten; historische Welten von neuer Generierung trennen.

- [ ] **A07 Kessel-Dropdown Nativ:** Zwoelfteiliges Rezept-Dropdown tatsaechlich oeffnen, zum siebten Eintrag scrollen und ihn anklicken. Sichtbarkeit im geclippten Popup und autoritativ gewaehltes Giftrezept pruefen.
