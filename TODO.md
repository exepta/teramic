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

Die Oberflaechen-/HP-Balken-/Netzkorrekturen aus Phase 118 sind gesondert in
[PATCH.md](PATCH.md) und im
[Pruefbericht](tools/benchmarks/results/render-health-webs-2026-10-24/README.md)
dokumentiert. F35-F39 und das groessere organische Hoehlenraum-Redesign F36
bleiben offene Features; eine geringere Gangdichte ist nicht deren Abschluss.

Die gezielten Bewegungs-, Reit-, Karten-/HP-Balken- und Vegetationskorrekturen
aus Phase 119 sind im
[Mob-/Streamingbericht](tools/benchmarks/results/mob-movement-streaming-2026-10-24/README.md)
dokumentiert. Kurze native Streaming-Vergleiche ersetzen nicht A01/A04/A05.

Phase 124 ergaenzt faerbbare Teppiche/Wolle, bepflanzbare Blumentoepfe und
gemeinsam oeffnende 1x2-/2x1-/2x2-Fenster. Feldfortschritt und Wasser-Deltas
gleichen unveraenderte Pflanzen nicht mehr erneut ab. Die getrennte Messung
weist Fackel-Cubemaps als grossen Dorf-Engpass nach; die konservative
Sonnenlicht-Klassifikation spart Cubemaps offen sonnenbeschienener Quellen.
Der kurze Feldvergleich steigt von 43,33 auf 55,98 FPS; notwendige lokale
Schatten bleiben ein erheblicher Engpass, kein allgemeiner FPS-Abschluss.
Neue Wasserfall-Terrainrevisionen erhalten offene, felsige Uebergaenge; alte
Weltvertraege bleiben erhalten. Nachweise und konkrete Grenzen:
[Dorf-/Dekorationsbericht](tools/benchmarks/results/village-fields-decor-2026-10-25/README.md).

## Features

### Neue Features In Abhaengigkeitsreihenfolge

F01 (Buffs/Debuffs) ist mit Phase 104 implementiert und geprueft. Die 13
Effekte, automatische Ausloeser, HUD, PLY5-Migration und Replikation sind im
[Status-Effekt-Bericht](tools/benchmarks/results/status-effects-2026-10-22/README.md)
dokumentiert. Ruestung und weitere natuerliche Ausloeser bleiben getrennte Features.

F02 ist mit Phase 105 implementiert: fuenf Ruestungssets, vier Accessoires,
neun echte Plaetze, Schutz-/Werteboni, persistente Replikation und silberne
Ruestungsleiste. Herz-/HP-Overlays verwenden nur den ersten Status-Effekt.
Pruefungen, native Aufnahmen und Grenzen stehen im
[Ausruestungsbericht](tools/benchmarks/results/equipment-2026-10-22/README.md).

F03 ist mit Phase 106 implementiert: animierter Zweiblock-Crusher, private
Ein-/Ausgabe, gespeicherter Fortschritt und 16 einzelne Zerkleinerungsrezepte
mit Too-Many-Items-Ansicht und Plus-Transfer. Alte Farbstoff-Handrezepte bleiben
gueltig. Zusaetzlich: integrierter Schild im Stamina-Rahmen und faerbbare Capes
in fuenf Stufen mit zehntem Ausruestungsplatz. 1850 Workspace-Tests sowie die
nativen Ausruestungs-/Crusher-Pruefungen bestanden; Nachweise und Grenzen im
[Capes-/Crusher-Bericht](tools/benchmarks/results/capes-crusher-2026-10-22/README.md).

Phase 107 erweitert F03 um eine montierbare Handkurbel und echte manuelle
Bedienung mit getrennten Mechanik-/Mahlgeraeuschen und fallenden Spaenen.
Capes bleiben hinten und folgen Gang/Wind. Hoeheres Inventar, direkte
Hover-Tooltips, begrenzter Ruestungsrahmen und animierte Gifttropfen:
[Handkurbel-/UI-Bericht](tools/benchmarks/results/manual-crusher-2026-10-22/README.md).

Phase 108 ergaenzt die geschlossene Crusher-Verkleidung und Material-Foley,
13 Farbstoffe fuer Betten/Capes, sechs Werkzeug-Materialstufen und gerade
Blockkanten. Umsetzung und Pruefnachweise:
[Material-/Crusher-Bericht](tools/benchmarks/results/materials-crusher-2026-10-23/README.md).

F04 ist mit Phase 109 implementiert: Weizen, Mais und Salat mit acht Stufen,
Samen, vorbereitetem Ackerboden, Ernte/Nachpflanzen, Nahrungs-/Samenrezepten
und Too-Many-Items-Integration. Feuchtigkeit, Tageslicht und SeasonSample
steuern gespeichertes Wachstum; gefressene Kulturen und Wildfutter wachsen
kontrolliert nach. Umsetzung, native Aussaat-/Erntepruefung und Grenzen:
[Anbaubericht](tools/benchmarks/results/farming-2026-10-23/README.md).

Phase 110 erweitert den Anbau um Giessekannen, Hacken und Sensen in fuenf
Materialstufen, eigene Ackertexturen, dekorative Vogelscheuchen und eine echte
Wachstumsleiste. Werkzeughaltung, Jahreszeitenzeiger, Creative-Komplettleeren
und der CreatureStates-Empfang sind ebenfalls ueberarbeitet. Funktionsregeln,
Pruefungen und Grenzen:
[Farming-Werkzeuge](tools/benchmarks/results/farming-tools-2026-10-23/README.md).

Phase 111 korrigiert vorbereiteten Ackerboden/Salataussaat, zeigt auf Ackerboden
Wachstum beziehungsweise fehlende Samen und blendet Pflanzenstufen weich um.
Minimap-Hoehen, Hoehlen-/Unterwasserlicht und Daemmerung sind korrigiert;
drei Spinnenarten, verankerte Netze und Schwimmspuren erweitern die vorhandenen
Systeme. Umsetzung und Pruefgrenzen:
[Spinnen-/Pflanzen-/Lichtbericht](tools/benchmarks/results/spiders-farming-light-2026-10-23/README.md).

Phase 112 ergaenzt Wasserbedarf/Bodenfeuchte und behebt kollidierende IDs bei
benachbarter gemischter Aussaat. Hohe Hoehlen laden ihre echte Dach-/Bodenschale;
neue Welten nutzen organischere Hoehlenrevision 5 mit kleinen Wasserkanalauslaeufen.
Blockige Mineralien, gezielte Tragkettenpruefung, heller abgestimmte Hoehlen und
Fackeln, bessere Tier-Unterstuetzung/Hindernispruefung, aktivere braune Spinnen,
F1+E-Absichtsinspektion, echte Platzierungssilhouetten, bewegliche Truhendeckel
und budgetierte Crusher-/Vogelscheuchenlaternen sind umgesetzt. Nachweise und
Grenzen: [Bau-/Acker-/Hoehlenbericht](tools/benchmarks/results/building-farming-caves-2026-10-23/README.md).

F05 ist mit Phase 113 implementiert: Schafe, Kuehe, Schweine, Pferde und Spinnen
verwenden artspezifisches Futter, Vertrauen und persistente Besitzer.
Gezaehmte Tiere bleiben vom natuerlichen Spawnlimit ausgenommen. F33 ergaenzt
explizite Follow-/Rest-/None-Befehle ueber die Pet-Bedienoberflaeche; alte
gespeicherte Folge-Einstellungen aktivieren weiterhin kein automatisches Folgen.
Netzprioritaet und echte Nahrungssuche der Spinnen, Nachtsicht-/Effektbefehl,
Truhengeraeusche und Interaktionskorrekturen sind im
[Zaehmungs-/Interaktionsbericht](tools/benchmarks/results/taming-effects-interaction-2026-10-23/README.md)
mit Regressionstests und kurzen nativen Pruefungen dokumentiert.

F06 ist mit Phase 115 implementiert: ausdrueckliche Zucht eigener erwachsener
Schafe, Kuehe, Schweine, Pferde und Spinnen durch zusaetzliches passendes Futter.
Nachwuchs mit weichem Wachstum, jugendlichen Proportionen/Animationen,
groessenabhaengiger Physik, Eltern-/Besitzerdaten und Abklingzeiten ist gespeichert
und repliziert. Rotierende Such-/Geburtsbudgets und Populationsgrenzen verhindern
unkontrollierte Vermehrung; bestehende Tier-IDs bleiben erhalten. Dazu kommen
autonomer Netzbau gezaehmter Spinnen ohne Folgen, echte vertikale Bodenslabs,
getrennter Abbau gemischter Slabs und hellere Zwei-Pixel-Baukonturen.
2091 Workspace-Tests sowie kurze native Bau-/Zuchtpruefungen bestanden.
Regeln, Kompatibilitaet und Grenzen:
[Nachwuchs-/Slab-/Spinnenbericht](tools/benchmarks/results/babies-slabs-spiders-2026-10-23/README.md).

Phase 116 erweitert F06 um stabiles Maennlich/Weiblich fuer alle Tier-IDs.
Neue Paarungen erfordern unterschiedliche Geschlechter; Nachwuchs erhaelt sein
eigenes Geschlecht. Historische Familien und Tier-IDs bleiben erhalten.
Spinnen nutzen kontaktgepruefte Wand-/Deckenwege und echte Absatzlandungen;
der neue Stalker verwendet begrenzte Hoehlenrouten und kollisionsgeprueften
Spielerzug. Umsetzung, kurze native Pruefungen und verbleibende Abnahmen:
[Kletter-/Stalker-/Streamingbericht](tools/benchmarks/results/stalker-climbing-streaming-2026-10-24/README.md).

F07 ist mit Phase 117 implementiert: eigene gezaehmte erwachsene Pferde per
Platzier-/Tierinteraktion ohne Futter besteigen, mit WASD lenken, sprinten,
individuell hoch springen und seit Phase 119 mit X sicher absteigen. Shift
laesst das Pferd sprinten. Server und Vorhersage
teilen denselben Pferdemotor; Sitzpose und Kamera folgen demselben Sitzpunkt.
Pferdeausdauer wird im bestehenden HUD angezeigt. Fremde, wilde und junge Pferde
bleiben gesperrt; Tier-IDs und feste Sprung-/Ausdauerwerte bleiben erhalten.
Kurze native Reitprobe mit echtem UDP, Sprung, Absteigen und abschliessendem
Speichern sowie Schnee-/Autosave-Regressionen:
[Reit-/Schnee-/Autosavebericht](tools/benchmarks/results/riding-snow-autosave-2026-10-24/README.md).

- [x] **F33 Pet-Bedienoberflaeche:** Sneak + Platzieren auf dem eigenen Tier oeffnet ein dunkel-goldenes Radialmenue mit echtem Modellportraet, HP-Herzen und Follow/Rest/None. Autoritative Besitzer-/Reichweiten-/Sichtpruefung, bestehende begrenzte Navigation, gespeicherte explizite Auftraege und kein automatisches Aktivieren alter Follow-Felder. None erlaubt autonomes Verhalten, Rest behaelt Fallen/Schwimmen/Pferdesprung bei. Phase 122 ueberarbeitet den Referenzaufbau mit vollem Ring, erhoehtem Follow-Segment, runden Medaillons, aeusseren Beschreibungstafeln und schmaler Fensteransicht. [Grundsystem](tools/benchmarks/results/pets-trees-water-location-2026-10-24/README.md), [neues HUD](tools/benchmarks/results/villages-pets-spiders-2026-10-24/README.md).
- [x] **F08 Ganzbaumfaellen:** Shift + gehaltene Abbau-Taste an jedem natuerlich zugeordneten Holzblock, einschliesslich Aesten. Zeiten je Holz: Holz-Axt 0,1 s, Stein 0,0925 s, Eisen 0,08 s, Gold 0,09 s, Diamond/Ruby 0,05 s. Gold und Ruby zaehlen jeden dritten Block nur im Timer nicht mit; der ganze Baum bleibt Teil des Abbaus. Creative unterstuetzt die zeitbasierte Axt-Aktion ohne Gratis-Drops, ohne Shift weiterhin Sofort-Einzelabbau. Exakte vorbereitete Baumstruktur, bedarfsgesteuertes Section-Laden, begrenzte Edit-/Drop-Batches und Schutz von Spielerbauten/Nachbarbaeumen. Grafikoptionen Current, Hell Fall und Magic (5 s), materialgebuendelt und ohne Entity je fallendem Block. [Grundsystem](tools/benchmarks/results/pets-trees-water-location-2026-10-24/README.md), [aktuelle Erweiterung](tools/benchmarks/results/tree-structures-performance-2026-10-25/README.md).
- [x] **F09 Strukturen:** Deterministische Doerfer in Wueste, Plains/Forest und Highlands, Ruinen und abgestuetzte Bergminen. Revision 2 ergaenzt drei seedabhaengige Dorfgrundrisse, gedrehte Gebaeude, Veranden, Dachueberstaende, Fensterrahmen, Marktstaende und Zaun-/Slab-Details. Verzweigte Wege verwenden ausgerichtete Dirt-Treppen und -Slabs, erhalten bestehende Steigungen und pruefen ihre Fundamente. Neue Minen betreten echte Berghaenge mit gepruefter Felsueberdeckung statt oberirdischer Huetten. Revision 3 ergaenzt Teppiche und bepflanzte Toepfe in Innenraeumen, korrigiert Maschinenfronten und gedrehte Werkbank-Peers sowie Truhen in Minen/Ruinen und verwendet Stein-Zaun/Slab/Treppen in Ruinen. Einmalige Objektregistrierung ohne kuenstliche Terrain-Edits verhindert verworfene Nachbarjobs; Treppenprofile werden je Mesh-Job begrenzt wiederverwendet. Bestehende Welten behalten ihre Strukturversion und Spielerbauten. Bewohner/Berufe gehoeren zu F10. [Urspruengliche Umsetzung](tools/benchmarks/results/villages-pets-spiders-2026-10-24/README.md), [Ueberarbeitung](tools/benchmarks/results/settlement-streaming-2026-10-24/README.md), [Revision 3](tools/benchmarks/results/tree-structures-performance-2026-10-25/README.md).
- [x] **F10 Villager-Grundsystem:** Persistente Bewohner mit Namen, zehn Rollen-/Modellvarianten, Grundbeduerfnissen, begrenzter Navigation, Dorfspaziergaengen, Heimwegen, individuellen Schlafzeiten und Begruessung per Rechtsklick. Bestehende Mob-Snapshots, Regionsspeicherung und Bett-/Tuersysteme werden genutzt; Bewohner zaehlen nicht zum natuerlichen Tierlimit. Neue Strukturrevision 4 unterscheidet kleine (6-8), mittlere (10-16) und grosse (18-24) Wohnhaeuser plus die geforderten Medic-/Kirchen-/Schmied-/Polizeigebaeude, mit mindestens zwei Betten pro Wohnhaus. Bestehende Weltversionen behalten ihre Bauten. Handel, Berufsproduktion und Wirtschaft bleiben eine spaetere Ausbaustufe. [Umsetzung, Messungen und Grenzen](tools/benchmarks/results/villagers-pipes-physics-2026-10-25/README.md).

Phase 127 ueberarbeitet alle zehn Bewohner-Modelle anhand der Referenz mit
geschichteter Kleidung/Ruestung, Haarstraehnen, detaillierten Gesichtern,
Korbgeflecht, Roben, Werkzeugen und Berufstaschen. Innenflaechen werden
entfernt, gleichfarbige Flaechen verbunden; weiter sechs gemeinsam genutzte
Koerperteile je Rolle statt Entities je Detail. Geh-/Bettpose und echte
Clientaufnahmen sind geprueft. Keine behauptete pixelgenaue Referenzkopie:
[NPC-Details und Performance](tools/benchmarks/results/npc-detail-performance-2026-10-25/README.md).

- [ ] **F11 Character Customization:** Spielbare Charakteranpassung mit Vorschau, Varianten/Farben und gespeicherter Auswahl. Lokalen Spieler, entfernte Avatare und Inventarportraet konsistent darstellen; bestehende --gender-Profile weiter lesen.

### Neue Biome Und Natur

F34 ist mit Phase 128 implementiert: zusammenhaengende Sakura-Regionen mit
pinken/weissen Kronen und Blumen, dezent rosa Wasser, Kartenfarben und schmalen
Baechen mit natuerlichem Gefaelle. Auch Forest und Highlands erhalten solche
Baeche ueber die bestehende Hydrologie. Neue Welten verwenden
`ecology_revision=1`; alte Weltvertraege behalten ihre gespeicherte Revision.
Generierte Regionen, Baeche, Save-Kompatibilitaet und Grenzen:
[Kirschwald-/KI-/Lichtbericht](tools/benchmarks/results/cherry-ai-light-2026-10-25/README.md).

- [ ] **F35 Spirit Forest:** Wasserfallreiches Biom mit gruen-tuerkisem Laub, blaulich schimmernden leicht transparenten Spirit Mushrooms, seltenem Spirit Crystal und violetten Haengeranken. Lokale Nebelstellen aus einem erdaehnlichen Nebelblock unter Gras mit kleinen pulsierenden blauen Steinen; budgetierte Darstellung ohne eine Lichtquelle/Entity je Detail.
- [ ] **F36 Natuerliche Hoehlenraeume:** Bestehende Kammer-/Connector-Engine organischer gestalten: unregelmaessige, geologisch lesbare Raeume statt auffaelliger Kugeln. Terrassen, wechselnde Querschnitte und natuerliche Uebergaenge; keine zweite Hoehlenengine und keine stille Regeneration bestehender Welten.
- [ ] **F37 Schmetterlinge:** Freundliche Tiere mit sieben Varianten in Cherry Forest, Forest und Plains. An bestehende Spawn-/Tier-Systeme anbinden; fuer Spinnen als echte Beute verwenden.
- [ ] **F38 Spirit Gloom:** Zwei freundliche fliegende Varianten ausschliesslich im Spirit Forest. In der Naehe Regeneration I ueber das bestehende Effektsystem mit begrenzter Reichweite/Updates vergeben; kein unbegrenztes Stapeln der Aura.
- [ ] **F39 Skelettkrieger:** Feindliche Bewohner des Spirit Forest nur nachts: ungeruesteter Schwertkaempfer, Eisenruestung/Eisenschwert und unbewaffnete Variante. Bei Tagesanbruch sichtbar in den Boden eingraben und erst danach despawnen. Vorhandene KI, Navigation, Ausruestung und Spawn-Budgets verwenden.
- [ ] **F40 Abwechslungsreichere Weltgenerierung:** Mehr erkennbare Landschaftsformen, wechselnde Reliefs, markante Orte und natuerliche Uebergaenge statt gleichfoermiger Gebiete. Bestehende Terrain-/Biom-/Strukturengine erweitern und mit mehreren Seeds pruefen; gespeicherte Welten nicht still regenerieren.
- [ ] **F41 Groessere Verbundene Seen:** Unterschiedlich grosse, auch ausgedehnte Seen mit natuerlichen Ufern und gelegentlichen Fluss-Zu-/Abfluessen. Bestehende Hydrologie und echte Wasserzellen verwenden; Hoehenanschluesse, Chunk-Grenzen und stabile Becken testen.
- [ ] **F42 Laengere Fluesse:** Laengere zusammenhaengende Flusslaeufe mit unterschiedlichen Laengen und Verbindungen zwischen Quellen, Seen und Meer. Natuerliches Gefaelle, Wasserfaelle und begrenzte Generierungskosten erhalten; keine unabhaengige zweite Flussengine.

### Kingdom

- [ ] **F43 Kingdom-Grundsystem Und Bergfried:** Kingdom wird das zentrale Spielsystem. Als erste Ausbaustufe einen Strukturblock `Bergfried` ergaenzen, ueber den ein Spieler ein Koenigreich gruenden kann. Identitaet, Gruender, Position und Zustand autoritativ speichern und replizieren; eindeutige Gruendungsregeln und Rueckmeldung ueber vorhandene Interaktions-/UI-Systeme. Spaetere Gebietsrechte, Mitglieder und Ausbau darauf aufbauen, nicht als bereits fertige Funktionen darstellen.

### Mechanische Technik

- [ ] **F25 Kinetik-System:** Zahnraeder und Wellen aus Holz, Stein und Eisen, verbindbare Antriebe, Drehrichtung, Drehzahl und begrenzte Belastbarkeit. Bestehenden Handkurbel-/Crusher-Anschluss weiterverwenden; Wasserraeder an das vorhandene fliessende Wasser anbinden, damit sie die Handkurbel als automatischen Antrieb ersetzen. Fliessbaender fuer budgetierten Itemtransport ergaenzen. Antriebsnetze autoritativ, zyklensicher und chunkuebergreifend speichern; kein Entity pro Zahn und keine unbegrenzte Netzsuche pro Frame.
- [ ] **F26 Mixer:** Mischmaschine mit Zahnradanschluss und Kinetikantrieb wie beim Crusher, aufbauend auf F25. Mehrere Eingaben, gemeinsame Inventar-/Rezeptregistry, Too-Many-Items-Rezeptansicht, passende Animationen, Geraeusche und gespeicherter Fortschritt. Betrieb nur bei gueltigem Antrieb; kein zweites Technik- oder Inventarsystem.

### Wassertechnik

- [ ] **F27 Wasserrohre Und Ventile:** Verbindbare Rohre aus Holz, Stein und Eisen mit Ein-/Ausgaengen fuer Wasseranschluesse und absperrbaren Ventilen. Vorhandene Fluessigkeitszellen und Update-Budgets verwenden; Durchfluss, Speicherinhalt und Ventilzustand autoritativ und chunkuebergreifend persistieren. Keine zweite Wassersimulation und keine unbegrenzte Netzsuche pro Frame.
- [ ] **F28 Wasserhahn:** An einen Rohr-Ausgang anschliessbarer Hahn, der nur bei vorhandenem Wasser ausgibt und unter anderem Giessekannen auffuellt. Auf F27 und die vorhandene Nachfuellaktion aufbauen; kein Wasser aus dem Nichts, geschlossene Ventile unterbrechen die Versorgung.
- [ ] **F29 Ruhendes Wasser Und Quellenbildung:** Stabile Becken wirklich zur Ruhe kommen lassen. In einem gestuetzten 2x2-Becken sollen zwei geeignete Quellzellen die beiden verbleibenden Zellen in Quellen umwandeln. Quellenbildung, Mengenregeln, Chunk-Grenzen und Save/Reload testen; kein schwebendes Wasser, unkontrolliertes Wachstum oder dauerhaftes Remeshing stabiler Becken.

### Alchemie Und Temperatur

- [ ] **F30 Traenke Und Kessel:** Glasflaschen und gefuellte Trankflaschen, ein Kessel sowie Loeffel aus Holz, Stein und Gold. Datengetriebene Braurezepte auf dem bestehenden Effekt-/Inventarsystem aufbauen; Einzel- und Mischtraenke wie Regeneration + Staerke mit getrennten Staerken/Laufzeiten. Zutaten, Wasser, Erhitzen und Flaschen autoritativ verbrauchen, Kesselinhalt/Fortschritt speichern und passende Rezepte in Too Many Items zeigen. Keine zweite Effekt-Registry; Materialunterschiede der Loeffel explizit festlegen.
- [ ] **F31 Lagerfeuer:** Platzierbares, anzuendbares Feuer mit echtem Brennstoffverbrauch, dezenter Animation, Licht, Rauch und Geraeusch. Kessel aus F30 erhitzen, zum Waermen an F32 anbinden und vorhandene Lebensmittel-/Garrezepte zum Braten nutzen. Abstand, Brandschutz, Loeschen, Mehrspielerzugriff und gespeicherten Brennzustand beruecksichtigen; vorhandene Ofen-/Rauch-/Lichtbudgets verwenden.
- [ ] **F32 Kaelte Und Hitze:** Umgebungstemperatur aus Tageszeit, Jahreszeit, Oberflaechenbiom, Hoehe, Wasser/Nassheit und Schutz ermitteln: kalter Winter, kalte Wuestennaechte und heisser Sommer. Bestehendes Frieren, Resistenz, Kleidung und Lagerfeuerwaerme einbeziehen; Hitze als eigenen Status sauber ergaenzen. Wirkung langsam auf-/abbauen statt beim Ueberqueren einer Biomkante springen lassen; Werte, Schutz und passende HP-/Stamina-HUD-Anzeige autoritativ speichern/replizieren.

### Bestehende Erweiterungen Beibehalten

- [x] **F12 Bienen Und Honig:** Autoritative Honigentnahme mit Platzieren/Benutzen am echten Nest, Sicht-/Reichweiten-/Baurechte-/Inventarpruefung und unveraendertem Nest. Echte Nektarbesuche bestaeuben begrenzt geeignete feuchte Nutzpflanzen. Gesunde erwachsene Eltern unterschiedlichen Geschlechts erhalten durch Nektar finanzierten Nachwuchs ueber F06 mit Wachstum, Abklingzeiten, gespeicherten Familien und monotonen IDs. Bestehende Reifung, Nektar, Honig, lokale Moblimits und zehn Nestbewohner bleiben erhalten. [Umsetzung, Tests und Grenzen](tools/benchmarks/results/tree-ai-bees-water-2026-10-24/README.md).
- [x] **F13 Wassertransport (Eimer-Ausbaustufe):** Eisen-Eimer samt Werkbank-/Too-Many-Items-Rezept, leerer/gefuellter Inventar- und Handdarstellung sowie autoritativer Entnahme/Platzierung echter Quellen. Reichweite, Baurechte, gehaltenen Slot und Revisionen pruefen; keine Item-/Wasserduplikation. Quellen und explizit entnommenes Naturwasser ueber die bestehende Journal-/Region-Persistenz sichern. Pumpen sind ausdruecklich die spaetere Ausbaustufe zusammen mit F27; keine zweite Fluessigkeitssimulation. [Code, Tests und Grenzen](tools/benchmarks/results/cherry-water-transport-2026-10-25/README.md).
- [ ] **F14 Saisonales Wachstum:** Dirt->Grass und Tierverhalten an regionale Feuchtigkeit/Jahreszeiten anbinden; Ernten verwenden dies bereits seit F04. Kontrolliertes Gefrieren/Schmelzen von Eisblock 49 und dessen Rutschphysik; keine Zerstorung von Spielerbauten oder Wasserquellen.
- [ ] **F15 Weltobjekte:** Brand-/Lebenspunktschaeden fuer geeignete Objekte. Die seit Phase 112 anhand berechtigter Betrachter animierten Truhendeckel besitzen seit Phase 113 raeumliche Oeffnungs-/Schliessgeraeusche. Kosmetisches Feuer allein ist kein Schadenssystem.
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

Phase 128 korrigiert festgefahrene Bewohner, die Kantenpruefung der gemeinsamen
Navigation und ruckelnde Mob-Wiedergabe aus verspaeteten Netzwerk-Snapshots.
Konkrete weitere FPS-Vorschlaege (unveraenderte Preprocessing-Bindings cachen,
uebergrosse Uploads staffeln, entfernte Materialbatches buendeln, leere
Punktlicht-Schattenviews behandeln) mit Aufwand, Messgrundlage und Risiken:
[Naechste FPS-Schritte](tools/benchmarks/results/cherry-ai-light-2026-10-25/README.md#naechste-fps-schritte).
Phase 130 korrigiert zwei nachgewiesene Rueckschritte aus Phase 129:
Terrain-/Prop-Uploads werden wieder pro vollstaendiger Section budgetiert,
ohne Materialbatches kuenstlich zu zerteilen. Raeumliche Fernbatches bleiben
ein ausdruecklicher Smoke-Test-Versuch; im Spiel bleibt das feinere
Section-Culling erhalten. Veraltete LOD-Ergebnisse werden vor Mesh-Allokation
verworfen. Binding-Cache und die Korrektur leerer Schattenseiten bleiben
erhalten. Ein lokaler Bevy-0.19.1-Patch vermeidet zudem bytegleiche Uploads
CPU-eigener Mesh-Binlisten, ohne dynamische Renderdaten einzufrieren.
Native Vergleiche: Nacht ca. 62 -> 73 FPS; 4K-Regen ca. 42 -> 57 FPS;
Nacht-Ladezeit ca. 42 -> 24 s. Einzelne 4K-Ausreisser bleiben offen.
Unveraenderte Qualitaet/Sichtweite, drei Seeds, keine neuen Langzeittests.
Vergleichsmessungen stehen im
[Regressionsbericht](tools/benchmarks/results/streaming-regression-2026-10-25/README.md).
Der [Phase-129-Bericht](tools/benchmarks/results/cherry-water-transport-2026-10-25/README.md)
ist der historische Zwischenstand, keine Performance-Abnahme. A05 bleibt offen.

Phase 131 ergaenzt `FPS Min` / `FPS Max` im F1-Overlay: ungeglaettete
Einzelframe-Werte ab der spielbaren Welt, auch bei verborgenem Overlay und
offenem Inventar. Reset beim Weltwechsel; der initiale Ladebildschirm zaehlt
nicht dazu. Messung verwendet reale Zeit, damit auch Hanger ueber 250 ms
nicht durch das Simulationszeit-Limit verdeckt werden.
Kurzer nativer Regen-Flug (Seed 7, Sichtweite 12, 1280x800, 24 s):
97,00 FPS im Mittel, p95 15,61 ms, p99 18,13 ms, schlechtester Frame
27,80 ms (= 35,97 FPS), initiales Laden 8,68 s. Rohdaten:
[Flugmessung](tools/benchmarks/results/session-fps-2026-10-26/flight-before.log).
Wiederholung mit FPS-Erfassung: 97,49 FPS, p95 15,41 ms, p99 17,87 ms,
Minimum 35,73 FPS / Maximum 283,44 FPS im 24-s-Messfenster;
[Rohdaten](tools/benchmarks/results/session-fps-2026-10-26/flight-after.log).
Diese Schwankung belegt keinen Performance-Gewinn. Im integrierten nativen
HUD-Test sind die Session-Werte separat vom kurzen Benchmarkfenster geprueft:
[Screenshot](tools/benchmarks/results/session-fps-2026-10-26/overlay.png),
[Log](tools/benchmarks/results/session-fps-2026-10-26/overlay.log).
Validiert: 351 UI-Tests, Client-Build, Clippy fuer UI/Client mit allen Targets
und `-D warnings`, Workspace-Formatcheck. Keine Grafikqualitaet reduziert.
Keine neue Performance-Korrektur aus dieser Messung abgeleitet; die
verbleibenden Low-FPS-Ausreisser sind weiterhin offen. Keine neuen Langzeittests.

Phase 132 setzt die freigegebenen Rendering-Schritte um: gemeinsame GPU-Submits
fuer Slab-Wachstum, konservative asynchrone Hoehlen-Portale vor Komposition /
Upload und beidseitige Schatten-Proxys ohne doppelte Indexgeometrie. Collider
und Physikradien bleiben auf ausdruecklichen Wunsch unveraendert.
Eine 32-MiB-Slab-Grenze wurde wegen gemessener Verschlechterung verworfen.
Kurze native Vergleiche zeigen keinen allgemeinen FPS-Gewinn; insbesondere
einzelne Streaming-Spitzen und A05 bleiben offen. Details, Einschraenkungen,
Zwischenversuche und unverkuerzte Rohdaten:
[Rendering-/Culling-Bericht](tools/benchmarks/results/render-culling-2026-10-26/README.md).

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

Phase 116 korrigiert den gemessenen grossen Terrain-Arbeitsvorrat nach dem
Fliegen: einmalige Sortierschluessel je Kandidat und gezielte lokale
Exposure-Invalidierung. Im nativen 120-s-Hin-/Rueckflug bei 4K, Regen und
Sichtweite 20: 39,18 -> 71,33 FPS, p95 43,87 -> 25,16 ms. Sichtweite 12,
1080p/Clear bleibt bei 123,61 -> 121,73 FPS ohne behaupteten FPS-Gewinn.
Keine Grafikqualitaet wurde reduziert; mehr Terrain wird rechtzeitig sichtbar.
Das ist keine Langzeitabnahme und schliesst A05 nicht. Rohdaten und Grenzen:
[Streaming-/Stalker-Bericht](tools/benchmarks/results/stalker-climbing-streaming-2026-10-24/README.md).

Phase 119 begrenzt die Prop-LOD-Auswahl und vermeidet leere globale Durchlaeufe.
Im nativen 60-s-Flug bei 1080p, Regen und Sichtweite 20: 86,28 -> 93,45 FPS,
p95 20,80 -> 17,32 ms, Prop-Dispatch-Aufrufe ueber 8 ms waehrend der Route
147 -> 0. Alle 102730 Props bleiben dargestellt. Einzelne Frameausreisser
bleiben vorhanden; A05 ist damit nicht geschlossen. Messbedingungen und Rohdaten:
[Mob-/Streamingbericht](tools/benchmarks/results/mob-movement-streaming-2026-10-24/README.md).

Phase 125 korrigiert wiederholte globale GPU-Schattenvorbereitung und den
Executor-Overhead je Cubemap-Seite. Im kurzen nativen Dorf-/Regenvergleich:
1280x800/Sichtweite 12: 37,91 -> 56,88 FPS;
3840x2160/Sichtweite 20/Ultra-Wetter: 33,82 -> 47,48 FPS.
Alle Punktlicht-Schattenseiten bleiben erhalten; keine reduzierte Sichtweite
oder Grafikqualitaet. Der kurze Waldflug verbessert sich nur 66,06 -> 70,06 FPS.
Verbleibende Frame-Ausreisser und A05 bleiben offen. Messpfade und Grenzen:
[Baum-/Struktur-/Performancebericht](tools/benchmarks/results/tree-structures-performance-2026-10-25/README.md).

Phase 126 untersucht verbleibende Kosten ohne weitere Performance-Eingriffe:
Im nativen Dorf-/Regenblick 57,03 FPS, Render-Wall 16,79 ms gegen Main 9,00 ms;
beim kurzen Hin-/Rueckflug steigt Render-Asset-Vorbereitung von stationaer
0,50 auf 4,08 ms pro Frame. Bind-Groups kosten weiter etwa 1,5 ms.
Priorisierte Befunde, Rohdaten und Abgrenzung noch unbewiesener Ursachen:
[Bewohner-/Rohr-/Physikbericht](tools/benchmarks/results/villagers-pipes-physics-2026-10-25/README.md).
Keine Langzeitabnahme, kein behaupteter weiterer FPS-Fix; A05 bleibt offen.

Phase 127 behebt die danach nachgewiesenen per-face Encoder-Abschluesse bei
Punktlichtschatten: Unpacking/Preprocessing phasenweise buendeln und die
vollstaendige Tiefenzeichnung im bestehenden gemeinsamen Bevy-Pass ausfuehren.
Mit den neuen NPC-Modellen auf beiden Seiten: Dorf nachts, 1280x800/Sichtweite
12, 50,12 -> 60,86 FPS; Dorf/Regen, 4K/Sichtweite 20/Ultra-Wetter,
46,22 -> 52,47 FPS. Keine reduzierte Sichtweite oder Schattenqualitaet;
GPU-Readbacks bestaetigen erhaltene Schatten-Draws. Der Seed-7-Wasser-/Regenflug
bleibt bei etwa 101 FPS, ohne behaupteten Schattengewinn in einer Szene ohne
Punktlichtschatten. Grosse Uploads, einzelne 4K-Spitzen und A05 bleiben offen:
[Messungen, Rohdaten und Umsetzung](tools/benchmarks/results/npc-detail-performance-2026-10-25/README.md).

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
