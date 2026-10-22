# TERAMIC: Features Und Optimierungen

Diese Datei enthaelt nur offene Aufgaben. Technische Altaufgaben haben Vorrang
vor neuen Features. Innerhalb der Gruppen gilt die angegebene Reihenfolge;
Abhaengigkeiten stehen direkt am Eintrag. Erledigte, gepruefte Arbeit kommt nach
[PATCH.md](PATCH.md), Architektur und Grenzen nach [README.md](README.md).

`F` = Feature, `O` = Optimierung/Fehlerkorrektur, `A` = Abnahme mit besonderen
Voraussetzungen. IDs bleiben bei Umordnungen stabil. Neue Inhalte sind nicht
automatisch umgesetzt, nur weil sie hier eingeordnet sind.

Die bisherigen 100 offenen Eintraege sind in der
[Altstand-Zuordnung](tools/benchmarks/results/technical-backlog-2026-10-22/migration.md)
vollstaendig nachvollziehbar. Historische Messwerte und Einschraenkungen bleiben
im [unveraenderten Altstand](tools/benchmarks/results/technical-backlog-2026-10-22/previous-todo.md)
und den dort verlinkten Pruefberichten erhalten.

## Features

### Neue Features In Abhaengigkeitsreihenfolge

F01 (Buffs/Debuffs) ist mit Phase 104 implementiert und geprueft. Die 13
Effekte, automatische Ausloeser, HUD, PLY5-Migration und Replikation sind im
[Status-Effekt-Bericht](tools/benchmarks/results/status-effects-2026-10-22/README.md)
dokumentiert. Ruestung und weitere natuerliche Ausloeser bleiben getrennte Features.

- [ ] **F02 Ruestungen Und Ausruestung:** Die acht vorhandenen Platzhalter-Slots zu echten, servervalidierten persistenten Ausruestungsplaetzen ausbauen. Ruestungsitems, Schutzwerte und Werteboni; HP-/Stamina-HUD um Ruestungsanzeige ergaenzen. Mit F01 abstimmen.
- [ ] **F03 Crusher:** Neuer Technikblock zum Zerkleinern, insbesondere fuer Farbstoff. Einzelne JSON-Rezepte, Ein-/Ausgabeinventar, Fortschritt, Bedienung und Too-Many-Items-Rezeptansicht; bestehende Farbstoffe/Rezepte nicht still ungueltig machen.
- [ ] **F04 Ernte Und Anbau:** Weizen, Mais und Salat mit Aussaat, Wachstumsstadien, Ernte und Nachpflanzen. Feuchtigkeit, Licht und Jahreszeiten ueber die vorhandene SeasonSample-API einbeziehen; gefressene Futterpflanzen kontrolliert nachwachsen lassen.
- [ ] **F05 Tiere Zaehmen:** Artspezifisches Futter, Vertrauen, Besitzer und Folge-/Warteverhalten. Vorhandene persistente Besitzerzuordnungen verwenden; gezaehmte Tiere nicht auf das natuerliche Spawnlimit anrechnen. Baut auf F04/F01 auf.
- [ ] **F06 Mob-Babys Und Zucht:** Fortpflanzung, Nachwuchs, Wachstum und Abklingzeiten. Junge Tiere mit passenden Groessen/Animationen; Eltern-, Besitzer- und Populationsregeln speichern und replizieren. Bestehende Tier-IDs nicht ersetzen.
- [ ] **F07 Pferde Reiten:** Auf-/Absteigen, Sitzposition, Kontrolle, echte Spruenge und laufender Ausdauerverbrauch. Bestehende zufaellige Sprung-/Ausdauerwerte und Fellvarianten erhalten; mit Zaehmung F05 und Wachstum F06 verbinden.
- [ ] **F08 Ganzbaumfaellen:** Am Stammfuss mit Sneaken + Abbauen den ganzen natuerlichen Baum faellen; Gesamtdauer 0,5 s je Holzblock. Strukturzuordnung, begrenzte Edit-/Drop-Batches, getrennter Laubzerfall und Schutz von Spielerbauten. Normales Einzelblock-Abbauen bleibt erhalten.
- [ ] **F09 Strukturen:** Minen, Ruinen und Doerfer deterministisch generieren, mit Terrainanpassung, chunkuebergreifender Platzierung und passenden Weltinventaren. Gespeicherte Spielerbauten und alte Weltvertraege schuetzen.
- [ ] **F10 Villager-Grundsystem:** Bewohner fuer Doerfer mit persistenten Identitaeten, Grundbeduerfnissen, Tagesablauf, Navigation und erster Interaktion. Auf F09 und die bestehenden Mob-/Inventarsysteme aufbauen; Handel und Berufe als naechste Ausbaustufe definieren.
- [ ] **F11 Character Customization:** Spielbare Charakteranpassung mit Vorschau, Varianten/Farben und gespeicherter Auswahl. Lokalen Spieler, entfernte Avatare und Inventarportraet konsistent darstellen; bestehende --gender-Profile weiter lesen.

### Bestehende Erweiterungen Beibehalten

- [ ] **F12 Bienen Und Honig:** Berechtigte Honigentnahme aus dem echten Nestzustand, Bestaeubung und Fortpflanzung. Vorhandenen Nektar, Reifung, Nestkapazitaet und IDs erhalten; Nachwuchs an F06 anbinden.
- [ ] **F13 Wassertransport:** Eimer und spaeter Pumpen mit Crafting/Inventar, autoritativer Entnahme und Platzierung sowie gespeicherten Quellen. Kein zweites Fluessigkeitssystem.
- [ ] **F14 Saisonales Wachstum:** Dirt->Grass, Tierverhalten und spaetere Ernten an regionale Feuchtigkeit/Jahreszeiten anbinden. Kontrolliertes Gefrieren/Schmelzen von Eisblock 49 und dessen Rutschphysik; keine Zerstorung von Spielerbauten oder Wasserquellen.
- [ ] **F15 Weltobjekte:** Brand-/Lebenspunktschaeden fuer geeignete Objekte; Truhendeckel mit berechtigten Betrachtern und Oeffnungs-/Schliessgeraeuschen. Kosmetisches Feuer allein ist kein Schadenssystem.
- [ ] **F16 Mod-Inhalte:** Neue Schmelz-, Brennstoff- und Nahrungsdefinitionen ueber die vorhandene Inhaltsabstimmung replizieren; validierte JSON-Kataloge und spaeter Live-Reload. Keine zweite Item-/Rezeptregistry.
- [ ] **F17 Multiplayer-Aktionen:** Allgemeine autoritative Schlagereignisse auch fuer Creative/Luft/Mobs und expliziten Schwimmantrieb replizieren; Remote-Pose nicht aus Stroemungsgeschwindigkeit erraten. Husten/Tierlaute als budgetierte gemeinsame Audioereignisse.
- [ ] **F18 Spectator:** Spieler-Kamerabesitz sowie vollstaendige Abkopplung von Populationsfokus, Wassereffekten und fremden Platzierungskoerpern. Vorhandenen kollisionslosen Flug und gesperrte Weltinteraktionen erhalten.
- [ ] **F19 Dimensionen Und Gemeinsame Karten:** Identitaeten und Speicherpfade vorab festlegen; Spieler, Container, Karten und Weltzustand konsistent trennen. Gemeinsame/serverseitige Wegpunkte auf dieser Grundlage ergaenzen.
- [ ] **F20 Erweiterte Darstellung:** Optional budgetierte Offscreen-Reflexionen, RGB-/Skylight-Ausbreitung, indirektes Licht und echter volumetrischer Nebel. Neue Lava/leuchtende Pflanzen an vorhandene Emissionsmetadaten anbinden; Lava-Gameplay separat definieren. SSR ist kein Offscreen-Rendering und SSAO keine GI. Statischen Custom-Mesh-Import nur fuer benoetigte Spezialassets um Animation/Skinning erweitern.
- [ ] **F21 Weitere Inhalte:** Artspezifische Baeren-/Pferdelaute, zusaetzliche Kau-/Schuettel-/Umgebungsaufnahmen und passende Drops. Weitere Unterwasser-Wand-/Deckenpflanzen, Hoehlenbegruenungszyklen und globale unterirdische Wasserlaeufe getrennt von lokal funktionierenden Reservoirs planen.
- [ ] **F22 Ressourcen-Stapelphysik:** Gegenseitige Kollision und Stapelbildung loser Items bei tatsaechlichem Gameplaybedarf; bewegte Drops und Einsammeln weiterhin budgetieren.
- [ ] **F23 Rollen Und Online-Accounts:** Chat-Moderation/Rollen sowie verifizierter Login-/Sessiondienst mit API, Vertrauensschluesseln und Gateway-Pruefung. Lokale UUIDs sind kein Besitznachweis; ohne Verifier bleibt oeffentlicher Zugang geschlossen.
- [ ] **F24 Erweiterte Fernkarte:** Budgetierte entfernte 3D-Rekonstruktion mit getrennten Boden-/Kronenhoehen oder echten Mesh-LODs. Kein World-Streaming ohne Budget und keine kuenstlichen Waende aus gemittelten Baumhoehen. Hoehlenkarten bleiben flach.

## Optimierungen

O01, O03, O04, O05 und O06 sind mit Phase 102 als konkrete Codekorrekturen
umgesetzt und geprueft. Implementierung, 1734 bestandene Tests, native
Funktionspruefungen und unverkuerzte Vergleichsmessungen stehen im
[Phase-102-Bericht](tools/benchmarks/results/render-water-2026-10-22/README.md).
Das ist keine allgemeine FPS-/Speicherparitaets- oder Langzeitabnahme;
gezielte verbleibende Pruefungen bleiben unter A01, A04 und A05 offen.

Die danach gemeldete Inventar-FPS-Regression ist mit Phase 103 im UI-Runtime-
und Vorschau-Bildpfad korrigiert: im gleichen nativen 4K-/20er-Blick offen
32,79 -> 68,38 FPS, bei 2880x1620/12 offen 43,22 -> 90,99 FPS. Reproduzierbare
Vergleiche, Regressionstests und ausdrueckliche Grenzen stehen im
[Phase-103-Bericht](tools/benchmarks/results/inventory-fps-2026-10-22/README.md).
Das ist keine vollstaendige Abnahme aller Streaming-/GPU-/Langzeitspitzen;
A05 bleibt offen. Performance hat weiterhin Vorrang vor neuen Features.

### Abnahmen Und Externe Voraussetzungen

- [ ] **A01 Grafik-Abnahme:** Blockformen/-oberflaechen, Kronen, Gras, Blumen, Pilze, Mineralien, Hoehlen, Ufer, Korallen, Anemonen und Tiere gemeinsam an nativen Referenzansichten beurteilen. Avatar-Gesichter, Griffpunkte und menschliche Animationen einschliessen; keine unbelegte 1:1-Qualitaet zusagen.
- [ ] **A02 Audio-Abnahme:** Tier-/Schritt-/Ess-/Ofen-/Oeffnungs-/Husten-/Wasser-/Wetterklang mit Kopfhoerern und Lautsprechern menschlich pruefen. Dekodiertests sind keine Hoerabnahme und bearbeitete Aufnahmen keine neuen artspezifischen Feldaufnahmen.
- [ ] **A03 Weitere Plattformen:** Serverkonsole unter Windows/macOS, PowerShell/Legacy-Terminals, Signalabbruch und Pipe-/Zeileneingabe nativ pruefen. Linux-PTY-Tests reichen dafuer nicht aus.
- [ ] **A04 Mehrere Rechner:** Reale LAN-/Internet-Sitzungen mit zweitem Rechner, Firewall/Router, Reconnect und hoher Latenz pruefen. Lokale UDP-Clients ersetzen diese Abnahme nicht. Zwei weit getrennte Streaming-Foci mit getrenntem Verlassen und Fluid-Pause/Reload-Wakeup als kombinierten Lauf ergaenzen; Union, Unload und Wakeup sind bisher einzeln getestet.
- [ ] **A05 Weitere GPUs Und Langzeitlast:** Speicher-/Framezeit-/VRAM-Verlauf bei dichten 32er-Streaming-, Wasser-, Wetter-, Tier- und Edit-Laeufen auf weiterer Hardware pruefen. Windlaub-/Blend-Overdraw sowie isolierte Near-/Middle-Pilzpark- und Riffkosten mit identischer Geometrie messen; Phase 102 reduziert Kompositions- und Indexdatenkosten, behauptet aber keine GPU-/Speicherparitaet. Einzelne hohe Frames und die schwankenden 4K-Regenwerte bleiben dokumentiert. Viele Avatare, Herden und Itemdrops einschliessen. Keine neuen 10-/15-/30-Minuten- oder mehrstuendigen Laeufe ohne ausdrueckliche Freigabe; vorhandene historische Messwerte bleiben als solche dokumentiert.
- [ ] **A06 Weltgenerierungs-Abnahme:** Viele neue Seeds, negative Koordinaten, extreme Kammern/Regionsecken, Seen/Fluesse, Schluchten und Oasenraender pruefen. Y-Grenzen, Grundstein, Erzhoehen und Salzbiom-Regeln erhalten; kein stiller Retrogen historischer Welten.

### Abschlussregeln

- Vorhandene Seed-/Welt-/Block-/Mob-IDs, logische Meterzellen und 16er-Sections erhalten. Dedicated Server bleibt headless.
- Singleplayer und Multiplayer verwenden dieselbe Autoritaet; vorhandene Registries, Inventare, Konfiguration und binaere Regionen erweitern, keine konkurrierenden Systeme oder unnoetigen Dependency-Upgrades.
- Aenderungen passend auf Determinismus, negative Koordinaten, Interaktion, Replikation, Save/Reload und Ressourcenfreigabe pruefen. Normale Bloecke gemeinsam meshen; keine Entity pro Micro-Voxel.
- Bei Codeaenderungen: cargo fmt --all --check; cargo check --workspace; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings; cargo build --workspace. Native Kurztests zum geaenderten Verhalten ausfuehren.
- Nur tatsaechlich implementierte und gepruefte Teilaufgaben abschliessen. Offene Teilaufgaben behalten ihren Platz; externe Abnahmen und ungemessene Performance nicht als bestanden ausgeben.
