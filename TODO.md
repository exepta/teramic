# TERAMIC: Offene Aufgaben

Stand: 20.11.2026. Nur offene Features, Korrekturen und Abnahmen.
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
F49/F80 mit lokaler Navigation und einmaliger Kingdom-Startbevoelkerung stehen in
[docs/navigation-and-founding.md](docs/navigation-and-founding.md).
F82 mit eigenstaendiger Editor-App sowie TAB-Baumodus und Stadtlager-HUD stehen in
[docs/construction-mode-and-editor.md](docs/construction-mode-and-editor.md).
F14 mit regionalem Wachstum, Tierklima und saisonalem Eis steht in
[docs/climate-growth-ice.md](docs/climate-growth-ice.md).
F68 mit Asphalt-Herstellung, Straßenformen und Rückbau steht in
[docs/fantasy-nature-sandworm-ambience.md](docs/fantasy-nature-sandworm-ambience.md).

## 1. NPCs, Siedlungen Und Waldwirtschaft


- [ ] **F81 Zufriedenheit Und Siedlungsstatistik:** Ein serverautoritaeres Zufriedenheits-/Statistiksystem fuer einzelne Villager und Kingdoms ergaenzen. Wohn-/Bettversorgung, Nahrung, Arbeitsbelastung, Sicherheit, Versorgung und erreichbare Arbeitsplaetze als nachvollziehbare Faktoren fuehren; bestehende Gesundheit/Beduerfnisse einbeziehen. Persoenliche Werte und Gruende sowie aggregierte Bevoelkerungs-, Berufs-, Versorgungs- und Sicherheitswerte im NPC-/Bergfried-UI darstellen. Auswirkungen auf Arbeit und Bindung schrittweise definieren, Grenzwerte/Hysterese, Rechte, begrenzte Updates sowie Save/Reload und Mehrspieler-Konsistenz pruefen.

- [ ] **F78 Neutrale Nomaden:** Gelegentlich kleine NPC-Nomadengruppen in geladenen neutralen Gebieten erscheinen und umherziehen lassen. Kingdom-/Dorfgrenzen, erreichbare Wege, Tagesrhythmus und begrenzte Spawn-/Simulationsbudgets beachten; kein Erscheinen in Spielerbauten oder beanspruchten Gebieten.

- [ ] **F79 Produktive Doerfer Und Bot-Doerfer:** Dorfaufbau um Berufsstationen, zugewiesene Arbeitsgebiete und echte Vorratstruhen erweitern. Holzfaeller und weitere unterstuetzte Arbeiter sollen ihre Arbeitsplaetze erreichen, Werkzeuge/Vorraete beziehen und Ertraege anliefern. Dorf-/Bot-Besitz und Rechte, begrenzte Simulation sowie Save/Reload erhalten.

## 2. Zuege, Strassen Und Pferdekutschen

### Fahrbetrieb Und Automatisierung



- [ ] **F65 NPC-Lokfuehrer:** Einen zuweisbaren Zugfuehrer-Beruf ergaenzen. Fuehrerstand erreichen, zugewiesenen Zug fahren, Weichen/Signale und Haltepunkte beachten sowie bei Hindernissen sicher bremsen. Wasser-/Brennstoffmangel, blockierte Strecke und fehlende Berechtigung anzeigen; Dienstzeit, Spieleruebernahme und Wiederaufnahme nach Laden regeln.

- [ ] **F66 Bahnhoefe, Fahrplaene, Signale Und Fracht:** Benannte Stationen/Haltepunkte mit Fahrplaenen, Wartezeiten und Zielwahl ergaenzen. Blockabschnitte/Signale fuer mehrere Zuege vorsehen. Fahrgastwechsel und begrenztes Be-/Entladen zwischen Wagen und Lagern ermoeglichen; Mengen, Filter und volle Zielinventare ueber vorhandene Inventar-/Logistiktransaktionen behandeln.

### Strassen Und Pferdekutschen

- [ ] **F69 Gemeinsame Wege Und Transportrouten:** Strassen, Arbeitsplaetze, Lager, Bahnhoefe und Kutschenhalte an die bestehende Navigation anbinden. NPCs/Pferde sollen geeignete Wege bevorzugen. Fahrzeugbreite, Steigung, Durchfahrtshoehe und Claim-Rechte beachten; Umleitungen und unpassierbare Verbindungen anzeigen. Wiederverwendbare Suchergebnisse und begrenzte Aktualisierungen fuer viele Auftraege nutzen.

- [ ] **F67 Pferdekutschen:** Kutsche mit Raedern, Deichsel, Geschirr und An-/Abkuppeln an vorhandene gezaehmte Pferde ergaenzen. Fahrersitz, Mitfahrer und echtes Ladungsinventar unterstuetzen. Lenkung, Rueckwaertsfahrt, Bremsen, Gespannkollisionen, Steigungen und Pferdeausdauer einbeziehen. Manuelle Fahrt und NPC-Routen an F69 anbinden; sichere Halte-/Anbindestellen sowie konsistente Speicherung von Gespann, Sitzen und Ladung vorsehen.

## 3. Spieler, Inventar Und Handwerk

- [ ] **F48 Tragbare Taschen Und Rucksaecke:** Taschen/Rucksaecke mit eigenem gespeichertem Inventar und passender Ausruestungs-/Taschenansicht ergaenzen. Slotbesitz, Zugriff, Verlust/Drop, Mehrspielertransaktionen und begrenzte Verschachtelung definieren; Inhalte beim Tragen, Ablegen und Zerstoeren erhalten.

- [ ] **F45 TMI-Prozessketten Wie Bei Create:** Vorhandene Stationsrezepte um zusammenhaengende Verarbeitungsketten erweitern. Benoetigte Antriebe, Fluessigkeiten, Transportverbindungen und Zwischenprodukte aus echten Rezept-/Maschinendefinitionen zeigen; zwischen verknuepften Schritten navigieren koennen.

- [ ] **F47 Chemie Und Experimentieren:** Experimentelle Mischungen im vorhandenen Trank-/Kesselsystem ergaenzen. Zutaten, Mengen, Wasser, Erhitzen und Werkzeuge beeinflussen Ergebnisse; Kombinationen, Fehlschlaege und Entdeckungen speichern und erklaeren. Bestehende Effekt-/Rezeptregistries verwenden.

- [ ] **F32 Kaelte Und Hitze:** Umgebungstemperatur aus Tageszeit, Jahreszeit, Biom, Hoehe, Wasser/Nassheit und Schutz ermitteln, einschliesslich kalter Wuestennaechte/heisser Sommer. Frieren, Resistenz, Kleidung und Lagerfeuerwaerme einbeziehen; Hitze als eigenen Status ergaenzen. Wirkung langsam auf-/abbauen; Werte, Schutz und HP-/Stamina-Rueckmeldung autoritativ speichern/replizieren.

## 4. Weitere Natur-, Welt- Und Darstellungsaufgaben

### Natur Und Kreaturen

- [ ] **F39 Skelettkrieger:** Nachts im Spirit Blossom ungeruestete Schwertkaempfer, Kaempfer mit Eisenruestung/Eisenschwert und unbewaffnete Varianten ergaenzen. Bei Tagesanbruch sichtbar eingraben und danach despawnen; bestehende KI, Navigation, Ausruestung und Spawn-Budgets verwenden.


- [ ] **F71 Hoehlenoekologie Und Unterirdische Wasserlaeufe:** Hoehlenbegruenung mit Wachstumszyklen sowie zusammenhaengende unterirdische Wasserlaeufe ueber einzelne lokale Reservoirs hinaus ergaenzen. Vorhandene Kammern, Feuchtigkeit, Hoehenprofile und Fluidpersistenz verbinden; Generierungsaenderungen versionieren.

### Welt Und Darstellung


- [ ] **F15 Schaeden An Weltobjekten:** Brand-/Lebenspunktschaeden fuer geeignete Objekte ergaenzen. Treffer, Feuer, Zustand, Zerstoerung und Drops an vorhandene autoritative Welt-/Inventarpfade anbinden.
- [ ] **F85 Feuer und Drachenatem.** Brennbare Weltblöcke entzünden, Feuer ausbreiten und ausbrennen lassen. Feuerangriffe von Drachen sollen am Einschlag brennende Stellen hinterlassen.
- [ ] **F86 Feuer mit Wasser löschen.** Platziertes und fließendes Wasser soll Feuer bei Kontakt löschen und die Ausbreitung an dieser Stelle beenden.

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

## 7. Release-Pipelines

- [x] **F83 Pull-Request-Prüfung einrichten.** Auf einem frischen GitHub-Actions-Runner die in der README festgelegte TiltUI-Revision am von Cargo erwarteten Pfad auschecken und die Toolchain aus `rust-toolchain.toml` verwenden. Formatierung, Clippy und Workspace-Tests mit `Cargo.lock` ausführen.
- [x] **F84 Release-Pakete aus einem Versionstag bauen.** Client, Dedicated Server und Gebäudeeditor für die unterstützten Zielplattformen im Release-Profil bauen. Die jeweils benötigten Assets, Standardkonfigurationen und Lizenzen beilegen. Pakete versionieren und SHA-256-Prüfsummen erzeugen.
- [x] **A08 Release-Kandidaten abnehmen und veröffentlichen.** Die Pakete für Linux, macOS und Windows haben jeweils eine Welt gespeichert und neu geladen, einen Client mit dem Server verbunden und ein Gebäude im Editor geöffnet. Alle drei geprüften Archive sind mit SHA-256-Prüfsummen und Versionshinweisen veröffentlicht. [Teramic 1.0.0](https://github.com/exepta/teramic/releases/tag/v1.0.0) · [Native Paketabnahme und Veröffentlichung](https://github.com/exepta/teramic/actions/runs/45068635638)

## 8. Verbesserungen
- [x] **B01** Beim Ziehen der Audioregler ändern sich die laufenden Spielgeräusche. Jeder Regler spielt eine passende Hörprobe. „Übernehmen“ speichert die Pegel, „Verwerfen“ stellt sie zurück.
- [ ] **B02** Windows Schwere Render FPS probleme bei einer 3060TI 20 - 30 FPS manchmal auch 5FPS.
