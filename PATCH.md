# TERAMIC: Abgeschlossene Arbeiten

Kompaktes Archiv der bisherigen Umsetzung. Offene Arbeiten stehen ausschliesslich
in [TODO.md](TODO.md), Architektur und Startbefehle in [README.md](README.md).
Die Phasennummern bleiben als Referenz fuer bestehende technische Berichte erhalten.
Historische Messungen und Screenshots belegen nur den jeweils genannten Stand.

## 1. Baumgenerator

Deterministische, abbaubare Voxelbaeume mit verjuengten Staemmen, Wurzeln,
verbundenen Aesten und unregelmaessigen Kronen. Gruene, herbstliche und Birkenformen
bleiben erhalten. Holz-/Laubtexturen und Groessenvarianten wurden abgestimmt;
Generatorversion 5 und Material-IDs 8 bis 10 eingefuehrt und getestet.

## 2. Landschaft Und Biome

Taeler, Terrassen, Felswaende und Bodenschichten mit weich gemischten Ocean-,
Plains-, Forest-, Desert- und Highlandprofilen. Globale Klima-/Kontinentmasken,
Waldlichtungen und terrainabhaengiges vertikales Laden statt sichtbarer Leerraeume.
Determinismus, negative Koordinaten, Edit-Replay und native Referenzregionen geprueft.

## 3. Wasser Und Ufer

Separate Wasseroberflaechen mit Sand-/Steinufern, Transparenz, Brechung,
Tiefenabsorption, Wellen, Schaum und atmosphaerischen Fresnel-/PBR-Reflexen.
Gemeinsame Ecken, Ufer-Edits und Reload getestet. Diese erste Fassung war statisch;
spaetere Fluesse, Schwimmen und dynamisches Zellwasser sind unten dokumentiert.

## 4. Bewuchs, LOD Und Streaming

Gras, Blumen, Farne und Buesche in deterministischen Gruppen und Lichtungen.
Drei LODs, Section-/Material-Batches, begrenzte Caches und vertikale Worker-Batches
ohne Neugenerierung beim LOD-Wechsel. Historische CPU/ECS-Erstgenerierung von
4375 Sections: 18.30 -> 2.63 s, ohne Renderer/GPU-Uploads; Terrainversionen
unterschieden sich ebenfalls, kein daraus abgeleitetes FPS-Versprechen.

## 5. Rendering Und Eigenstaendige Props

Grass-zu-Dirt-LOD-Fehler durch explizite Face-Materialien statt GLB-Primitive-Reihenfolge
behoben. Terrain-LOD 96/160 m mit 12 m Hysterese, priorisierte Worker und geteilte
Cube-/Greedy-Geometrie. Normale Cube-Bloecke laden keine GLBs; Custom-Meshes bleiben
moeglich. Gemeinsame PNG-/Material-Handles, wiederholende UVs und asynchrone Mips
statt zusaetzlichem Atlas-/Array-Renderer.

Bestehende Vegetation um stabile Prop-IDs, Block-/Prop-Targeting, Einzelabbau,
Journal-Persistenz und eigene Prop-Epochen erweitert. Micro-Geometrie und
Art-/Varianten-/LOD-Vorlagen in Section-Batches, keine Entity pro Halm.
Wasserqualitaet, Licht und native FPS-/Speicherdiagnostik angebunden.
Historischer Abschluss: 116 Tests und native/headless Pruefungen.
Materialvertrag: [Block Library](assets/terra/blocks/README.md).

## 6. Referenzbild-Abgleich

Getrennter Prop-LOD 80/144 m mit erhaltenen Fernsilhouetten, verbesserte
Bewuchsmasken und Micro-Kronen. Statische hierarchische GLTF-Szenen, echte SSR
fuer Medium/High, HDR-/Atmosphaerenabstimmung und optionale GPU-Passdiagnostik.
Hochgeladene Meshes geben CPU-Vertexdaten frei. Damaliges Dithering wurde in Phase 7
wieder entfernt. 136 Tests und native Vergleiche; keine vollstaendige Referenzqualitaet.
[Rohprotokolle](tools/benchmarks/results/reference-2026-10-14/).

## 7. Geschlossene Kanten Und Flimmerkorrektur

Verdeckte Bevel-Kanten auf kanonische Zellgrenzen gesetzt und konkave
Drei-/Vier-Zellen-Verbindungen getestet. Inkonsistentes Dithering im Depth-only-
Prepass durch atomare Mesh-Ersetzung ohne Doppelbelegung ersetzt. Materialien
und Hysterese bleiben stabil; geometrische LOD-Spruenge koennen sichtbar bleiben.
136 Tests, nativer 48-s-Flug, Block-/Prop-/Wasserpruefung und Headless-Start.
[Rohprotokolle](tools/benchmarks/results/seam-fix-2026-10-14/).

## 8. Controller, Einstellungen Und Asset-Bereinigung

Avian-Laufcontroller mit Sprung, Bodenkontakt, Kollision und begrenzten asynchronen
Terrain-Collidern; Warten auf fehlende Kollisionsdaten. TiltUI-Einstellungen mit
Vorschau, Apply/Cancel/Reset und atomarer Konfigurationsspeicherung. Neutrale
Grastextur mit Biom-/Wetterfarbe, lokale Feuchte/Verkohlung im bestehenden Journal.
Zehn unbenutzte Standard-GLBs entfernt: 15875208 Bytes weniger Assets, kein neuer
Draw-Call-Gewinn. 171 Tests und native Physik-/Settings-Pruefungen.
[Bericht](tools/benchmarks/results/player-settings-2026-10-14/README.md).

## 9. Pflanzen, Blockhuellen Und HUD

Gras-/Blumenformen und Farbvarianten verbessert; neue Pflanzen nach Freilegen
urspruenglicher Baumzellen verhindert. Zerstoerte Props bleiben entfernt.
Fehlerhaft offenes/hochgezogenes Relief zurueckgenommen, geschlossene Blockhuellen
getestet. Kraeftigere Wasserfarbe sowie TiltUI-HUD mit FPS, App-CPU/RAM, Chunk,
XYZ und echtem Zielnamen. 177 Tests und native Ansichten; CPU/RAM nativ auf Linux,
sonst ohne Backend als nicht verfuegbar. Relief danach in Phase 10 weiterentwickelt.
[Bericht](tools/benchmarks/results/voxel-detail-2026-10-14/README.md).

## 10. Geschlossene Mikrostruktur

Optionale JSON-Oberflaechenprofile und begrenztes, ausschliesslich nach innen
gearbeitetes Near-Relief mit unveraenderten Anschlussraendern. Teildegenerierte
Quads korrekt trianguliert; Ray-, Index-, Winding- und Materialtests ergaenzt.
181 Tests und native Nah-/Abbau-/LOD-Pruefungen; belasteter erster GPU-Vergleich
nicht als gueltige Performancebaseline verwendet.
[Bericht](tools/benchmarks/results/surface-detail-2026-10-14/README.md).

## 11. Mikrorelief-Performance

Fugenwaende greedy verbunden und exakt gleiche Template-Vertices indiziert,
ohne Sichtweite, Relief oder Materialien pauschal zu reduzieren. Isolierter
48-s-Vergleich: 68.800 -> 93.504 FPS, p95 18.231 -> 12.855 ms,
p99 24.560 -> 17.144 ms. 183 Tests und native Geometriepruefung.
[Bericht](tools/benchmarks/results/surface-performance-2026-10-14/README.md).

## 12. Gemeinsamer Integrated-/Dedicated-Server

Singleplayer nutzt denselben Headless-Bootstrap, Fixed-Takt, Controller und
autoritativen Edit-Pfad wie Dedicated, ueber echten Loopback-UDP. Eingaben,
Prediction/Reconciliation, interpolierte Mitspieler, mehrere Streamingzentren,
validierte Edits und Section-Deltas mit Late Join/Reconnect integriert.
197 Tests, zwei echte native Clients und headless Dedicated pruefen den Pfad;
damalige Join-Dateien wurden in Phase 14 abgeloest.
[Bericht](tools/benchmarks/results/multiplayer-2026-10-15/README.md).

## 13. TERAMIC-Menues

TiltUI Main Menu, reale Welten-/Serverlisten, Graphics/Controls, Pause und
Ladeansicht im gemeinsamen Stil. Persistente TOML-Optionen, echtes Rebinding,
Verwerfungsabfrage und 15-s-Display-Rollback. WorldManifest/Create/Delete,
Integrated-Journal-Save/Reload und asynchrone Session-Lebenszyklen.
212 Tests sowie native Ansichten, Aktionen, Pointer-Hit-Test und produktives Quit.
Die damalige Vegetationsdichte-Option wurde in Phase 16 entfernt.
[Bericht](tools/benchmarks/results/menu-2026-10-15/README.md).

## 14. UUID, LAN, Game Menu Und Loading Screen

Persistente lokale UUID-/Namensprofile, automatischer begrenzter Join-Handshake,
echter Serverstatus/Ping und IPv4-LAN-/Loopback-Discovery ohne manuell verteilte
private Zugangsschluessel. Transportverschluesselung bleibt erhalten; lokale
UUIDs sind keine verifizierten Accounts. Game Menu, Save/Resume/Main Menu und
echter Streaming-/Mesh-Ladefortschritt. ChoiceBox-Kontrast/Textlayouts korrigiert.
224 Tests, zwei UUID-Clients, native Menues und Headless-Featurepruefung.
[Bericht](tools/benchmarks/results/lan-menus-2026-10-15/README.md).

## 15. Performance, Weltzustaende Und Rechte

Separater Mikrorelief-Radius mit Hysterese, weniger ruhende Queue-/Scanarbeit,
lokale Collider-Suche und keine laufenden versteckten Menue-Rebuilds. Gemeinsames
Schwimmen/Auto-Climb, rebindbare Third-Person-Kamera und Spielermodell.
Dedicated-Saves mit Dateisperre, Autosave und Shutdown-Save; UUID-Baurechte.
Gebatchter Niederschlag, sparse Schneedecke und begrenztes autoritatives Feuer
im bestehenden Journal. 234 regulaere Tests, separater 180-s-UDP-Soak,
native A/B-Messungen und CPU-Streaming weiterhin 2.626 s ohne GPU.
Die damaligen zugespitzten Grashalme sind kein unveraenderliches Stilziel.
[Bericht](tools/benchmarks/results/performance-gameplay-2026-10-15/README.md).

## 16. Fluesse, Faelle Und Bewegungsgefuehl

Deterministische Fluesse ueber den gespeicherten opt-in Parameter `rivers`,
variable Wasserhoehen und vertikal geclippte Faelle. Stroemungsanimation,
Schaum und Unterwasserumgebung. Lauf-/Sprinttempo 5.4/8.2 m/s, kuerzerer Sprung,
Kamerainterpolation, korrigiertes Tauchen/Auftauchen und Uferausstieg.
Logische Props durch Grafikoptionen nicht mehr versteckbar. 240 Tests,
native Wasser-/Physik-/UI-Pruefungen und sechs isolierte Messlaeufe.
[Bericht](tools/benchmarks/results/hydrology-movement-2026-10-15/README.md).

## 17. Flussuebergaenge Und Auto-Climb-Option

Kleine Gefaelle kontinuierlich, senkrechte Wasserfaelle erst ab 5 m; gemeinsame
Wasserecken, breitere eingeschnittene Ufer und variable Becken. Auto-Climb
in Controls, Default aus, mit persistenter Einstellung und Netzwerk-/Prediction-
Integration. `config/` nach `configs/` ueberfuehrt, Werte erhalten.
248 Tests und native Mess-/Interaktionspruefungen; niedrigere Wald-FPS offen ausgewiesen.
[Bericht](tools/benchmarks/results/river-transitions-2026-10-15/README.md).

## 18. Wasseranschluesse, Animation Und Audio

Seitliche Uferhoehen begrenzen Pegel und schwebende Flussflaechen; gemeinsame
Eckhoehen und trockene Randzellen schliessen Anschluesse. Gekruemmte Fallschichten,
stabiler Noise, staerkere Wellen/Rippel sowie Kamm-/Aufprallschaum.
Drei raeumlich gemischte Wasserstimmen mit Spielerkoerper als Hoerposition,
echtem Unterwasser-Tiefpass und Session-Cleanup. 258 Tests, native Vulkan-/Audio-
Pruefungen und zwoelf Vorher-/Nachher-Laeufe; menschliche Mixabnahme separat offen.
[Bericht](tools/benchmarks/results/water-audio-2026-10-15/README.md).

## 19. Flusstunnel Und Audio-Optionen

Gewoelbte Dichte-Hohlraeume, wasserfuehrende Galerien und Endkammern statt
dekorativer Blindportale. Katarakte von 30 bis 44 m mit offenem Schacht,
Defaultseed-Fall 31.5856 m. TiltUI-Audio-Tab und `configs/audio.toml` mit
Apply/Reset/Discard, sanfterem Flussrauschen und budgetierter Felsabschirmung.
270 Tests, native Tunnel-/Audio-/Menuepruefungen und zwoelf Messlaeufe.
Endgalerien bleiben auf 40 m begrenzt.
[Bericht](tools/benchmarks/results/river-tunnels-audio-2026-10-15/README.md).

## 20. Flussvielfalt, Windlaub Und Dynamisches Zellwasser

Asymmetrische erodierte Portale, auslaufende Kataraktdecken, variable Bettiefen/
Breiten und gelegentliche Seitenarme mit trockenen Inseln. Gemeinsame Alpha-Mask-
Laubmaterialien, synchroner Wind in Haupt-/Tiefen-/Schattenpfad und maximal
48 fallende Blaetter in einem Batch. Kontaktmetadaten schliessen Laub-/Stammfugen;
Backface-Culling reduziert Kronenarbeit. Blattfall ist kosmetisch, kein Laubzerfall.

Wasser fliesst nach Uferabbau zellbasiert mit Achtelfuellstaenden und Stroemung.
Bestehendes EditJournal/SectionDelta traegt Zustand, Save/Reload und echte
Zwei-Client-/Late-Join-Replikation. Separate begrenzte Wasserjobs/Epochen vermeiden
Terrain-/Collider-Rebuilds; Schwimmen, Nebel und Audio verwenden die lebenden Pegel.
Solver: maximal 256 aktive Zellen pro Schritt bei 10 Hz, natuerliche Reservoirs
unbegrenzt, austretendes Wasser endlich; ungeladene Gebiete pausieren. Keine
Drucksimulation oder Erosion, kein zusaetzliches Terrainmaterial oder GLB.

302 Tests bestanden, ein Langzeit-Netzwerktest im normalen Lauf ignoriert;
fmt/check/test/Clippy/build, separater Headless-Build/-Start und native
Portal-/Gabelungs-/Laub-/Ausfluss-/Audio-/Physikpruefungen bestanden.
Zwoelf isolierte Vorher-/Nachher-Laeufe: Waldmittel 114.20 -> 105.10 FPS,
fester Flussblick 142.43 -> 137.43 FPS. Diese Regression bleibt offen;
CPU-Kaltstart 4375 Sections weiter 2.626 s, GPU nicht enthalten.
[Bericht und Rohdaten](tools/benchmarks/results/living-water-foliage-2026-10-15/README.md).

## 21. Verbundene Tunneldecken Und Kantiger Bewuchs

Spaltenweise Uferanbindung verhindert isolierte Dachreste aus ueberlagerten
Fluss-/Gelaendeformen. Zusaetzlicher Hoehlen-Noise trennt die Unterseite dieser
Daecher nicht mehr ab; angehaengte Ueberhaenge bleiben erhalten. Elf Regionen
mit Hoehlen an/aus werden gegen echte Boden-/Uferanker geflutet, einschliesslich
zweier zuvor fehlschlagender Positionen. Spielerbauten und Edits bleiben erhalten.

Schlanke Grashalme enden jetzt flach statt nadelfoermig, ohne mehr Vertices.
Drei Bluetenformen mit versetzten Seitenblaettern, Petalen und erhoehten Zentren
ergaenzen die bisherigen Farben. Stabile Prop-IDs, Picking und Section-Batching
bleiben erhalten; entfernte LOD-Stufen werden nicht teurer.

306 Tests, fmt/check/test/Clippy/build, separater Headless-Build/-Start sowie
native Gras-/Blumen-/Tunnel-/Wasserfallansichten bestanden. Die botanische
Pruefung enthaelt eine zusaetzliche Nahaufnahme. Zwoelf isolierte Vergleichslaeufe:
Waldmittel 109.91 -> 110.75 FPS, fester Flussblick 143.80 -> 145.56 FPS; kein
zusaetzlicher FPS-Verlust in diesen kurzen Laeufen, kein allgemeines Speedup-Versprechen.
Die frueheren Windlaubkosten bleiben eine eigene offene Aufgabe. TODO enthaelt nur noch offene
Arbeiten; bisherige Ergebnisse sind in diesem Archiv ohne Checkboxen gesammelt.
[Pruefungen, Aufnahmen und Messungen](tools/benchmarks/results/tunnels-botany-2026-10-15/README.md).

## 22. Abbau, Inventar Und Windbewuchs

Creative baut beim ersten Input ohne Haltezeit ab. Survival verwendet serverseitig
gepruefte Haltezeiten mit Abbruch bei Loslassen, Zielwechsel oder Menue; direkte
Loeschkommandos umgehen diese Pruefung nicht. Sechs Rissstufen, Hand-/Bauanimation,
maximal 48 Bruchpartikel und eine gueltige/ungueltige Bauvorschau ergaenzt.
FOV-abhaengige Handdarstellung ohne zusaetzlichen Renderpass.

Ressourcen aus gezielt abgebauten Bloecken und Props werden bei freier Sicht
aufgesammelt. UUID-gebundenes Inventar, lose Drops und Verbrauch nach bestaetigter
Platzierung nutzen das bestehende EditJournal und Save-System. Alte Saves bleiben
lesbar. TiltUI-Inventar mit E, Materialauswahl, Mengen und sicherer Eingabesperre;
Creative besitzt unbegrenzte Baumaterialien ohne Ressourcenverbrauch oder Drops.

Radiale, stumpfe Grasbueschel und unregelmaessige Blattlagen an Bueschen besitzen
sanften, an den Wurzeln fixierten GPU-Wind. Bestehende Section-Batches, Prop-IDs
und Vertexbudgets bleiben erhalten; keine Entity pro Pflanze oder Mikro-Voxel.
Vorherige Grafikdichte-Nachrichten koennen serverseitiges Prop-Picking nicht
mehr umgehen.

348 Tests, fmt/check/test/Clippy/build, separater Headless-Build/-Start und native
Survival-/Creative-/Inventar-/45-Grad-FOV-/Windpruefungen bestanden. Waldmittel
108.836 -> 109.029 FPS; Spawn ebenfalls nahezu unveraendert. Die Flusslaeufe sind
wegen einer waehrenddessen gestarteten weiteren 3D-Anwendung nicht abschliessend
vergleichbar; saemtliche Werte und der langsamere Wiederholungslauf bleiben im
Bericht. Keine allgemeine Performanceparitaet behauptet. Crafting, Drop-Physik
und weitergehende Mehrspieleranimationen sind nicht Teil dieses Minimalinventars.
Der bisherige TODO-Punkt 1 ist hier archiviert; die offenen Punkte sind neu nummeriert.
[Nachweise, Aufnahmen und Messungen](tools/benchmarks/results/building-inventory-wind-2026-10-15/README.md).

## 23. Sicherer Festland-Spawn

Integrated und Dedicated Server suchen denselben deterministischen Weltspawn
auf trockenem, tragendem Terrain statt den festen Startpunkt nur ueber den
Meeresspiegel anzuheben. Eine kleine trockene Umgebung, zwei feste Bodenlagen
und drei freie Zellen ueber dem Zentrum verhindern Meeres-, Baumkronen- und
isolierte Kantenstarts. Gespeicherte Block-/Fluid-Aenderungen werden schon beim
Serverstart beruecksichtigt; jeder Beitritt prueft den Spawn erneut.

Die vorhandene Welcome-Nachricht traegt die sichere Koerperposition. Ready und
Spieler-Snapshots warten auf geladene, gepruefte Standzellen. Bei zwischenzeitlich
veraendertem Boden wird der Beitritt abgelehnt; ein erneuter Beitritt sucht einen
geeigneten Ersatz. Eine begrenzte Ringsuche ohne Ergebnis meldet einen Fehler,
statt Wasser- oder Luftspawns zuzulassen oder kuenstliches Terrain zu erzeugen.
Keine Aenderung an Seeds, Generator, Weltformat oder Netzwerkprotokoll.

364 Tests bestanden, ein vorhandener Langzeit-Netzwerktest regulaer ignoriert.
16 neue Spawn-Regressionen decken Meer-Seeds, Fluesse, negative Koordinaten,
gespeicherte Block-/Fluid-Aenderungen, Kopffreiheit und echte UDP-Beitritte ab.
fmt/check/test/Clippy/build, separater Server-Build und Headless-Start ohne
DISPLAY/WAYLAND_DISPLAY bestanden. Keine neue visuelle GUI-Pruefung oder
FPS-Messung fuer diese reine Spawn-/Handshake-Aenderung durchgefuehrt.

## 24. Bloecke, Materialansichten Und Bruchdarstellung

Die bestehende Palette umfasst 19 Bloecke. IDs 1..10 bleiben unveraendert;
Sandstein, glatter Stein, Mossy Stone, fester Wasserblock, gerahmtes Glas,
Oak-/Birch-/Spruce-Planks und Lehm ergaenzen sie als IDs 11..19. Sand bekommt
feines Korn statt Steintextur. Sieben deterministische, gekachelte Texturmuster
mit Mips teilen sich die vorhandene Material-/Section-Pipeline. Normale Bloecke
benoetigen keine GLBs; statische Spezialmodelle bleiben unterstuetzt.

Sandstein liegt unter Wuestensand, Lehm stellenweise unter nassem Sand und Moos
vereinzelt an feuchten Hoehleneingaengen. Glatter Stein und Bauteile werden nicht
natuerlich generiert. Glas ist pro Flaeche 90 Prozent durchsichtig und verbindet
seine Holzrahmen auch ueber Section-Grenzen. Der feste Wasser-Cube nutzt die
Wasseranimation, kollidiert und bleibt von Fluid-/Schwimmwasser getrennt.
Physik, Mining, Inventar, Save/Reload und autoritative Replikation kennen alle
neuen Materialien; generische Custom-IDs im EditJournal bleiben erhalten.

Debug startet ausgeschaltet. F1 schaltet das Overlay, F1+B die ausschliesslich
im Debug sichtbare Platzierungsbox, ohne gleichzeitig die B-Bauaktion auszuloesen.
Menues und Inventar verdecken das Overlay. Echte einmalig gerenderte Blockbilder
ersetzen Blocksymbole im TiltUI-Inventar und Material-HUD; die temporaere
Vorschaukamera wird danach entfernt. Die Spielerhand ist vollstaendig entfernt.

Zwoelf zunehmende, unregelmaessige Bruchstufen und kleine Materialsplitter ersetzen
das alte Rissgitter. Leere Fruehstufen werden nicht als GPU-Mesh hochgeladen.
Verdeckte Flaechen bleiben aus; Abbruch stellt den unbeschaedigten Block dar.
Der echte Block bleibt bis zur Serverbestaetigung intakt, Creative baut sofort ab.
Crafting fuer reine Baumaterialien bleibt offen.

394 Tests bestanden, ein vorhandener Langzeit-Netzwerktest regulaer ignoriert.
fmt/check/test/Clippy/build, separater Headless-Build/-Start und native Material-,
Glas-, Survival-/Creative-, Inventar- und Custom-Mesh-Pruefungen bestanden.
Die finalen nativen Logs enthalten keine ERROR/WARN-Meldungen.

CPU-Kaltstart weiterhin 4375 Sections in 2.626 s ohne GPU-Uploads. Ein aktueller
18-Sekunden-Waldlauf misst 109.056 FPS, p95/p99 10.685/14.115 ms; ohne aktuellen
Vorherlauf ist das kein Paritaets- oder Verbesserungsnachweis.
[Pruefbericht, Rohdaten und Aufnahmen](tools/benchmarks/results/materials-debug-inventory-2026-10-15/README.md).

## 25. Hoehlen, Oasen Und Mushroom Park

Begehbare, hangorientierte Hoehleneingaenge fuehren ueber variable Gaenge in
groessere Kammern und das bestehende Hoehlenfeld. Tragende Boeden und verbundene
Felsdaecher werden geprueft; nahe Abstiege laden bis zu 18 weitere Sections
vorausschauend, ohne alle tiefen Weltbereiche aktiv zu halten.

Wuesten erhalten Sand ueber Sandstein und Stein, tote Buesche sowie seltene,
geschlossene Oasen mit eigenem Wasserstand. Palmen mit gebogenen Wedeln und
trockenes Gras wachsen an geeigneten Ufern. Fluesse werden in Wuesten als ganze
Laeufe seltener ausgewaehlt, nicht am Biomrand abgeschnitten.

Mushroom Park ist ein sechstes weich gemischtes Biom mit grossen Pilzen und
farblich gruppiertem Unterwuchs. Der weiss-braeunliche Stamm (20) und sechs
Kappen in Lila, Rot, Gelb, Gruen, Blau und Weiss (21..26) verwenden normale
Cube-JSONs, gemeinsame Materialien und Lamellenunterseiten, keine GLBs.
Alle neuen Bloecke funktionieren mit Abbau, Inventar, Speicherung und Netzwerk.
Fichten, Baumartengruppen, Farn-Unterwuchs, Blumenwiesen und variierende tiefe
Ozeanboeden ergaenzen die bisherigen Biome; Highland bleibt erhalten.

Isolierte Seen sind umgesetzt. Die Pruefung allgemeiner unterirdischer Aquifere
und der Entwurf fuer Ganzbaumfaellen/Laubzerfall stehen in README; deren
weitergehende Implementierung bleibt ausdruecklich in TODO offen.
419 Tests, fmt/check/test/Clippy/build und separater Headless-Build/-Start
bestanden. Native Oasen-, Hoehlen-, Pilzpark- und 26-Block-Materialansichten
geprueft. Echte GPU-Inventarbilder warten jetzt zeitbegrenzt auf Shaderbereitschaft
statt nach wenigen schnellen Frames vorzeitig als leer zu gelten.

CPU-Kaltstart bleibt bei 4375 Sections in 2.626 s. Entfernter Regionswechsel:
5273/4.934 s -> 5303/4.968 s durch zusaetzliche Hoehlen-Sections.
Fester Flussblick 145.49 -> 144.07 FPS, p95/p99 8.519/10.491 -> 8.408/10.538 ms.
Der fruehere Waldblick ist jetzt Pilzpark (110.02 -> 168.14 FPS), kein isolierter
Renderer-Speedup-Nachweis. Rohdaten und genaue Einschraenkungen stehen im Bericht.
[Technik, Pruefungen, native Aufnahmen und Messungen](tools/benchmarks/results/caves-biomes-2026-10-15/README.md).

## 26. Hochland, Kammern, Pflanzen Und Chat

Highland beginnt bei Y80 mit angepasstem Hoehenprofil. Schnee ist ein voller
Block; gruene Haenge behalten Dirt unter Grass und zeigen Fels erst bei
entsprechendem Hoehenabstand. Hoehlen erhalten unterschiedliche Gaenge und
grosse Kammern. Fichten wachsen etwa 40 Prozent hoeher mit einspaltigen Staemmen
und bedeckten Holzspitzen. Pilze erhalten variierende Kappen, Lamellen und
budgetierte Nahbereichsdetails. Der u64-Maskentyp im Pilzmesher ist explizit.

TiltUI-Chat mit Verlauf und `/locate biome` nutzt echte Serverantworten,
asynchrone Suche und einmalige, berechtigungsgepruefte Teleporttickets.
Biomname/Koordinaten sind rosa, die Teleportaktion gelb. Texteingabe blockiert
Gameplay und Escape schliesst nur den Chat. Die Connected-/Lademeldung im
Gameplay-HUD wurde entfernt.

443 Tests bestanden, ein vorhandener Langzeittest ignoriert. Alle fuenf
Cargo-Pruefungen sowie separater Headless-Build/-Start bestanden. Native
Chat-/Teleport-, Hoehlen-, Pilz- und Hochlandansichten wurden geprueft.
CPU-Kaltstart: 4375 Sections in 2.626 s, ohne GPU-Uploads. Der feste alte
Fluss-Kamerapunkt liegt nach der Terrainkorrektur im Berg; sein Nachherlauf
ist deshalb als Performancevergleich verworfen, nicht als Verbesserung gemeldet.
[Bericht, Rohdaten und Aufnahmen](tools/benchmarks/results/terrain-chat-2026-10-16/README.md).

## 27. Inventar, Waldlaeufer Und Bedienung

Alle bestehenden Props werden zentral 30 Prozent groesser dargestellt;
gespeicherte Positionen, Varianten und IDs bleiben unveraendert. Wind,
Pickinggrenzen und Section-Batches verwenden dieselbe visuelle Skalierung.
Der Blatt-Waldlaeufer erhaelt Kapuze, Gesicht, Blattkleidung, Lederdetails und
Laufbewegung. Lokale/entfernte Avatare und die echte Inventarvorschau teilen
sechs Koerperteilmeshes statt Entities pro Detailvoxel.

TiltUI-Inventar nach dem Layout der Referenzen, ohne deren Bildhintergruende:
Spielervorschau, Creative-Katalog mit Suche/Kategorien, Survival-Slots,
persoenliches 2x2-Crafting, Hotbar und Itemdetails. Echte Block-/Propbilder,
Mengen oben rechts, Standardstapel 128 und Hover mit Name, `teramic::`-Kennung
und Beschreibung. Tooltip-Layout und Clipping innerhalb der Slotkomponente
sind korrigiert. Keine vorgetaeuschten Ausruestungs- oder Gesundheitswerte.

Stabile Inventarslots, Merge/Swap, drei einfache Rezepte und Creative-Befuellung
laufen ueber dieselbe Serverautoritaet in beiden Spielmodi. Revisionen und
konkrete Slotquellen schuetzen Bestand und Platzierung. Geerntete Pflanzen
lassen sich wieder einsetzen; Platzierungen bleiben im bestehenden Journal
nach Reload erhalten. Lose Ressourcen erhalten serverseitige Schwerkraft,
Terrainkollision und 300 Sekunden Lebensdauer. Das Netzwerkprotokoll ist 12.

Debug und Blockgrenzen (Default F1/F1+B) sowie Chat (T/Slash) sind rebindbar;
Debug bleibt anfangs aus. Tab vervollstaendigt vorhandene Chatbefehle.
Texteingabe-/Menuesperren und Konfliktpruefung bleiben erhalten. Wiederkehrende
Streamingmeldungen liegen auf Debug, normale Konfigurationen auf Info.
Moderation, weitere Rollen und weiterfuehrende Inventar-UX bleiben in TODO.

470 Workspace-Tests bestanden, ein bestehender Soak ignoriert. Separater
Headless-Build/-Start und Featurepruefung ohne Render-/UI-Abhaengigkeiten
bestanden. Native Avatar-/Inventar-/Bau-/Chatpruefungen und zwei reale UDP-Clients
wurden ausgefuehrt. Die kleine Wayland-Menueansicht wurde auch mit echter
OS-Maus auf Dropdown, Scrollen, Slider und Verwerfen geprueft.
Die alte Performancebinary ist mit den neuen Laufzeittemplates nicht kompatibel;
dieser Vorherlauf ist verworfen und kein Performancevergleich.
Isolierter Nachherlauf bei 1920x1200 auf RX 6950 XT: 155.97 FPS,
p95/p99 8.087/9.584 ms, 92789 in 1306 Batches repraesentierte Props.
Das ist eine stationaere Offline-Aufnahme, keine Bewegungs-/Netzwerkabnahme.
[Pruefbericht, Grenzen, Rohdaten und Aufnahmen](tools/benchmarks/results/inventory-ranger-2026-10-16/README.md).

## 28. Kamera, Mausgesten, Physikalische Drops Und Gras

Lokaler Avatar und Kamera verwenden dieselbe geglaettete Physikdarstellung statt
unterschiedlicher Fixed-/Renderpositionen. F5 schaltet erste Person, hinten und
vorne; die unabhaengige Blickrichtung erhaelt Laufsteuerung und Picking.
Freundlichere kleine Augen blinzeln auch im Inventarportrait. Zwei einmalige
GPU-Portraits ersetzen einen dauerhaft laufenden Vorschau-Renderpass.

Feste leicht gerundete Quadratslots, 18 Rucksackplaetze und neun separate
Hotbarslots. Serverautoritativer gespeicherter Cursor mit Aufnehmen, Teilen,
Einzelablage, Verteilen, Ziehsammeln, Shift-Transfer, Doppelklick und Mausrad.
Schnelle Aktionen werden nach ACK geordnet und behalten ihre Cursorabsicht.
Middle Pick kopiert den Zielblock nur in Creative; Q/Shift+Q werfen eine Einheit
oder den ganzen Stack. Aussenklick wirft den Cursor ab. Blockbeschreibungen
stehen im jeweiligen JSON, Propbeschreibungen im bestehenden Assetkatalog.

Lose Ressourcen nutzen echte geteilte 3D-Block-/Propmodelle, autoritative
Wurfrotation, leichtes Abprallen und flache Ruhelage statt Schweben.
Normale Cubeziele wackeln beim Survival-Abbau und verlieren Teile; geschlossene
Nachbarflaechen und asynchroner Mesh-Handoff vermeiden Durchblicke/Blockblitzen.
Creative bleibt sofortig. Sections mit importierten Custom-Mesh-Szenen behalten
vorerst das Riss-Overlay; Drop-gegen-Drop-Stapelsimulation bleibt offen.

Gras erhaelt drei Formen aus stumpfen, breiten, geknickten Halmen, bodennahen
Blattdetails und vereinzelten weissen Blueten. Bestehende Wind-, Biom-, Picking-
und Section-Batch-Pfade bleiben erhalten, ohne schwebende Dirtunterlage.

Alle fuenf Cargo-Pruefungen bestanden: 516 Tests, ein bestehender Soak ignoriert,
Clippy ohne Warnungen. Separater Headless-Build/-Start ohne Render-/UI-Features
und zwei native GPU-Netzwerkclients bestanden. Reale OS-Inventarklicks,
Middle Pick, F5, Q/Shift+Q, stabile 3D-Drops, Blinkportraits, Wind und beide
Bau-Spielmodi wurden nativ geprueft. Kamera und Avatar stimmten in 726 Samples
auf derselben geglaetteten Position ueberein; das ist kein Internetlatenztest.

Isolierter gleicher stationaerer Blick: 155.97 -> 154.29 FPS,
p95/p99 8.087/9.584 -> 7.846/9.090 ms, unveraendert 92789 repraesentierte Props
in 1306 Batches. Etwa 1.1 Prozent weniger mittlere FPS in einem Messpaar,
kein behaupteter Speedup und kein Bewegungs-/Drop-Worst-Case-Benchmark.
[Technik, Grenzen, Tests, Aufnahmen und Rohdaten](tools/benchmarks/results/interaction-camera-drops-2026-10-16/README.md).

## 29. Stroemung, Wasserquellen, Bildtabs Und Artenfarben

Der Hals ist 65 Prozent kuerzer, Glas wirft einschliesslich seiner Rahmen keine
Schatten und das dunkle Farbgrading ist mit Gain 1.2 aufgehellt. Oak, Birch und
Spruce erhalten aus der echten Baumart abgeleitete Rindenfarben, Birke eine
gemusterte Rinde und Fichte dunklere Nadeln. Vorhandene Block-IDs bleiben erhalten.

Fluid Water Block (neue ID 27) ergaenzt Solid Water (14) als echte, platzierbare
und wieder entfernbare Zellwasserquelle. Sie nutzt dasselbe Journal, dieselbe
begrenzte Simulation, das gemeinsame Wassermesh und bestehende Replikation.
Gemeinsame Stroemung wirkt mit 40 Prozent auf Spieler, dynamische Avian-Koerper
und lose Items; Fallgardinen haben einen begrenzten Abwaertszug. Aktiv fliessendes
Zellwasser entfernt nur wirklich beruehrte Props ueber deren bestehende IDs und
Prop-Epochen, auch an negativen/vertikalen Sectiongrenzen.

Die Ursache breiter planer Wasserfallwaende war der ueber die ganze Talbreite
verwendete hydraulische Pegelsprung. Kontinuierliche aeussere Erosionsprofile
trennen jetzt Bank-/Dachgelaende vom nassen Absturz. Fall-/Aufprallschaum ist
verstaerkt; ein gemeinsamer Batch liefert bis zu 96 leichte Spruehnebelquads
in 48 m Entfernung, abschaltbar ueber Low-Wasserqualitaet. Keine echte Volumetrik.

Gras hat quadratische stumpfe Segmenthalme, farblich abgesetzte Abschnitte,
dichtere bodennahe Blattgruppen und gelegentliche weisse Blueten, ohne
schwebende Dirtplatte. Die vorhandenen Wind-/Farb-/Prop-Batches bleiben erhalten.
Creative nutzt echte Itembildtabs ausserhalb und oberhalb des Inventarrahmens,
auch kompakt, mit Hover-Titeln und Auswahlmarkierung. Namespaced Customkategorien
sind ueber den vorhandenen Assetkatalog definierbar; kein vollstaendiger Modloader.
Slottooltips liegen bevorzugt rechts unten ausserhalb des Slots, mit freier
Ausweichseite am Fensterrand. Tabklicks verlieren keinen gehaltenen Cursorstapel.

Alle fuenf Cargo-Pruefungen, 548 Tests und separater Headless-Build/-Start
bestanden; ein vorhandener Langzeit-Soak bleibt ignoriert. Native Avatar-,
Inventar-, Botanik-, Wasserfall- und autoritative Source27-Abnahmen bestanden.
Die Source27-Runde bestaetigte 105 Nasszellen, fuenf entfernte Props und
serverseitige Quellentfernung. Gleicher stationaerer Blick: 154.29 -> 155.19 FPS,
p95/p99 7.846/9.090 -> 7.557/8.708 ms in einem 18-s-Messpaar, kein garantierter
Speedup. Terrain-Workerarbeit steigt dagegen 0.630 -> 0.732 s; dies bleibt als
CPU-Mehrarbeit transparent dokumentiert. Keine daraus abgeleitete Bewegungs-
oder Streamingparitaet. Konkrete Nachweise und Grenzen stehen im
[Phase-29-Bericht](tools/benchmarks/results/water-current-tabs-2026-10-16/README.md).

## 30. Wasserverdraengung, Audio, Tooltip Und Renderbudgets

Ein gemeinsamer Client-/Server-Zielvertrag erlaubt das Bauen direkt in eine
Wasserquelle. Der feste Block ersetzt deren Anker/Fuellstand, statt darauf zu
stapeln; Solid Water bleibt ein fester Bauuntergrund. Nachbarwasser bleibt endlich
und reagiert auf die neue Barriere. Revisionen, Inventar, Sicht-/Koerperpruefung
und Deltareload bleiben erhalten; feste Bloecke sind nicht frei ueberschreibbar.
Native Integrated-/UDP-Quelle durch Stein ersetzt und zwei Aufnahmen bestaetigt.

Tabtitel erscheinen oben rechts ausserhalb des Tabs, Itemtooltips weiter unten
rechts am Slot. Echter Wayland/uinput-Lauf bestaetigt Titelposition, kompakte/
breite Ansicht, erhaltenen Cursor und 65+63=128 beim Aussenwurf.
Stilles Wasser hat eine deutlich leisere, weichere eigene Stimme; Fluesse sind
sanfter und erhalten nicht mehr zusaetzlich den alten Flachwasser-Rauschanteil.
Wasserfall, Unterwasser-Tiefpass, Spielerhoerpunkt und Audiooptionen bleiben
getrennt erhalten. Reale AudioSink-/DSP-/Cleanuppruefung bestanden.

Terrain/Props/Wasser erhalten getrennte Vertexbudgets neben den vorhandenen
Joblimits. Fertige Ergebnisse warten bei Bedarf, alte Meshes bleiben bis zum
atomaren Austausch sichtbar. Wasseruploads sind nach Naehe priorisiert.
Begrenzte geteilte Spalten-/Sectionsnapshots cachen die echte Baumart auch ueber
LOD-Wechsel; fertige PNG-Mips halten keine dauerhaften CPU-Pixelkopien.
Keine verringerte Texturaufloesung und keine logischen Welt-/Blockaenderungen.

Optionale Diagnostik zaehlt tatsaechliche PBR-Draw-API-Aufrufe: im festen Blick
42 indirekte Aufrufe ueber Hauptpass, Prepass und Schatten. GPU-interne Subdraws,
UI und Fullscreenpasses werden nicht mitgezaehlt. Ein reiner Offline-Fixturemodus
vergleicht identische Kronen ohne/mit Maskierung und Wind, keine Grafikoption.
Neue A/B-Vergleiche fuer Pilzpark und geprueften Flussblick sind archiviert.

Alle fuenf Cargo-Pruefungen, 557 Tests, separater Headless-Build/-Start und native
UI-/Wasser-/Audio-/LOD-Pruefungen bestanden; ein bestehender Soak bleibt ignoriert.
Der serielle Testlauf umgeht eine beobachtete Ephemeralport-Kollision des alten
parallelen UDP-/TCP-Testsetups, keine Tests wurden ausgelassen.
Zeitnahes LOD-Paar: 134.19 -> 133.30 FPS, p95/p99 10.624/13.552 -> 9.994/11.897 ms.
Der feste Pilzparkblick verliert im Buildpaar rund 2.3 Prozent FPS; Fluss-FPS sind
nahezu gleich, dessen p99 ist schlechter. Erstladen im Pilzpark kostet durch die
Uploadbegrenzung etwa 0.24 s mehr. Keine allgemeine FPS-/Speicherparitaet.
Der ausprobierte Horizontabschluss brachte keinen verlaesslichen Bildgewinn und
wurde entfernt. Restliche Qualitaetserweiterungen bleiben bewusst in TODO.
[Nachweise, Rohdaten, Vergleiche und Grenzen](tools/benchmarks/results/water-placement-performance-2026-10-16/README.md).

## 31. Creative-Mengen, Wasserzufluss Und Verzweigte Grosspilze

Der unendliche Creative-Katalog zeigt weder Slotmengen noch eine kuenstliche
128 im Detailbereich. Echte Spieler-, Cursor- und Hotbarstapel behalten ihre
Mengen und das vorhandene 128er-Limit; serverseitiges Aufnehmen und MouseTweaks
bleiben unveraendert. Ausschliesslich vorhandene TiltUI-Komponenten werden genutzt.

Zellwasser verwendet nun quellengespeiste lokale Sollpegel statt verschobener,
dauerhaft erhaltener Restpakete. Fallzellen bleiben durchgehend gefuellt,
seitlicher Fluss verliert je gestuetztem Schritt eine Achtelstufe. In der Luft
entstehen keine Seitenplatten. Ohne Nachschub sinken Pegel schrittweise, auch
hinter einer blockierten Zuleitung oder nach Save/Reload; gleich hohe Pfuetzen
koennen sich nicht zyklisch erhalten. Naturreservoirs bleiben Quellen. Lokale
Arbeit bleibt serverseitig auf 256 Zellen bei 10 Hz begrenzt, an ungeladenen
Grenzen pausiert sie. Kein neues Fluessigkeits-/Speichersystem oder Drahtformat.
Gemeinsame Fall-Eckpunkte und eine geschlossene freie Fallfront beseitigen
Luftspalten; ein Strang schliesst ohne Luftzelle an vorhandenes Naturwasser an.

Fluid Water (27) laesst sich weder in Creative noch Survival abbauen, auch nicht
durch manipulierte direkte Netzwerkbefehle. Feste Bloecke verdraengen Quellen
weiter ueber die normale Platzierung. Der feste animierte Dekoblock (14) bleibt
abbaubar. Eimer und Pumpen sind ausdruecklich noch nicht implementiert.

Grosspilze aller sechs Farben besitzen deterministisch ein bis drei Kappen.
Seitliche Koepfe wachsen an geknickten, ansteigenden Armen statt nur direkt am
Stamm. Suchgrenzen und flaechenverbundene Rastergeometrie bleiben erhalten.
Near-Batches ergaenzen zwei Stiel-/Lamellenrippen beziehungsweise zwei bis drei
variierende helle Flecken, ohne neue Materialien oder Mikro-Entities. Die neue
Form gilt auch fuer regenerierte, uneditierte v5-Pilzstrukturen; alte Edits
bleiben an ihren Koordinaten. Keine Block-ID-, Seed- oder Generatorversionsaenderung.

Alle fuenf Cargo-Pruefungen bestanden: 562 Tests, ein vorhandener ignorierter
Langzeittest. Separater Headless-Build/-Start ohne DISPLAY/WAYLAND erfolgreich,
27 Sections; keine Render-/TiltUI-/wgpu-/winit-Abhaengigkeiten im Serverbaum.
Native Integrated-/UDP-Abnahme bestaetigt 108 Ausflusszellen, fuenf gezielt
entfernte Props, abgewiesenen Wasserabbau, Quellenersatz und vollstaendigen
Rueckgang. Eine 30-Block-Testgardine ueber vertikale Sections fuellt einen
kontrollierten Steinbehaelter, passiert dessen geoeffnetes Grenztor und trocknet
nach dessen Verschluss rechts vollstaendig aus. Pilzansichten und echte
Wayland-/Maus-Inventarinteraktionen sind geprueft. Ein erster UI-Lauf waehrend
des Kompilierens traf ein Verbindungs-Timeout; die getrennte Wiederholung bestand.

Fester Pilzparkblick: 145.47 -> 144.17 FPS, p95/p99 8.171/9.162 -> 8.244/8.951 ms;
Peak RSS steigt um rund 53 MiB. Mehr Geometrie, kein behaupteter Speedup.
[Nachweise und Messgrenzen](tools/benchmarks/results/fluid-recession-mushrooms-2026-10-16/README.md).

## 32. Regiondateien, Fallwasserqualitaet Und Exaktes Pflanzen-Picking

Fallwasser verwendet weiter dasselbe PBR-Wassermaterial wie ruhige Oberflaechen.
Die Ursache der weissen Streifen war eine Zellmetadaten-Zuordnung, die jede
Fallzelle gleichzeitig als Lippe und Aufprall markierte. Ein eigener Effektcode
trennt die dynamische Gardine vom festen Wasserbaublock. Interpolierte lokale
Randabstaende begrenzen Schaum; die Koerpermitte bleibt transparent und gefaerbt.
Mehrskalige, mitlaufende Normalen ersetzen die gleichmaessigen senkrechten Rillen.
Keine weitere Wasserpipeline oder zusaetzlicher Reflexionspass.

Der alte monolithische TOML-Zustand wird durch binaere `regions/r.X.Z.region`
und `players/UUID.data` ersetzt. Regionen umfassen 32x32 Chunk-Spalten mit allen
Y-Sections und speichern bestehende SectionDeltas, Wasser, Props, Bodenzustaende
und Drops. Spielerdateien enthalten echte Inventare/Cursor sowie autoritative
Bau-/Abbau-/Prop-Erntezaehler. Ein kleiner `level.data`-Index ersetzt keine Welt-
oder Blockregistry. Unveraenderte Dateien bleiben unveraendert; ein fsync-gesicherter
Commitplan erlaubt Replay nach Teilinstallation. Korrupte Dateien und fehlende
Indizes starten keine leere Ersatzwelt. Integrated speichert ebenfalls asynchron.

Singleplayer nutzt sichere eindeutige Namen unter `worlds/Name/`, Dedicated
direkt `worlds/`, ohne weitere Namensebene. Alte Hex-Ordner, TOML-Zustaende und
der bisherige Dedicated-Standard werden validiert uebernommen und gesichert.
Weltformat 2 verhindert, dass alte Binaries neue Regionsdaten ignorieren;
interner Binaerheader 1, Generator 5 und Protokoll 14 bleiben getrennte Vertraege.
Keine Aenderung bestehender IDs/Seeds. Dimensionen sind weiterhin eine spaetere
Erweiterung, kein jetzt nur vorgetaeuschter Mehrdimensionenbetrieb.

Die vorhandenen Pflanzenvorlagen liegen jetzt CPU-seitig gemeinsam in terra-world.
Renderer und Picking nutzen dieselben Dreiecke, Varianten und visuellen Skalierungen.
Boxen sind nur Vorauswahl; Luft zwischen Halmen/unter Pilzkappen wird nicht mehr
getroffen. Windverformung und Auswahlkanten folgen derselben Bewegung wie der
Shader; serverseitig ist nur der enge Dreiecks-Windbereich zugelassen. Normale
solide Terrainbloecke behalten ihre exakten Meterzellen; kosmetische Bevels und
Reliefs aendern nicht das Bau-/Kollisionsraster. Keine Entity pro Detail.

Der Speicherbenchmark mit 16384 Edits, 16 Regionen und 16 Spielern misst
3980604 Byte fuer die alte TOML-Struktur gegen 270140 Byte binaer, rund 93.2 Prozent
kleiner. Zeiten stammen aus tmpfs und sind keine Festplatten-/FPS-Garantie.
Native Fallwasser-, Pflanzen-/Wind-, Menue-Save-/Hostneustart-/Reload- und
Headless-Pruefungen bestanden. Format-1-Metadaten wurden nativ mit Backup auf 2
angehoben. Alle fuenf Cargo-Pruefungen und die konkreten Restgrenzen stehen im
[Pruefbericht](tools/benchmarks/results/regions-water-picking-2026-10-16/README.md).

## 33. Monitor, Inventar Und Streaming-Optionen

Aufloesungen werden aus dem aktuellen/Primaermonitor, nativen Abmessungen und
Videomodi zusammengestellt, einschliesslich 4K und passenden Fensterformaten.
Dynamische TiltUI-Optionen sind korrekt im Popup gruppiert und ihre gebundenen
Beschriftungen werden synchronisiert. Bestehende Display-Bestaetigung bleibt.

Wasser besitzt keine Auswahlumrandung mehr, bleibt aber fuer Quellenersetzung
treffbar. Feste Platzierungen wandeln darunterliegenden Grass-Boden durch das
bestehende autoritative Journal in Dirt um, auch ueber Section-Grenzen.

Creative-Katalog gibt ohne Shift eine Einheit, mit Shift bis 128 direkt in den
Bestand. Die quadratischen Kategorie-Tabs sind rund 40 Prozent groesser. Alle
Tooltips folgen dem Cursor mit zehn UI-Pixeln Abstand; am Rand weichen sie aus.
Detailskennungen bleiben einzeilig, lassen bei Platzmangel den Namespace weg und
zeigen dessen Herkunft im Tooltip. Fremde Namespaces werden beibehalten.

Sichtweite bis 32 Chunk-Spalten, Oberflaechennebel aus/Distanz und drei echte
Streamingmodi sind persistent in den Grafikoptionen verdrahtet. Sofortiger
Wechsel bleibt Default; optionale Fades verwenden fuer 300/750 ms temporaere
Blend-Materialien und geben diese danach frei. Terrain-, Prop- und Wasserbatches
sind unter einem Limit von 96 gleichzeitig animierten Wurzeln eingebunden.
LOD-/Edit-Ersetzungen und die bestehenden Uploadbudgets bleiben unveraendert.
Keine Generator-, Welt-, Block-ID- oder Dependency-Aenderung.

Nachweise und Einschraenkungen stehen im
[Phase-33-Bericht](tools/benchmarks/results/settings-inventory-streaming-2026-10-16/README.md).

## 34. Renderbestaetigte Animationen, Weitere LODs Und Renderkosten

Neue Chunkbatches warten unsichtbar und starten ihre Fade-Uhr erst nach einem
tatsaechlich erfolgreichen Farb-Draw. Die begrenzte Queue hat keinen sichtbaren
Instant-Fallback mehr. Neben Fade/Slow Fade gibt es Bounce, Rise und Glide;
Default bleibt Instant. Endtransforms, Materialhandles und Schattenflags werden
restauriert. Wartende schattenfaehige Batches bleiben auf einem reinen
Sonnenlayer Schattenwerfer; aktive Animationen sind gegen kosmetische LOD-Wechsel
geschuetzt, noch unsichtbare Queue-Batches duerfen ihre LOD aktualisieren.

Block-LOD samt Hysterese ist 50 Prozent weiter entfernt: effektiv 144/240/18 m
bei bisherigen Basiswerten, Nahrelief 48 m statt 32 m. Prop-Reichweiten bleiben
getrennt. Quad-Eckpunkte werden ohne Veraenderung von Dreiecken/UV-Seams geteilt.
Windlaub verwendet auf Workern gepackte Normalen/Farben, 28 statt 48 Bytes pro
Vertex; Masken, Wind und Float-Preview-/Abbauteile bleiben erhalten. LOD-/Queue-
Scans sind gebucktet/gecacht, redundante Transformupdates entfallen.

Eine tiefengepruefte, nebelabhaengige Horizonthuelle gleicht den planetaren
Hintergrundstreifen an die endliche Sichtweite an. Optionaler echter
Wasserfall-Volumennebel nutzt den vorhandenen Bevy-Pfad: nur High, Default aus,
maximal vier nahe echte Aufprallquellen. Keine weitere Renderpipeline.

Erfolgreiche TiltUI-Renderer-Aufrufe und asynchron ausgelesene GPU-Farb-Subdraws
sind getrennt instrumentiert. Vollstaendige Shadow-/Prepass-/Fullscreen-Zaehler
werden weiterhin nicht behauptet. Benchmarkwerkzeug erfasst optional echte
DRM-VRAM-/GTT-Allokationen; lange Flugfenster sind bis 1800 Sekunden einstellbar.

Im isolierten 60-s-Laubpaar bei gleicher Geometrie sinkt Peak-VRAM von
2746.07 auf 2154.94 MiB. Das ist keine allgemeine FPS-/RSS-Paritaet. Der finale
120-s-32er-Fade-Flug bleibt langsamer: 34.40 -> 30.47 FPS im wiederholten alten/
neuen Paar, p95 38.939 -> 44.914 ms, p99 74.878 -> 62.584 ms. Restkosten bleiben
offen; historische Regressionen werden nicht nachtraeglich umgedeutet.

Alle fuenf Cargo-Pruefungen bestanden, 592 Rust-Tests erfolgreich und ein
vorhandener ignoriert, zusaetzlich ein Python-Test. Native Animationen, TiltUI-
Sitzung, Wasserfall-Gischt, Horizont/Ocean und renderfreier Serverstart geprueft.
Nur eine physische GPU verfuegbar. Weitere GPUs, lange Soaks, vollstaendige
Frameabnahme, echte Offscreen-Objektreflexionen, bedarfsabhaengiges Skinning
und gemeinsame Art-Abnahme bleiben konkret in TODO offen.

Messbedingungen, Zwischenregressionen, Aufnahmen und Reproduktion:
[Phase-34-Bericht](tools/benchmarks/results/rendering-streaming-2026-10-16/README.md).

## 35. Erstupload-Animation Frisch Generierter Chunks

Die transparente Horizonthuelle wird vor einblendendem Terrain gezeichnet.
Ihr kameranaher Mittelpunkt hatte die Sortierung umgekehrt: Neue Chunks wurden
vom Hintergrund verdeckt und erst am Ende des Fades sichtbar. Ein reiner
Sortier-Bias korrigiert das bei unveraenderter Rastertiefe, ohne weiteren Pass.

Jeder Materialteil startet seinen Fade nach dem eigenen ersten Farb-Draw.
Ein schneller Batch verbraucht nicht mehr die Animation spaeter GPU-bereiter
Geschwister. Teilweises Entladen behaelt die individuelle Deckkraft bei,
Offscreen-Teile blockieren keine fertigen Animationsplaetze. Identische
Alphawerte loesen keine erneuten Materialuploads aus. Erste sichtbare Prop-/
Wassermeshes nach leeren Cacheeintraegen durchlaufen ebenfalls die Animation.
Generierung, Meshing, Uploadbudgets und die Begrenzung aktiver Fades bleiben
unveraendert.

Der native Test beobachtet jetzt den ersten Upload frisch generierten Terrains
und prueft neben Alpha-/Drawwerten auch sichtbare Terrainpixel im Screenshot.
Danach folgen erneuter Fade, Bounce, Rise und Glide. Alle fuenf Cargo-Pruefungen
bestanden, 597 Rust-Tests erfolgreich und einer weiterhin ignoriert.
Streaming-Messungen samt verbleibenden Frame-Spitzen und die tatsaechlich
geoeffneten Vorher-/Nachheraufnahmen stehen im
[Phase-35-Bericht](tools/benchmarks/results/fresh-chunk-fade-2026-10-16/README.md).

## 36. Prop-Gruppen, Offenes Gras Und Biomfarben

Generierte und gepflanzte Pflanzen teilen vier feste Plaetze pro Bodenblock.
Gras sitzt mittig und belegt zwei Einheiten; daneben passen zwei Eckpflanzen,
ohne Gras bis zu vier. Primaere generierte IDs bleiben erhalten, Zusatzplaetze
erhalten stabile abgeleitete IDs. Einzelabbau verschiebt keine Nachbarn.
Dieselbe Belegungspruefung gilt im Server, im Commit und nach Journal-Reload.
Alte grosse gespeicherte Pflanzen werden nicht automatisch verschoben.

Gras hat zwoelf stumpfe, verbundene Halme mit unterschiedlichen Hoehen,
dunkleren Wurzeln, waermeren Spitzen und niedrigen Blatttrieben. Seine
aeussere Grundflaeche liegt ungefaehr bei 80 Prozent einer Blockoberseite;
die Zwischenraeume bleiben echte Luft und sind auch beim Picking durchlaessig.
Wind und Section-Batching bleiben bestehen. Eckpflanzen werden passend
verkleinert; Lichtungen und Biommuster begrenzen die zusaetzlichen Kandidaten.

Zufaellig pro Meterzelle gewaehlte Sand-/Schneeflecken sind durch zusammenhaengende
Materialkonturen ersetzt. Gemeinsame lineare Oberflaechenfarben und passende
Textur-Albedofaktoren lassen Gras, Sand und Schnee in Uebergangsbaendern
ineinander uebergehen. Gebaute Sand-/Schneebloecke behalten ausserhalb dieser
Baender ihre eigene Farbe. Regen-/Schneewetter verwendet konsistente Faktoren.
Grobes Meshing unterteilt nur Flaechen mit sichtbar nichtlinearem Farbverlauf;
gleichfarbige oder lineare Flaechen bleiben grosse Greedy-Rechtecke.

Wasser erhaelt kontinuierlich gewichtete Biomfarben: Wald/Plains/Ocean bleiben
unveraendert, Pilzpark wird helllila, Wueste heller blau und Hochland dunkler blau.
Gemeinsame Section-Randproben gelten fuer Fluessigwasser, Wasserfaelle und feste
Wasserbloecke; Absorption folgt dem Farbton. Weiterhin ein Wassermaterial und
kein weiterer Renderpass. Ein RGB-Attribut kostet zusaetzlich 12 Byte pro
Wasservertex. Inventar-/Drop-Vorschauen ohne Ortsbezug behalten die Grundfarbe.

Generierte Hoehen, Welt-/Block-IDs, Seeds, Dateiformate und Generatorversion
bleiben unveraendert. Uneditierte Oberflaechenmaterialien und Pflanzenverteilung
aendern sich absichtlich; gespeicherte Block-Edits und Prop-Entfernungen bleiben
erhalten. Client und Server gemeinsam aktualisieren.

Alle fuenf Cargo-Pruefungen bestanden, 618 Rust-Tests erfolgreich und ein
bestehender Langzeittest ignoriert. Native Pflanzenansichten pruefen auch
Zielauswahl und Wind; der Server startet headless. Der identische 60-s-Waldblick
misst 82.75 -> 79.03 FPS bei 92784 -> 156348 repraesentierten Props und
hoeheren Speicher-/Kompositionskosten. Keine Performanceparitaet behauptet;
Rohwerte und Grenzen stehen im
[Phase-36-Bericht](tools/benchmarks/results/biome-props-2026-10-16/README.md).

## 37. Zentrale Pflanzengruppen Und Artgerechte Groessen

Nicht-Gras-Props koennen bis zu ein Viertel der Blockgrundflaeche einnehmen,
statt auf die bisherigen 0.26-m-Ecken geschrumpft zu werden. Blumen erreichen
im Nahbereich 65 bis 95 cm, kleine Pilze 35 bis 55 cm. Grundflaeche und Hoehe
werden getrennt normalisiert; Varianten, Stamm-/Bluetendetails, Wind und
gemeinsame Picking-Geometrie bleiben erhalten. Bestehende Blumen-/Pilzgroessen
erhalten dieselben Darstellungslimits ohne Aenderung ihrer gespeicherten Daten.

Ohne Gras steht eine einzelne Pflanze mittig. Zwei stehen nebeneinander, drei
bilden eine Dreiergruppe und vier ein Quadrat um die Blockmitte. Mit Gras bleiben
zwei diagonal versetzte Begleiter moeglich; Gras belegt weiterhin zwei Einheiten.
Die Anordnung wird nach aktiven IDs abgeleitet. Beim Hinzufuegen/Entfernen wird
nur dieselbe Gruppe neu zentriert, nicht Nachbarbloecke. ID, Art, Rotation und
Groesse der verbleibenden Pflanzen bleiben erhalten. Autoritaet, Replikation
und Reload verwenden dasselbe Layout, ohne neue Journal-/Save-Felder.

Groessenmetadaten sind je Template gecacht, Normale werden nach der
nichtuniformen Anpassung korrekt normalisiert. Generierte Gruppen brauchen
keine eigenen Heap-Listen; bestehende Section-Batches und Vertexzahlen bleiben
unveraendert. Daraus wird keine ungemessene FPS-Paritaet abgeleitet.

Alle fuenf Cargo-Pruefungen erfolgreich, 623 Tests bestanden und ein vorhandener
Langzeittest ignoriert. Native Pflanzen-/Wind-/Auswahlpruefung und fensterloser
Serverstart bestanden. Details und Aufnahmen:
[Phase-37-Bericht](tools/benchmarks/results/prop-layout-2026-10-17/README.md).

## 38. Weltzeit, Sonne, Mond Und Tag/Nacht

Gemeinsame serverautoritative Tagesuhr fuer Integrated und Dedicated Server.
Tagesnummer und Tagesanteil bleiben ueber Autosave, manuellen Save und Shutdown
im binaeren `level.data` erhalten. Tageslaenge, Startzeit und Pause sind pro Welt
konfigurierbar; Default 24 reale Minuten pro Tag und Start um 10 Uhr.
Protokoll 15 synchronisiert die Uhr beim Join und zweimal pro Sekunde,
Clients extrapolieren mit weichen Korrekturen. Alte `INDX`-Speicherdateien werden
gelesen und beim Speichern explizit zu `IDX2` erweitert. Keine Region-/Player-
Migration, keine neuen Seeds, keine Terrain-Aenderung durch reine Zeitfortschritte.

Bestehende Atmosphaere um laufende Sonne, Mondlicht, strukturierte Mondscheibe
und rotierenden Sternenhimmel erweitert. Gemeinsame weiche Tageskurven steuern
Licht, Schatten, Ambientlicht, Belichtung, Nebel und Unterwasserdarstellung.
PBR-Wasser und SSR folgen derselben Beleuchtung. Jeweils nur ein Himmelslicht
wirft Schatten; keine Einzelstern-Entities und kein zusaetzlicher Fullscreen-Pass.
Brightness, Shadow Quality und Fog Off funktionieren weiterhin. Ein nativer
Fund beim Wechsel der Grafikoptionen wurde behoben: Nachtbelichtung wird nicht
mehr versehentlich durch den Tageswert ersetzt.

629 Tests bestanden, ein bestehender Langzeittest ignoriert. Kalenderwechsel,
Pause, Late Join mit zwei echten Clients, Save/Reload, alter Index und fehlende
Terrain-Invalidierung geprueft. Acht native Vulkan-Aufnahmen auf RX 6950 XT
einschliesslich Tag, Daemmerung, Nacht, Mond/Sternen und Unterwasserbildern;
Bilder geoeffnet und Nachtlesbarkeit nachgebessert. Headless-Server-Smoke-Test
ohne DISPLAY/WAYLAND_DISPLAY erfolgreich mit 27 Sections.

Dekorative Mondoberflaeche und Sterne sind nicht Teil der atmosphaerischen
Environment-Map; Mondlicht und Nachtfarben schon. Keine FPS-/Speicherparitaet
oder Langzeitabnahme behauptet. Details und Logs:
[Phase-38-Bericht](tools/benchmarks/results/daylight-2026-10-17/README.md).

## 39. Chat-Vorschlaege, Zeitbefehle Und Voxel-Himmel

Sichtbare TiltUI-Vorschlaege mit Auswahl, Tab/Shift+Tab, Klick und Scrollen.
Gemeinsames Befehlsvokabular fuer UI und Parser; echte verbundene Spielernamen
werden mit Protokoll 16 repliziert. `/time set` unterstuetzt Tagesnamen,
24-Stunden-Zeit und AM/PM. `/tp` verwendet absolute Fusskoordinaten oder einen
Online-Spielernamen, inklusive Namen mit Leerzeichen in Anfuehrungszeichen.
Serverberechtigungen, freier Zielraum, Lade-Handshake und Generationenwechsel
bleiben verbindlich. `/clear` leert ausschliesslich den lokalen Chatverlauf.

Goldene Voxel-Sonne mit Blockstrahlen und heller Sichelmond mit blauem Rand
ersetzen die einfachen Scheiben im vorhandenen Himmelpfad. Prozedurale
Silhouette und Relief statt zusaetzlicher Einzelvoxel-Entities; Beleuchtung,
Tag/Nacht, Sterne, Unterwasser und Fog-Off bleiben integriert. Beide Zeitformate
sind unabhaengig von einer noch nicht vorhandenen globalen Sprachwahl nutzbar.

634 Tests bestanden, ein bestehender Langzeittest ignoriert. Fuenf Cargo-
Pruefungen, nativer TiltUI-Test samt Pfeil/Tab/Klick und zwei Fenstergroessen,
neun Himmel-/Unterwasseraufnahmen sowie fensterloser Serverstart erfolgreich.
Erster GUI-Lauf unter paralleler Buildlast mit Verbindungs-Timeout; erfolgreiche
Wiederholungen und Grenzen im [Phase-39-Bericht](tools/benchmarks/results/chat-celestials-2026-10-17/README.md).

## 40. Rundum-Streaming, Hellere Abende Und Weniger Blumen/Pilze

Fertig vorbereitete Chunks ausserhalb des Blickfelds verlassen ihre bisher
sichtabhaengige Animationswarteschlange im Hintergrund. Maximal 64 Wurzeln je
100-ms-Aktualisierung, keine zusaetzlichen Fade-Materialien. Sichtbare neue
Batches behalten ihren ersten Draw als Fade-Start; fehlende Bounds und noch
nicht propagierte Welttransformationen warten weiterhin sicher. CPU-Streaming
war bereits rundum aktiv; Job- und Mesh-Uploadbudgets bleiben unveraendert.

Naechtliches Ambientlicht von 90 auf 300 angehoben und den Nacht-Fuellton
neutraler abgestimmt, um Schatten insbesondere zwischen 19 und 22 Uhr
aufzuhellen. Tageslicht und Belichtung bleiben erhalten.
Natuerliche Blumen und kleine Pilze werden nach der bisherigen Gruppenauswahl
mit stabilem ID-Hash auf 25 Prozent ausgeduennt. Von 57.805 Testkandidaten bleiben
14.502 erhalten (25,09 Prozent). Keine Ersatzpflanzen; andere Arten, grosse
Pilzbaeume und platzierte Props bleiben erhalten. Vorhandene IDs, Seeds,
Generatorversion und Speicherformate aendern sich nicht.

638 Tests bestanden, ein vorhandener Langzeittest ignoriert. Alle fuenf
Cargo-Pruefungen, nativer Rundum-/Kameradrehungs-/Animationstest, zwoelf
Beleuchtungsaufnahmen und Headless-Server-Smoke-Test erfolgreich. Nachtansichten
um 19:00, 20:30 und 22:00 visuell geprueft; erste zu dunkle Probe nachgebessert.
Logs, Verteilungszahlen, Aufnahmen und Grenzen im
[Phase-40-Bericht](tools/benchmarks/results/surroundings-2026-10-17/README.md).

## 41. Kuerzere Abendschatten, Pilzdetails Und Chunk-Ladeanzeige

Sonnen-/Mondlicht bei flachem Winkel kuenstlerisch auf mindestens 30 Grad
angehoben: Schatten auf ebenem Boden bleiben unter etwa 1,73 Objekt-Hoehen,
und Oberflaechen ab 17 Uhr erhalten mehr direktes Licht. Himmelsazimut, sichtbare
Himmelskoerper, Weltzeit und Mittag bleiben erhalten; Atmosphaerenbeleuchtung
folgt der stilisierten Lichtrichtung. Keine zusaetzlichen Licht-/Renderpaesse.

Details grosser Pilze vom nahen Micro-Relief-Cutoff entkoppelt. Volle Anbauten
in Near, reduzierte identische Teilgeometrie in Middle, Basis in Far. Default
144/240 Meter plus Hysterese und bestehender LOD-Regler. Geschlossene
Terrainbasis und bestehende Material-/Section-Batches bleiben erhalten.

Ladebildschirm zeigt einheitlich echte Chunk-Spalten statt wechselnder Section-
und Mesh-Zaehler. Alle vertikalen Sections und Terrain-Uploads einer Spalte
werden beruecksichtigt; Einstieg wartet auf Vegetation und erste Wasser-
Darstellung. Laufende Fluidepochen verhindern den Einstieg nicht dauerhaft.

UI Scale oben im Graphics-Tab, 75-150 Prozent mit Entwurfsvorschau und
Apply-/Reset-/Discard-/TOML-Integration. Relative Fensteranpassung macht den
Regler auch bei kleinen HiDPI-Fenstern wirksam; Sicherheitsgrenze bleibt
sichtbar und das Wunschmass gespeichert. Kein neues Konfigurationssystem.

643 Tests bestanden, ein vorhandener Langzeittest ignoriert. Alle fuenf
Cargo-Pruefungen erfolgreich. Native Vulkan-Aufnahmen fuer 17/17:30/18 Uhr,
Tag/Nacht/Unterwasser, entfernte Pilze und echte Menue-/Session-Lebenszyklen.
Headless-Server mit 27 Sections erfolgreich. Keine neue Langzeit-FPS-/Speicher-
Paritaet behauptet; zusaetzliche entfernte Pilzgeometrie bleibt im Profiling-
Backlog. Reproduktion, Logs und bekannte Grenzen:
[Phase-41-Bericht](tools/benchmarks/results/menu-lighting-lod-2026-10-17/README.md).

## 42. UI Scale Erst Mit Apply Und Schattenwechsel Bei Daemmerung

UI-Groesse folgt nur noch angewendeten Grafikeinstellungen. Regler und Reset
aendern den Entwurf, ohne Menueelemente unter dem Cursor zu verschieben. Erst
Apply uebernimmt die Groesse; Speicherung, Verwerfen, HiDPI/Fenster-Fit,
Speicherfehler und Display-Rollback bleiben im bestehenden Optionssystem.

Harten Shadowmap-Schalter bei Himmelsrichtung `y=0.05` entfernt: Die Sonne
beleuchtete dort noch mit rund 47.000 Lux, ploetzlich aber ohne Schatten.
Sonnen-/Mondkarten bleiben nun bis zur auf null ausgeblendeten Lichtenergie
aktiv. Wind, durchlaessiges Laub, Props und kurze Abendschatten bleiben erhalten.
In der kurzen Daemmerungs-Ueberlappung koennen beide vorhandenen Himmelslichter
Schatten werfen; Shadows Off gilt weiterhin. Keine neuen Shader oder Lichter.

645 Tests bestanden, ein bestehender Langzeittest ignoriert; alle fuenf
Cargo-Pruefungen erfolgreich. Echter TiltUI-Regler vor/nach Apply nativ
geprueft, 19 Vulkan-Aufnahmen inklusive Morgen-/Abend-Schwellen, Headless-
Serverstart mit 27 Sections. Keine pauschale Flimmer-/FPS-/Speicherparitaet
oder Video-Langzeitabnahme behauptet. Details und Nachweise:
[Phase-42-Bericht](tools/benchmarks/results/apply-shadows-2026-10-17/README.md).

## 43. Player Stats, Survival Und Gameplay-HUD

Gesundheit, Ausdauer, Hunger und Sauerstoff im vorhandenen `PlayerStats`
ergaenzt; Abbau-/Bau-/Erntezaehler bleiben erhalten. Integrated und Dedicated
simulieren dieselben begrenzten Regeln. Sprint, Sprung und Schwimmen verbrauchen
Ausdauer/Nahrung; Erschoepfung begrenzt Sprint/Sprung mit Hysterese, verhindert
aber niemals das Auftauchen. Fallschaden, 20 Sekunden Atemluft, Ertrinken,
Verhungern und nahrungsabhaengige Heilung sind autoritativ. Creative ist ausgenommen.
Gesammelte kleine Pilze sind die erste essbare Ressource: ausgewaehlter
Hotbarplatz, rebindbare Use-Aktion (Default R), 18 Hunger pro Einheit.
Inventarrevision und Servervalidierung verhindern Mehrfachverbrauch.

Tod sperrt Bewegung, Abbau und Aufsammeln. Der TiltUI-Respawn verwendet die
bestehende trockene Festland-Spawnpruefung und den Lade-/Teleport-Handshake,
einschliesslich neuer Bewegungsgeneration und zwei Sekunden Schutz. Inventar,
Cursor und bisherige Zaehler bleiben bei diesem Survival-Grundmodus erhalten;
Respawn setzt die vier Vitalwerte zurueck. Reconnect heilt nicht und umgeht
keinen Tod. Spielerwerte bleiben UUID- und weltgebunden.

Gameplay-HUD ausschliesslich mit TiltUI unten links: facettiertes Herz mit
HP-Leiste, goldener Blitz mit Ausdauerleiste, Keule in fuenf Bissstufen bis zum
blanken Knochen, bedingte Atemanzeige, Schadenfeedback und Respawn-Dialog.
Gemeinsamer transparenter Bitmapatlas statt Einzelbildern/zweitem UI-System;
F1 steuert weiterhin nur Debug. Die Keule ist das Hungersymbol, kein neu
vorgetaeuschtes Tier-/Fleischitem.

Spielerdateien schreiben den expliziten Record-Typ `PLY2`; alte `PLYR`-Dateien
werden mit vollen Defaultwerten verlustfrei eingelesen. Regionen, Weltformat 2,
Seeds, IDs und Generatorversion 5 bleiben erhalten. Protokoll 17 repliziert
Vitalwerte und generationengebundene Respawns; Client und Server gemeinsam
aktualisieren. Spieler-Metadaten besitzen eine eigene Dirty-Revision im
bestehenden Journal, damit laufende Vitalwerte keine Terrain-/Wasser-/Collider-
Rebuilds ausloesen. Autosave erkennt sie auch bei pausierter Weltzeit.

660 Tests bestanden, ein bestehender Langzeittest ignoriert. Alle fuenf Cargo-
Pruefungen erfolgreich. Native Vulkan-Durchlaeufe in zwei Fenstergroessen mit
echtem Integrated Server, allen Bissstufen, Essen, Erschoepfung, Sturztod und
UI-Respawn erfolgreich; HUD-/Tod-/Respawn-Aufnahmen geoeffnet. Headless-
Serverstart erfolgreich. Keine neue FPS-/RAM-Paritaetsmessung behauptet.
Regeln, Tests, Art-Prompt, Reproduktion und bekannte Grenzen:
[Phase-43-Bericht](tools/benchmarks/results/survival-2026-10-17/README.md).

## 44. Verschiebbares Survival-HUD, Gamemode Und Zeitliche Schattenfilterung

Kompaktes TiltUI-HUD nach Referenz mit rosa Gesundheitsbalken, gelber Ausdauer
und Keule/Hunger-Prozentwert; Standard oben links. Graphics besitzt einen
HUD-Editor mit Ziehen, Reset, Cancel/Escape und Save. Relative Position in der
bestehenden graphics.toml, nur der Statuscontainer beweglich; Hotbar unveraendert.
Die Werkzeugleiste weicht dem Container aus und hat feste, einzeilige Buttons.
Speicherfehler bleiben sichtbar, andere Options-Entwuerfe unangetastet.

`/gamemode survival|0|creative|1` mit Vorschlaegen, Servervalidierung und eigener
UUID-Berechtigung. Integrated-Besitzer erhalten sie automatisch. Spieler-Overrides
bleiben nach Reconnect/Save erhalten; Inventar, Abbau, Flug und Survival folgen
dem eigenen Modus. Neue Eingabegeneration verhindert verspaetete Flugpakete beim
Wechsel. PLY3 liest bestehende PLY2-/PLYR-Spielerdaten kompatibel ein.

Bevy-TAA und zeitliche Schattenfilterung fuer Mittel/Hoch, engere Nahkaskade und
einheitliche Qualitaetskonfiguration. Keine groesseren Schattenkarten, keine
Dependency-Version geaendert; Bevy-Feature bevy_anti_alias aktiviert. Kameraspruenge
und Uhrzeitwechsel setzen den Verlauf zurueck. Niedrig/Aus bleibt ohne TAA.

668 Tests erfolgreich, ein bestehender Langzeittest ignoriert; fmt, check,
clippy mit -D warnings und Workspace-Build erfolgreich. Nativer Vulkan-HUD-Test
mit Integrated Server, Ziehen/Speichern/Reload und Gamemode-Rundlauf sowie
25 Licht-/Unterwasseransichten inklusive weiterlaufender Sonne erfolgreich.
Aufnahmen visuell geprueft, Headless-Start erfolgreich. Kein Langzeit-FPS-/VRAM-
Paritaetsnachweis; weitere TAA-/Transparenzabnahme bleibt im TODO.
[Phase-44-Bericht](tools/benchmarks/results/hud-mode-shadows-2026-10-17/README.md).

## 45. Rezeptmontage, Werkbank, Werkzeuge Und Groessere Slots

Gemeinsamer validierter Rezeptkatalog mit stabilen Namenraum-IDs, geformten und
formlosen Rezepten, Stueckzahlen und additiven TOML-/JSON-Moddateien. 18 erste
Rezepte verbinden Holz/Stein/Sand mit Planks, Staeben, Werkbank, Truhe und vier
Werkzeugen. Der Server liefert den Katalog vor Spielbeginn; Verbrauch/Ausgabe
sind atomar und revisionsgeprueft, auch fuer Cursor und Mehrfachherstellung.

Platzierbare Werkbank (28) mit eigenem statischen Spezialmodell und TiltUI-
Rezeptmontage statt verpflichtendem 3x3-Raster. Suche, Zutaten, Ergebnis,
Menge, Craft Max, Cursorfertigung und Fehlerzustaende; persoenliches 2x2 bleibt
ueber dieselbe Registry erhalten. Reichweite, freie Sicht und Werkbankbestand
werden fuer jeden Vorgang autoritativ erneut geprueft. Truhe (29) ist ebenfalls
herstellbar/platzierbar; ihr Containerinventar bleibt naechster Roadmap-Punkt.

Holz-/Steinspitzhacke, Steinaxt und Steinschaufel mit 3D-Vorschauen und -Drops,
JSON-Beschreibungen und serverseitigem Werkzeugbonus beziehungsweise
Materialanforderungen. Normale Bloecke erhalten keine Modell-Assets.
Inventar-, Hotbar- und Craftingslots sind 25 Prozent groesser, inklusive
responsiver Masse; angepasste Panelbreiten und korrekte Zutatenvorschauen.
Gleiche glTF-Loader-Einstellungen fuer Szenen und Materiallabels verhindern
Source-Verlust beim Nachladen der neuen Spezialmodelle.

Native TiltUI-Pruefung mit Integrated Server: Handherstellung/Craft Max,
echtes Platzieren der Werkbank, vier Werkzeuge, Truhe und binaeres Save/Reload.
Zwei-Spieler-Tests pruefen getrennte Zutatenbestaende, Replay und verschwundene
Werkbank; weitere Tests decken volle Inventare, Cursor, Muster und Werkzeugabbau
ab. 680 Tests erfolgreich, ein bestehender Langzeittest ignoriert; fmt, check,
clippy mit -D warnings und Workspace-Build erfolgreich. Headless-Start und
native Vulkan-Abnahme mit Save/Reload erfolgreich. Ein Zwischenlauf traf eine
belegte Testport-Kombination; der vollstaendige Wiederholungslauf besteht.
Abnahme/Logs: [Phase-45-Bericht](tools/benchmarks/results/crafting-2026-10-17/README.md).

## 46. 3x3, Einzelne Assetdateien Und Ausgerichtete Moebel

Werkbank mit echtem 3x3-Rezeptmuster, automatischer oder manueller Belegung,
Mengenwahl, Cursor und Craft Max. Das bestehende 2x2 und die atomare
Servertransaktion bleiben erhalten. Rasterfelder verweisen auf echte
Inventarslots; wiederholte Quellen werden summiert, keine Item-Duplikate.
Raster, Ergebnis und ein eigener scrollbarer Vorrat sind gleichzeitig bedienbar.

18 einzelne Rezept-JSONs unter assets/crafting; terra/recipes entfernt.
Items, Props und Kategorien ebenfalls in einzelnen Dateien. Block-JSONs
steuern hardness, resistance, required_tool und mining_level; der Headless-
Server liest dieselben Mechanikdaten und liefert die Animationsdauer.

Neue Werkbaenke belegen zwei horizontale Zellen entsprechend der Blickrichtung;
Freiraum, Boden und Spielerkollision werden vorab geprueft. Abbau jeder Haelfte
entfernt das ganze Objekt mit einem Drop. Einzeltruhen sind schmal; gleich
ausgerichtete seitliche Nachbarn verbinden sich zu einer Doppeltruhe, ohne
vertikale oder dritte Verbindung. Entfernen loest die andere Haelfte wieder ab.
Facing und Teilzustaende werden im vorhandenen Journal/Regionspfad gespeichert
und repliziert. REG2 liest alte REGN-Regionen kompatibel; IDs/Seed bleiben gleich.

Referenzorientierte statische Voxelmodelle mit gewoelbtem Truhendeckel,
Metallbaendern, Schloss sowie Werkbank-Ablage, Werkzeugen, Tuch und Laterne.
Nach aussen gerichtete eindeutige Modellflaechen; neue Holztextur mit echten
1254x1254 Pixeln, Materialmuster 64 -> 512 Pixel, vollstaendige Mips und
Anisotropie 8. Keine GLBs fuer normale Bloecke, keine Detailvoxel-Entities.

690 Workspace-Tests bestanden, ein bestehender Langzeittest ignoriert.
Native Integrated-Server-Probe mit 3x3-Fertigung, Zweizellen-Werkbank,
vier Werkzeugen, Einzel-/Doppeltruhen und REG2-Reload; Headless-Start erfolgreich.
Truheninhalt und Deckelanimation bleiben offen. Kein allgemeiner FPS-/VRAM-
Paritaetsnachweis oder vollstaendige Flimmerfreiheit durch Einzelbilder behauptet.
[Phase-46-Bericht](tools/benchmarks/results/furniture-grid-2026-10-17/README.md).

## Phase 47: Gemeinsames Inventar, Rezepte Und Holzvarianten

Einheitlicher TiltUI-Inventarrahmen fuer Survival und Creative mit rechtem
Itembrowser, Kategorien, Suche, Seiten und Rezeptinspektor. Die separate
Creative-Katalogseite ist entfernt. Creative gibt ein Item bzw. mit Shift
einen Stack zuerst in freie Hotbarslots; Survival zeigt echte Serverrezepte.

Persoenliches 2x2 und Werkbank-3x3 verwenden Mengenreservierungen auf denselben
Inventarslots. Vorhandene Zutaten werden bei passender Station automatisch
eingeordnet. Fehlende Zutaten pulsieren rot und verwenden den echten TiltUI-
Signal-Lost-Filter. Fertigung ueber Ergebnis-/Shift-Klick und Rasterstacks,
ohne Plus-/Minus- oder Craft-Max-Schaltflaechen. Mengen, mehrfach verwendete
Quellen und Cursor werden atomar serverseitig geprueft. Reservierungen bleiben
bei Sammeln, Wheel, Shift-Transfer, Q/Shift-Q und Revision-Retries geschuetzt.
Rasterwechsel warten auf ausstehende Bestaetigungen. Werkbank-Tooltips repariert.

`struct_block` aus gemeinsamer Block-JSON erhaelt Gras unter Moebeln, waehrend
ueberlappende Props weiterhin entfernt werden. Gemeinsame Holzrezepte erlauben
gemischte Bretter; die verbrauchte Mehrheit bestimmt die gespeicherte Holzart.
Birken-/Fichtenmoebel behalten ihre Farbe in Vorschau, Inventar, Platzierung,
Drops und Reload. Gemischte Doppeltruhen zeigen beide Holzarten innerhalb
derselben Materialbatches. REG3 liest REG2/REGN kompatibel; Protokoll 21.
Birkenlaub ist eichengruener und besitzt eigene schmalere Micro-Kronen.

727 Workspace-Tests bestanden, ein bestehender Langzeittest ignoriert.
Native Survival-/Creative-Ansichten mit echten OS-Mausgesten, Werkbank-Tooltip,
kompaktem Zweispaltenlayout, Birkenmoebeln und binaerem Reload geprueft;
Dedicated Server startet ohne Renderer. Alle fuenf Cargo-Pruefungen erfolgreich.
Truheninhalt und Ofen-Backend bleiben offen; der gemeinsame UI-Rahmen
taeuscht diese Funktionen nicht vor. Keine ungemessene FPS-/Speicherparitaet.
[Phase-47-Nachweise](tools/benchmarks/results/inventory-browser-2026-10-17/README.md).

## Phase 48: Crafting-Mausgesten, Rezeptuebernahme Und Bodenkontakt

2x2-/3x3-Raster erhalten Links-/Rechtsziehen, Halbieren, Sammeln, Shift-
Rueckgabe und Mausradtransfers. Exakte atomare Cursortransfers verwenden
weiterhin dasselbe autoritative Inventar und Mengenreservierungen, ohne
zweiten Itembestand. Weitere Grid-Eingaben warten begrenzt auf Serverantworten.

Rezeptklicks aendern das Raster nicht mehr. Ein Plus im Rezeptdialog uebernimmt
verfuegbare Zutaten an der passenden Station und schliesst die Vorschau;
Seitennavigation erscheint nur bei mehreren Rezepten. Der Itembrowser steht
rechts am Fenster auf 95 Prozent Hoehe und Desktopabstand 100 UI-Pixel,
mit kleineren Abstaenden fuer kompakte Ansichten.

Statische Werkbank-/Truhenmodelle auf Zellenboden abgesenkt: kein kuenstlicher
Luftspalt unter Fuessen/Boden. Neue atomare Transfers erfordern Protokoll 22
auf Client und Server; Weltformat, Generator, Saveformate, IDs und Seeds gleich.
Containerinhalte und Ofen-Backend bleiben ausdruecklich offen.

736 Workspace-Tests bestanden, ein vorhandener UDP-Langzeittest ignoriert.
Native Crafting-/Moebelansichten und echte OS-Grid-Drag-/Sammelgesten am
Integrated Server geprueft; Headless-Server-Smoke erfolgreich. Keine neue
FPS-/Speicherparitaet behauptet.
[Phase-48-Nachweise](tools/benchmarks/results/crafting-input-2026-10-17/README.md).

## Phase 49: Weltinventare, Scrollbare Ansichten Und Modellabbau

Truhen verwenden das vorhandene Inventarmodell mit 18 Slots je physischer
Haelfte und 36 Slots bei verbundener Doppeltruhe. Spieler behalten 27 Slots
einschliesslich Hotbar. Klick, Halbieren, Drag-Verteilung, Shift-Transfer,
Sammeln und Ctrl+Mausrad funktionieren ueber gemeinsame atomare Gesten; normales
Mausrad scrollt den Slotbereich, ohne Items zu transferieren. Cursor,
Stacklimit und stabile Slotindizes bleiben erhalten.

Private Lightyear-Snapshots gehen nur an berechtigte Containerbetrachter.
Reichweite, Sichtlinie, Weltzellen, Verbindung und Inventarrevisionen werden
serverseitig geprueft; konkurrierende veraltete Zugriffe erhalten Konflikte
statt Teiltransfers. Ein gemeinsamer Journal-Commit schreibt Spieler und
Haelften zusammen, erhoeht Metadaten einmal und invalidiert keine Terrainmeshes.
Nichtleere physische Truhen koennen weder entfernt noch ersetzt werden.
Leere Haelften duerfen sich trennen, ohne den Inhalt der anderen zu verlieren.

REG4 speichert Container nach Zellposition im bestehenden atomaren Regions-
Commit, nicht in einer neuen Speicherarchitektur. Explizite Leser erhalten
REG3, REG2 und REGN; duplizierte, falsch zugeordnete oder ungueltige Inhalte
werden abgewiesen. Spielerdateien bleiben PLY3, Weltformat 2, Generator 5,
Seed und IDs unveraendert. Client und Server verwenden gemeinsam Protokoll 23.

Spieler-, Werkbank- und Truhenansichten bleiben im Fenster zentriert und
erhalten scrollbare Slotbereiche. Der 95 Prozent hohe rechte Itembrowser
nutzt nach 100 logischen UI-Pixeln Abstand die restliche Breite; kompakte
Fenster passen Abstand und Anordnung an. Seitenzaehler stehen zwischen den
Pfeilen. Rezeptnavigation erscheint nur bei Alternativen; das Plus uebernimmt
weiterhin ausdruecklich Zutaten und schliesst die Vorschau.

Acht dezente Ausruestungs-Silhouetten stehen links neben dem Charakter als
2x4-Anordnung. Sie sind klar deaktiviert: echte Ausruestungsitems, gespeicherte
Equipment-Slots und Werteboni sowie ein Ofen-Backend bleiben offen.

Shaped-Matching verwirft rechts/unten abgeschnittene Muster und ungueltige
aeussere Leerreihen, bewahrt aber gueltige Verschiebungen und innere Luecken.
Shapeless-Matching loest ueberlappende generische und exakte Zutaten ohne
Abhaengigkeit von der Zellenreihenfolge. Reserviertes Crafting verbraucht exakt
die gepruefte Zuordnung. Vorhandene Truhen-, Werkbank- und Werkzeug-JSONs
bleiben shaped; keine neue Rezeptregistry.

Zweizellen-Werkbaenke besitzen eine gemeinsame Auswahlbox, Doppeltruhen
bleiben getrennte Objekte. Abbau importierter statischer Modelle zerlegt die
wirklichen Oberflaechen mit erhaltenen UVs, Farben und Zwischenraeumen in
budgetierte visuelle Teile; Vorbereitung asynchron fuer das aktive Nahziel.
Keine ausgefuellten Bruchwuerfel anstelle von Moebelmodellen und keine neuen
Gameplay-Voxel oder Einzelentities pro Modellvertex.

**Abnahme:** Container-UDP und REG4-Reload bestanden, ebenso native
Crafting-/OS-Mausgesten und Headless-Start mit 27 Sections. Native Werkbank-
und Truhenproben zeigen vier Modell-Bruchstufen ohne Wuerfeleffekte und stellen
nach Abbruch das Original wieder her. Alle fuenf Cargo-Pruefungen erfolgreich;
781 Tests bestanden, ein bestehender Langzeittest ignoriert. Abschliessender
Testlauf mit `--test-threads=2` nach Korrektur einer vorhandenen Survival-
Fixture-Race beim Eintreffen privater Inventardaten. Produktionsregeln bleiben
unveraendert. Finale native Inventarprobe mit sieben Aufnahmen, echter
OS-Maus, erzwungenem Scrollen bis zur letzten Slotreihe und erhaltenem
Itembestand bestanden. Acht Silhouetten, Zentrierung, Browserabstand und
Pagination nativ gesichtet. Der Dedicated-Abhaengigkeitsbaum enthaelt keinen
Renderer und keine UI. Keine neuen FPS-/Speicher- oder Langzeitparitaetsbehauptungen.
[Phase-49-Nachweise](tools/benchmarks/results/containers-2026-10-17/README.md).

## Phase 50: Inventar-Feinschliff, Stationslager Und Wachsende Oberflaechen

Rahmenloser Crafting-Reset, neutrale Warn-/Statustexte und Live-Stackvorschauen
beim Mouse-Tweaks-Ziehen durch Inventar-, Crafting- und Truhenslots. Vorschauen
veraendern keine echten Bestaende vor dem autoritativen Commit. Unter 30 Prozent
Gesundheit ergaenzt der echte TiltUI-Stoerfilter den vorhandenen roten Schadensschein.
Die Doppeltruhenansicht erhaelt mehr Hoehe; fuer kleine Fenster bleibt Scrollen.

Werkbaenke zeigen links die zuerst gebaute angrenzende Einzel-/Doppeltruhe.
Beide Werkbankzellen, private Zugriffsrechte, Reichweite, Sichtlinie, Topologie
und reservierte Craftingzutaten werden serverseitig geprueft. Platzierungsreihenfolge
wird dauerhaft in REG5 gespeichert; REG4/REG3/REG2/REGN bleiben lesbar. Alte Saves
ohne historische Ordinale verwenden einen ehrlichen stabilen Koordinaten-Tiebreak.
Vier Bretter stellen die Werkbank nun shapeless her, einschliesslich 2x2-Anordnung;
Truhen und Werkzeuge bleiben shaped, dominante Holzfarbe erhalten.

Aus denselben Asset-Modellbausteinen exportierte Teilvolumen ersetzen die
Vollzellen-Hitbox fuer Truhen und Werkbaenke bei Auswahl, Sichtlinie und Kollision.
Vollbloecke behalten volle Zellen. Abbauteilchen brechen an echten
Modelloberflaechen heraus; der Grasblock bekommt eine gruene Inventaroberseite.
Die urspruengliche Modell-Auswahlkontur wird beim Abbauen verborgen, statt
bereits entfernte Teile weiterhin als intaktes Gitter zu umzeichnen.

Exponiertes Dirt neben Grass erhaelt nach Wartezeit vier sichtbare gespeicherte
Grasstadien und wird danach zum echten Grass-Block. Autoritative Simulation,
maximal 256 aktive Zellen, acht Aenderungen je Sekunde und amortisierte Discovery.
Vollbedeckung, Feuer, Schnee oder Ueberflutung stoppen Wachstum; strukturelle
Objekte ersticken es nicht. Kosmetische Stadien belegen keine Gameplay-Pflanzenplaetze,
verschieben keine Blumen und sind nicht erntbar. Bestehende Grasbueschel werden
nicht dupliziert; Fortschritt pausiert beim Unload und nutzt das vorhandene Journal.

Grosse Pilzkappen erhalten verbundene haengende Myzelbaender in begrenzten
Near-/Middle-Batches. Ein deterministisches 24-Prozent-Ticket geeigneter Baeume
erzeugt kleine rote, braune oder weissgraue Stammpilzcluster bei freier Ankerflaeche.
Bestehende Prop-IDs, Praezisions-Picking, Einzelabbau und Reload-Persistenz bleiben
die Grundlage; Kandidatensuchen werden pro PreparedColumn wiederverwendet.

Client und Server benoetigen gemeinsam Protokoll 24. Weltformat 2, PLY3,
Generator 5, Block-/Welt-IDs, Seeds und Dependencies bleiben unveraendert.
**Abnahme:** Alle fuenf Cargo-Pruefungen bestanden; 816 regulaere Tests erfolgreich,
ein bestehender UDP-Langzeittest ignoriert, final mit `--test-threads=1` nach
einem zwischenzeitlichen Test-Startfehler durch belegten TCP-Port. Native Inventarprobe mit echter
OS-Maus und neun GPU-Aufnahmen, Crafting-/Stationslager mit unverdeckten
36 Slots, reservierten Transfers und REG5-Reload, kritischer Gesundheitsfilter,
Respawn sowie sechs Oekologie-Aufnahmen bestanden. Alle vier Grasstadien wurden
in Echtzeit beobachtet; nach 55.355 Sekunden seit Fixturestart war der finale
Grasblock erreicht, einschliesslich vorheriger Pilzansichten und Streaming.
Headless-Start mit 27 Sections und Dedicated-Abhaengigkeitsbaum ohne Renderer/UI
bestaetigt. Relevante Inventar-, HUD-, Pilz- und Wachstumsbilder nativ gesichtet.
Beide abschliessenden Moebelabbauproben bestanden mit vier Geometriestufen,
Wiederherstellung nach Abbruch und zwingend beobachteten Bruchteilchen:
vier an der Werkbank und zehn an der Truhe, ohne intakten Auswahlkaefig beim Abbau.
Drei doppelte Screenshotauftraege erzeugten Bevy-Readbackwarnungen, keine
beobachteten UI-Runtimewarnungen. Kein neuer FPS-/GPU-Speicher- oder Langzeitnachweis.
[Phase-50-Nachweise](tools/benchmarks/results/ecology-inventory-2026-10-17/README.md).

## Phase 51: Erze, Kristalle Und UI-Feinschliff

Die Palette erhaelt sechs normale Erzmaterialien mit stabil angehaengten IDs
30..35, acht Mineral-Ressourcen und eine Eisenspitzhacke. Tiefen-/gesteinsabhaengige
Lagerstaetten nutzen die vorhandene Weltgenerierung; Salzregionen reichern
bestehende Hoehlenwaende an, ohne einen weiteren Hoehlengenerator einzufuehren.
Einzelne JSON-Dateien steuern Material, Werkzeuganforderungen, Beschreibungen
und Rezepte. Erz-GLBs und einzelne Kristall-Entities sind nicht erforderlich.

Facettierte, gebatchte Einschluesse behalten geschlossene Felskerne und stabile
Primaermaterialien im Fern-LOD. Sichtbare Spitzen werden beim Picking ihrem
tragenden Erzblock zugeordnet. Abbau und Ressourcendrops verwenden weiterhin
den gemeinsamen autoritativen Pfad. Zwei Roherz plus Kohle ergeben vorlaeufig
an der Werkbank einen Barren; ein echtes Ofen-Backend bleibt offen.

Erzgenerierung ist fuer neue Welten mit eigenem Featurestand 1 aktiv. Alte
Welten ohne Metadaten bleiben bei aus/0; Retrogen wird nicht angeboten. Der
Dedicated-Startpfad erhaelt bei fehlender alter globaler Option die gespeicherte
Weltpolitik. Weltformat 2, Generator 5, REG5/PLY3, Seeds und vorhandene IDs bleiben
erhalten. Client und Server wechseln gemeinsam auf Protokoll 25.

Verknuepfte Truhenslots verwenden normale Inventargroessen und responsive
Seitenraster. Tooltips bleiben scrollfrei und umbrechen lange Kennungen.
Schadensfeedback beleuchtet die Bildraender statt die gesamte Ansicht; kritisch
kommen pulsierende Raender und der vorhandene Stoerfilter zusammen.
Verankerte Pilzfasern bis 2.5 m und ueber 2.4 s weich wachsende Grasstadien
erweitern die bestehenden Detail-/Wind-Batches, nicht die logische Blockaufloesung.

**Abnahme:** Alle fuenf Cargo-Pruefungen und 845 Workspace-Tests bestanden,
ein vorhandener 180-s-UDP-Soak ignoriert. Native Erzprobe mit sechs, Oekologie
mit neun, Survival mit zwoelf, Crafting mit 22 und Inventar mit neun Aufnahmen
bestanden: insgesamt 58 erfolgreiche GPU-Aufnahmen.
Headless-Start mit 27 Sections ohne Renderer/UI bestaetigt; verknuepfte Slots und
scrollfreie Werkbank-/Hotbar-Tooltips sowie Schadensraender nativ gesichtet.
Zwei doppelte Crafting- und ein doppelter Survival-Screenshotauftrag erzeugten
Bevy-Readbackwarnungen; keine Shaderfehler beobachtet.
CPU-Featurevergleich im gleichen Dev-Build bei
4375 Sections: Headless +16.08% Oberflaeche und +10.10% Untergrund, Render-
Vorbereitung +12.07% und +9.56%; dies sind Kosten, keine FPS oder historischen
Vorherbuildwerte. Keine neue FPS-/Speicher- oder vollstaendige Referenzqualitaet
behauptet. GPU-/Speicher-Langzeitmessungen und seltene Interpolationsgrenzen
bleiben im begleitenden Backlog; naechster nummerierter Schritt ist Tier-KI.
[Phase-51-Nachweise](tools/benchmarks/results/ores-growth-2026-10-17/README.md).

## Phase 52: Cliffted Desert, Hoehlentypen Und Renderkosten

Neue Welten erhalten Cheese-Kammern, breite Dripstone-Gewoelbe und Spaghetti-
Gaenge im gemeinsamen Hoehlenfeld. Cliffted Desert entsteht als Mesaregion
innerhalb starker Desert-Klimaregionen mit normalem Desert-Saum, Sand und tieferem
Sandstein. Zehn Prozent geeigneter Hoehlenregionen besitzen eine besonders
breite Salzkammer. Ausserhalb dieses Bioms wird im neuen Vertrag kein Salz
erzeugt. Facettierte Salz-/Tropfsteinspitzen nutzen bestehendes Section-Batching
und das gemeinsame praezise Picking, ohne einzelne Detail-Entities.

Fichten besitzen einen eigenen dunklen Stamm mit eigenem Plankenrezept.
Die angehaengten IDs 36 (SpruceLog) und 37 (Dripstone) verschieben keine alten
IDs. Stein und Erz-Felskerne teilen waermere, gebrochene Koerner sowie matte
Roughness-/Reflectancewerte; Texturen behalten ihre Mipmaps. Kaltes
Hoehlen-Umgebungslicht ist damit nicht vollstaendig neutralisiert.

Bevys Zweiphasen-GPU-Occlusion-Culling nutzt ab Sichtweite 12 den bestehenden
Depth-Prepass; kleine Sichtweiten behalten ihren guenstigeren einphasigen Pfad.
Fernes Gras behaelt alle Pflanzen mit 32 statt 80 Vertices, gepackte Fernprops
benoetigen regulaer 28 statt 48 Bytes pro Vertex. Erzdetails sind auf maximal
52 statt 104 Dreiecke je freiem Face reduziert. Neue Salzwaende verwenden die
ohnehin berechnete Kammerdichte statt des dichten alten Salzsuchrasters.
Nah-/Mittelgras, Wind, Interaktion und Sichtweite bleiben bestehen.

Gespeicherte `geology_revision = 1` kennzeichnet neue Welten; alte Dateien ohne
Feld bleiben Revision 0 mit ihrer bisherigen Geologie und ihren Baumaterialien.
Kein automatischer Retrogen. Generator 5, Weltformat 2, REG5, PLY3, Seed und
bisherige IDs bleiben erhalten; Client und Server verwenden gemeinsam Protokoll 26.
Abnahme: Alle fuenf Cargo-Pruefungen und 853 Workspace-Tests bestanden, ein
vorhandener UDP-Soak ignoriert. Native Hoehlen-/Biomansichten, Inventargesten
und Headless-Start bestanden. Im identischen zeitfixierten 32er-Messpaar bei
3828x2043 Pixeln steigen FPS 20.13 -> 55.51, p99 sinkt 67.87 -> 23.30 ms,
Peak-VRAM 10.40 -> 7.47 GiB, bei unveraendert 401 244 dargestellten Props.
Im frischen 8er-Paar bleiben FPS 164.53 -> 164.60; p99 ist 8.49 -> 8.74 ms
leicht schlechter. CPU-Worker mit alter Geologie sind rund fuenf Prozent
langsamer, kein allgemeiner CPU-/Frame-Paritaetsgewinn behauptet.
Konkrete Messwerte, Tests, native Aufnahmen und verbleibende Grenzen stehen im
[Phase-52-Abnahmebericht](tools/benchmarks/results/cliffted-caves-2026-10-17/README.md).

## 53. Lokale Testprofile, Stein, Erzadern Und Grosse Tropfsteine

`--profile NAME` waehlt eine eigene persistente UUID unter `configs/profiles/`.
Standardprofil, Online-Vertrauensgrenzen und serverseitiger Duplicate-UUID-Schutz
bleiben bestehen. Zwei native Clients mit verschiedenen Profilen sind gemeinsam
verbunden; Bewegung und sichtbares Remote-Modell wurden geprueft.

Stein und sechs Erz-Felskerne teilen ein neues mattes HD-Albedo mit vollstaendigen
Mips. Schmale verzweigte Bruchbetten und Mineraleinlagen ergaenzen die bestehenden
Kristalle; Rendering und Picking verwenden dieselbe Geometrie. Kein Loch im
tragenden Cube und keine zusaetzliche Entity pro Ader.

Neue Geologie-2-Welten erhalten gestuetzte, mehrblockige Boden-/Deckenformationen
mit facettierten Spitzen. Salz nutzt den neuen hellen Koerperblock 38 und droppt
die vorhandene Salzressource; Formationsblock 39 teilt das Material der normalen
Dripstonewand 37, aber nicht deren kleine vereinzelte Anbauten. Weltvertraege 0/1,
IDs 0..37 und Generator 5 bleiben
erhalten; Client und Server benoetigen gemeinsam Protokoll 27.

Nahe Schatten werden in groben Section-Schalen gebatcht; sichtbare Geometrie,
Laubmasken und Wind bleiben bestehen. Leere Uploadqueues vermeiden wiederholte
Bestandsscans. Alle fuenf Cargo-Pruefungen mit 864 Tests, native Erz-/Hoehlen-/
Animations-/Zweiclient- und Headless-Proben sind bestanden. Im ruhenden 8er-Paar
steigen FPS 162.85 -> 205.85, im 32er-Paar 55.30 -> 63.33; dort sinkt p99
26.63 -> 19.00 ms. Zusaetzliche Kosten: +121 MiB Peak-RSS und +0.81 s Ladezeit
im grossen ruhenden Paar. Der 120-s-Streamingvergleich verbessert FPS nur von
33.83 auf 36.17; diese Restkosten bleiben offen. Keine allgemeine FPS-/
Speicherparitaet oder vollstaendige Referenzqualitaet behauptet.
[Messbedingungen, Rohlogs, Bildprompt, native Nachweise und Grenzen](tools/benchmarks/results/minerals-performance-2026-10-17/README.md).

## 54. Kristallgruppen Und Gerichtete Salzspitzen

Erze tragen breitere facettierte Kristallgruppen mit hohen Hauptspitzen,
kleineren Nebenarmen, Mineraleinschluessen und schmalen Rissen. Cyaner Diamant,
roter Rubin, warmgraues Eisen und warmweisses Salz orientieren sich an der
Referenz. Das gemeinsame Mineralmuster folgt den Facetten statt sich an langen
Seiten zu strecken. Nicht-Salz-Erzdetails benoetigen am voll exponierten Block
maximal 288 statt 384 Near-Dreiecke; Sections und Materialien bleiben gebatcht.

Grosse Salzcluster wachsen vom Boden 2 bis 3.85 m hoch und von der Decke bis
maximal 10 m herunter. Freiraum, gegenueberliegende Cluster, Blockaenderungen
und sectionuebergreifendes Picking werden gemeinsam beruecksichtigt. Nur
betroffene Salz-Renderbatches werden bei entfernteren Clearance-Aenderungen
neu komponiert; Kollisionsdaten bleiben gueltig. Die Bewegungskollision bleibt
der logische Traegerblock. Neue Geologie-3-Welten ersetzen die alten massiven
Salzkegel durch diese Gruppen; gespeicherte Vertraege 0/1/2 werden erhalten.
Client und Server benoetigen gemeinsam Protokoll 28.

Alle fuenf Cargo-Pruefungen erfolgreich, 869 Tests bestanden, ein bestehender
UDP-Soaktest ignoriert. Native Erz-/Geologieaufnahmen und Headless-Serverstart
geprueft. Das einzelne 60-s-Waldpaar misst 149.12 -> 158.19 FPS; die
CPU-Render-Vorbereitung im gleichen Salzterrain steigt dagegen 0.879671 ->
0.903830 s. Kein Nachweis allgemeiner FPS-/Speicherparitaet oder dichter
Salzhoehlen-GPU-Paritaet; die Messgrenzen bleiben offen dokumentiert.

[Tests, native Aufnahmen, Messungen und Grenzen](tools/benchmarks/results/crystal-clusters-2026-10-17/README.md).

## 55. Tier-KI, Hoehlenbewuchs Und Chatverlauf

Schafe, Kuehe, Schweine und Fische mit stabilen Habitat-IDs, autoritativem Spawn,
budgetierter lokaler Navigation, Ruhe/Grasen/Flucht und Schwimmen integriert.
Echte Wasserzellen, Stroemung, Blockaenderungen und Wasserverlust werden geprueft.
Geteilte Voxelmodelle, gegliederte Animationen, synthetisierte Tierlaute und
serverseitig validierte Angriffe/Fuetterung nutzen bestehende Renderer-, Audio-,
Inventar- und Drop-Pfade. REG6 speichert Populationen und tote Identitaeten;
fruehere Regionsformate bleiben lesbar. Zwei UDP-Clients, Late Join, Treffer,
Drops, Reload und begrenzte Herdensimulation sind getestet.

Tropfstein nutzt die Salz-/Mineralpipeline mit seltenen Deckengruppen bis 14 m,
breiteren verankerten Spitzen, ausgeduennten kleinen Einschluessen und gemeinsamem
Picking. Geologievertrag 4 erzeugt zusammenhaengende Erdflecken; alte Geologie
wird nicht umgebaut. Chat-Pfeile blaettern durch gesendete Eingaben und erhalten
den ungesendeten Entwurf. Grenzen, native Ansichten und Messwerte stehen im
[Phase-55-Bericht](tools/benchmarks/results/creatures-caves-2026-10-18/README.md).

## 56. Herdenspawns, Tierbeduerfnisse Und Aufgenommenes Audio

Dichtere deterministische Gruppen derselben Art auf geeignetem Land und in
tieferem Wasser. Tiere besitzen persistente Nahrung und Wasser mit den neuen
Art-Maxima: Kuh 10/30/30, Schaf 7/35/25, Schwein 8/20/40, Fisch 3/--/--.
Budgetierte Ressourcensuche, Fressen einzelner Props und Trinken vom trockenen
Ufer integrieren sich in die bestehende KI und das EditJournal. Faustschlaege
verursachen nun auch in Creative einen Lebenspunkt. F3+E zeigt die echten Werte
des anvisierten Tiers in einem unabhaengigen TiltUI-Panel; die Tastenkombination
ist in Controls neu belegbar und oeffnet nicht gleichzeitig das Inventar.

Aufgenommene Tierstimmen und natuerliche Foley ersetzen synthetische Rufe.
Fressen, Schritte, Schuetteln, Schwimmen und Trinken besitzen eigene Cues;
Wind, lokale Kronen und Hoehlenraum steuern drei sanft gemischte Umgebungsspuren.
Ein gemeinsamer begrenzter Mixer verwendet die Spielerposition, Voxel-Verdeckung,
Live-Lautstaerke und einen weichen Unterwasserfilter. Bevy-WAV-Decoder aktiviert,
keine Dependency-Versionen angehoben. Audioquellen und Lizenzen liegen unter
assets/audio/CREDITS.md. REG7 liest REG6-Tiere mit anteilig umgerechneten Leben
und vollen Beduerfnissen; Client und Server benoetigen Protokoll 30.
895 Workspace-Tests, Formatierung, Check, strenges Clippy und Build erfolgreich.
Native Tieranzeige und Headless-Start geprueft. Der kurze Waldvergleich liegt bei
210.48 -> 209.48 FPS mit identischer Geometrie; kein Tierlast-/Langzeitnachweis.
Eine akustische Abnahme war ohne verlaesslichen Audioausgang nicht moeglich.
[Bericht und Nachweise](tools/benchmarks/results/animal-needs-audio-2026-10-18/README.md).

## 57. Naturkulisse, Materialschritte, Spuren Und Hoehlenstil

Vogel-/Insektenaufnahmen ersetzen die dauernden Overworld-Wind-/Laubschichten;
Tageszeit, Feuchte, Bewuchs und echte Hoehlendecken steuern den Mix. Spieler und
replizierte Landtiere verwenden gemeinsame streckenabhaengige Bodenkontakte.
Sechs Materialgruppen mit je zwei Aufnahmen und begrenzter Hoehlen-Schritthall
erweitern den vorhandenen Audiostream. Keine uebernommenen Spiel-/Modsounds.

Eine gemeinsame begrenzte Mesh-Entity stellt Sohlen-/Hufspuren in Sand und
Schnee dar. Spieler 10/22 s, Tiere 3/5 s, weiches Fading und Entfernen bei
Supportverlust/Unload. Ein eigener tiefenbasierter Hoehlen-WGSL-Pass samt
Ambient-/Belichtungs-/Nebelabstimmung laeuft nur in Hoehlen. Unterwasser bleibt
der bestehenden Wasserumgebung untergeordnet; kurze Sichtweiten und dichter
Nebel bleiben gueltig. Kein GI-/Hytale-Renderer oder physikalischer Raumhall.

Neue Welten verwenden Landschaftsrevision 1 mit bestehender Continentalness
sowie neuen Erosion-, Peaks/Valleys-, Jaggedness- und Surface-Lake-Feldern.
Geschlossene lokale Seen nutzen vorhandene Wasserzellen und Beckenformen.
Alte Weltdateien ohne das Feld bleiben auf 0; gespeicherte Dedicated-Vertraege
werden beim Neustart erhalten. Generatorversion, Seeds, IDs und Regionen bleiben
erhalten; Netzwerkprotokoll jetzt 31.

907 Workspace-Tests, Formatierung, Check, strenges Clippy und Build erfolgreich.
Native Spuren-/Ablaufpruefung, Hoehlen mit/ohne MSAA und Headless-Start geprueft.
60-s-Waldpaar: 210.89 -> 211.20 FPS, p99 7.009 -> 7.092 ms. Neue Landschaft im
kurzen Worker-Paar: reine Generierung 0.331 -> 0.344 s, Render-Vorbereitung
0.583 -> 0.590 s fuer 4375 Sections. Keine allgemeine Performance-Paritaet und
ohne verlaesslichen Audioausgang keine akustische Abnahme behaupten.
[Bericht und Nachweise](tools/benchmarks/results/soundscape-landscape-2026-10-18/README.md).

## 58. Hoehlenlicht, Fackeln, Kontakt-Foley Und Bienen

Blauer pauschaler Hoehlenlift und undefinierte Reihenfolge gegenueber TAA
ersetzt. Wirksame Bevy-Environment-Komponenten werden gemeinsam abgeblendet;
Aussenluft-Streuung wird in geschlossenen Hoehlen weich begrenzt. Gecachte
interpolierte Voxel-Kontaktabschattung mit diagonaler Edit-Invalidierung und
Low-/Medium-/High-/Ultra-Optionen ergaenzen die vorhandene PBR-Pipeline.
Beidseitige opake Schattenproxies verhindern Lichtlecks durch nur innen
sichtbare Decken, auch ohne darueber geladene Terrainoberflaeche.
High/Ultra bieten SSAO und dezentes Bloom; Ultra begrenzten raeumlichen
Sonnennebel. Kein RGB-Floodfill oder indirektes GI als fertig ausgegeben.

Fackel als eigener statischer Custom-Block 40 mit Teilkollision und Rezept
Stock + Kohle; warmes, sanft schwankendes 18-m-Licht. Validierte JSON-Emission
und ein begrenzter Schattenlichtpool fuer Fackeln und farbige Kristalle.
Der Lichtcache beruecksichtigt Journal-Edits vor fertigen Meshwechseln.
Landungen, staerkere Aufpralle sowie bestaetigte Abbau-/Setzereignisse verwenden
materialabhaengige Foley im bestehenden Mixer mit Hoehlen-/Unterwasserbehandlung.

Kleine Bienen mit geteilten Body-/Fluegelmeshes, Flug, Landen, Nektarbesuch und
Krabbeln erweitern die autoritative Tier-KI. Sparse Waldnester im vorhandenen
Prop-/Journal-/Regionssystem, maximal ein Kandidat je vier Chunkspalten und
zwei Bienen je Nest. Eigene synthetisierte Flug-/Landecues; Honigproduktion
und globale Navigation bleiben offen. Terrain-Generator unveraendert.
Ein nativer glTF-Laderennfehler wurde durch Wiederverwendung vorhandener
PBR-Unterhandles und Weglassen unbenutzter Paletteneintraege behoben.

Explizite opt-in Testhoehle mit Felskammer, Sonnenoeffnung, Tropfsteinen,
statischem See, Fackeln, Kristallen und dunklem Tunnel. Tests, native Ansichten,
Messungen und konkrete Grenzen stehen im
[Phase-58-Bericht](tools/benchmarks/results/cave-lighting-bees-2026-10-18/README.md).

## Phase 59: Lebendige Tierwelt, Fackeln Und Zielanzeige

Animiertes selbstleuchtendes Fackelfeuer erweitert die vorhandene Material-
Pipeline. Boden- und geneigte Wandmontage teilen Modell-, Treffer- und
Lichtpose; Stuetzverlust entfernt die Fackel mit einem 3D-Drop. Geschlossene
Relief-Schattenhuellen verhindern schwarze Vertiefungen und Lichtlecks.
Hoehlen erhalten etwas Grundlicht, Pflanzen nur weiche kurze Bodenkontakte.

Vierer-Bienengruppen mit neuem kleinen Koerper, Stachel und Sammelanimation,
endlicher Blumennektar mit dreiminuetiger Regeneration, Nacht-Rueckkehr und
persistente Nestvorraete mit Honigreifung. Landtiere legen sich nachts hin;
kurze Ausweichwege und Schwimmen erweitern die bestehende KI. Fische springen
gelegentlich. Koerperbasierte Splash-/Schwimmspuren und Saeugetier-Atemblasen
verwenden begrenzte gemeinsame Effektmeshes. Neue Foley-Cues und leiseres
Vogelambiente erweitern den vorhandenen Audiomixer.

Einzelne Mob-JSONs konfigurieren die Arten. Die neue TiltUI-Zielkarte zeigt
Namen, Registry-ID, Werkzeug/Stufe, Abbaubalken beziehungsweise Mobstatus und
Leben. Blumen-/Nestanzeigen lassen sich in ihren JSONs als Balken oder Text
festlegen. Oekologie bleibt im bestehenden Journal/Regionssystem: REG8 mit
REG7-Leser, kleine laufende Oekologie-Uhr im IDX3-Weltindex statt unnoetiger
Neuschreibvorgaenge unveraenderter Regionen. IDX2/INDX bleiben lesbar.

Native Ansichten, Messbedingungen und konkrete Grenzen stehen im
[Phase-59-Bericht](tools/benchmarks/results/living-world-2026-10-18/README.md).

## Phase 60: Weltgrenzen, Tierverhalten Und Bedienung

150-m-Habitate mit fairen, nahbereichs-priorisierten Proben fuer alle Spieler.
Globales Wildtierlimit standardmaessig 256, Dedicated-Override und persistenter
Einzelspieler-Slider; gespeicherte Besitzerzuordnungen zaehlen nicht mit.
Bienen reservieren Blueten, bleiben hoechstens 100 m vom Nest entfernt und
verschwinden auch bei Tageslieferungen kurz darin. Schlafende Landtiere
bleiben bei vorbeigehenden Spielern ruhig, falten Beine und schliessen Augen.
Blockierte Tiere pausieren statt staendig zu kreiseln. Riverfin ergaenzt
Clownfish; aktive Fische koennen moderate Stroemung ueberwinden.

Neue Welten erhalten den gespeicherten Vertrag Y -128..356, zwei Grundstein-
schichten, Hoehlenende vor -120 und absolute Erz-Hoehengrenzen. Mehr trockene
Eingaenge und Schluchten respektieren Wasserbecken und Flussueberhaenge.
Grundstein fuegt ID 41 hinzu, ist in Survival unzerstoerbar und nutzt die
vorhandene Cube-Pipeline. Kein automatischer Retrogen historischer Welten.
Hoehlen-Grundlicht ist 40 Prozent reduziert; budgetierte Fackel-Nahlichter
hellen Traegerbloecke auf. Sonne ohne Spitzen, Mond unveraendert.

Chat blendet nach fuenf Sekunden weich aus; Pfeile waehlen bei Befehlen
Vorschlaege, sonst Verlauf. Spielernamen lassen sich mit @ ergaenzen;
Erwaehnungen sind fett/pink und benachrichtigen den Betroffenen akustisch.
HUD Layout verschiebt nun auch Chat und Info-Karte, nicht die Hotbar.
Strg-Tap aktiviert gerichtetes Tauchen, Leertaste beendet es; gemeinsame
Vorhersage/Autoritaet und eine neue Avatar-Schwimmpose bleiben gekoppelt.
Drops fliegen vor der Inventaruebernahme serverseitig zum Spieler.
Schritt-Foley und dezente, begrenzte UI-Toene sind ueberarbeitet.

REG9 erhaelt explizite REG8-/REG7-Leser. [Phase-60-Pruefbericht](tools/benchmarks/results/world-life-2026-10-18/README.md)
enthaelt konkrete Tests, native Ansichten, Messbedingungen und Grenzen.
950 Workspace-Tests und separater 180-s-Zwei-Profil-UDP-Soak erfolgreich;
fmt/check/Clippy/build ebenfalls gruen. Native Tier-, Hoehlen-, Chat-, HUD-
und Tageszeitansichten geprueft, keine menschliche Audio-Abhoere.
Ein ruhendes Waldpaar misst 225.93 -> 217.58 FPS und p99 6.666 -> 7.398 ms;
keine behauptete Performanceparitaet oder Abnahme langer 150-m-Herdenlaeufe.

## Phase 61: Fackeln, Lokale Population Und Multiplayer-HUD

Fackeln ohne Bewegungskollision, mit verwurzelten animierten Flammenzungen,
staerker bewegtem Licht und sanftem originalem Feuer-Foley im bestehenden Mixer.
An die Spielbelichtung angepasste 18-m-Lichter funktionieren auch nachts
oberirdisch. Budgetierte Suche ueber geladenes Terrain ersetzt die alte
48-/64-m-Lichtgrenze; Edits werden ohne Mesh-Wartezeit beruecksichtigt.
Aktive Fackeln werden als eigene sowie bis drei nahe entfernte Lichtquellen
getragen. Statische Qualitaetsbudgets und Schattenproxies bleiben erhalten.

Das 256er-Wildtierlimit zaehlt die jeweilige 150-m-Spielerumgebung statt den
ganzen Weltbestand. Besitzerzuordnungen zaehlen weiterhin nicht. Bienen duerfen
an passenden Baeumen in Forest, Plains und Highland nisten.

Persistente Multiplayer-Optionen fuer Namen, gemeinsame Truhencursor und Farbe,
ausschliesslich TiltUI inklusive Colorpicker. Positionsdaten sind relativ zum
Truhenraster; nur andere berechtigte Betrachter derselben Truhe erhalten sie.
Name und gezogenes Item sind serverseitig bestimmt. Aenderungsbasierte Roster,
10-Hz-Cursorlimit und Ablauf unaktiver Cursor begrenzen die Uebertragung.

Chat und Lebensanzeigen bleiben im Inventar sichtbar; F1 liegt darueber.
Ein kurzer Layout-Gate blendet die Inventarflaeche erst stabil angeordnet ein.
Protokoll 35, kein Regionsformatwechsel. Nachweise und Grenzen im
[Phase-61-Bericht](tools/benchmarks/results/torch-social-2026-10-18/README.md).
964 Tests und alle fuenf Cargo-Pruefungen erfolgreich. Zwoelf native Fackel-
ansichten, vollstaendiger Menue-Smoke und zwei native Clients an eigenem
Dedicated mit sichtbarem Spielernamen geprueft. Der native Inventar-Hoverlauf
bleibt wegen fehlender Desktop-Zeigerposition offen; kein neues kontrolliertes
FPS-Messpaar und keine menschliche Audio-Abhoere behauptet.

## 62. Navigation, Tierklang, Items Und Inventar-Politur

Begrenzte Voxel-A*-Umwege, gecachte Bienenflugwege, sanftes Lenken und gebinnte
Abstandshaltung erweitern die bestehende KI. Die rotierende 128er-Updategrenze
verlangsamt groessere Tierbestaende nicht mehr; vorbereitete Wasserspalten und
einmalige Zellpruefungen vermeiden wiederholte Noise-Arbeit. Netzwerkposen und
Truhencursor beginnen neue Uebergaenge am aktuell sichtbaren Ort.

Zeitversetztes Schlafen mit leisem Atem-Foley statt Tierlauten. Echte CC0-
Feueraufnahme nur fuer gesetzte Fackeln, ohne doppelte Wandfackelquellen.
Erze reflektieren Licht ohne Eigenemission; Fackellicht ist weicher abgestimmt.
Neue Welten nutzen breitere gewundene Schluchten, alte Weltvertraege bleiben.

Einzelne Item-JSONs, farbige gebatchte 3D-Modelle, vier weitere Werkzeugrezepte,
Leder-/Clownfischdrops und Naehrwerte fuer die neuen Lebensmittel ergaenzen die
bestehenden Register. Kein Ofen oder Honigentnahme-Backend vorgetaeuscht.
Namensreichweite ist standardmaessig 40 m und in Multiplayer-Optionen aenderbar.
Gueltige Tilt-Symbole ersetzen fehlende Glyphen. Die Vorschau-Anbindung behaelt
geerbte Bilddeckkraft bei und umgeht den Inventar-Layoutschutz nicht mehr.

Nachweise und Grenzen: [Phase-62-Bericht](tools/benchmarks/results/life-polish-2026-10-18/README.md).
972 Tests, Workspace-Fmt/Check/Clippy/Build und native Menue-, Zwei-Client-,
Fackel- sowie echte OS-Inventar-/Crafting-Mauslaeufe bestanden. Klangabnahme,
nativer Mehrspieler-Truhencursor und lange GPU-/Streaminglast bleiben offen.

## 63. Fackellicht, Schwimmen, Cursor Und Gehaltene Items

Fackel-Nahhelligkeit und Stuetz-Fill reduziert, Cullingradius auf 28 m erweitert.
Die gemeinsame autoritative Physik haelt nach dem Abtauchen die Tiefe, statt
ungefragt aufzutreiben; seitliche Brustschwimm-Arme ersetzen die Ueberkopfpose.
Gehaltene Bloecke, Props, Werkzeuge und Waffen teilen die 3D-Dropmodelle,
erscheinen ohne Hand in First Person und am rechten Third-Person-Arm.
Lokale Schlaege bewegen Arm/Item nach vorn, Remote-Survival-Mining ebenfalls.

Echter links oben ausgerichteter Mauszeiger statt Papierflieger, feste groessere
Namensschrift und direkt bedienbare Cursorfarbe mit gespeichertem Apply.
Der Itembrowser nutzt die gemessene Rasterflaeche statt fester 45er-Seiten;
in der nativen 4K-Aufnahme passen alle 79 Items auf eine Seite.

978 Tests bestanden, ein bestehender Langzeit-Soak ignoriert. Alle fuenf
Workspace-Cargo-Pruefungen, isolierter Serverbuild und fensterloser Serverstart
bestanden. Zwoelf Fackelansichten, fuenf Held-Item-Ansichten, Menues und neun
Inventar-/Crafting-Aufnahmen mit echter OS-Maus geprueft. 4K separat als
Layout-/Renderpruefung, nicht als bestandener Mauslauf. Keine neue FPS-/
Speicherparitaet behauptet. Grenzen und Rohlogs im
[Phase-63-Bericht](tools/benchmarks/results/hand-light-ui-2026-10-18/README.md).

## 64. Fackelschatten, Wassertreten Und Werkbanklaterne

Das Fackelmodell wirft keinen eigenen Schatten mehr; der bisherige Nah-Fill
entfaellt. Hoehlenfackeln leuchten weich mit Faktor 0.6, aussen unveraendert.
Die Armrotation schlaegt jetzt nach vorn; Endpunkt-Tests pruefen Hand und
Werkzeugspitze. Stillstehende Schwimmer richten sich auf und bewegen Arme
und Beine langsam weiter, mit weichem Uebergang zur aktiven Schwimmpose.

Die Werkbanklaterne besitzt ein warmes 4-m-Licht aus ihrer JSON, korrekt
gedreht und mit nur einem Emitter fuer die zweiblockbreite Werkbank.
Kein zusaetzlicher Schattenpass, keine Welt-/Speicher-/Protokollaenderung.

982 Tests, Fmt/Check/Clippy/Workspace-Build und Headless-Start bestanden.
13 native Lichtansichten und fuenf Held-Item-Ansichten erfolgreich; die neue
Wassertretpose ist automatisiert, noch nicht nativ im Wasser abgenommen.
Remote-Antrieb bleibt bewegungsbasiert angenaehert. Keine neue FPS-/
Speicherparitaet behauptet. [Phase-64-Bericht](tools/benchmarks/results/water-pose-lantern-2026-10-18/README.md).

## 65. Referenzmodelle, Avatarwahl Und Streaming

Weibliche und maennliche Avatare mit heller Kleidung, brauner Hose, Haarformen,
nackten Fuessen und Augen/Blinzeln an die neuen Bilder angenaehert. Das vorhandene
sechsteilige Animationsmodell bleibt erhalten. `--gender=f|m` speichert die Auswahl
im bestehenden Profil und repliziert sie an Mitspieler; keine neue UUID.

Tierprodukte und Werkzeuge nutzen detailreichere volumetrische Modelle im
gemeinsamen Inventar-/Drop-/Handcache. Ein Materialbatch je Item, zwei beim
Honigglas, keine Einzelentities pro Detailvoxel. Gemeinsame Terraintexturen
ueberarbeitet, Kies (42) und Strukturstein (43) append-only hinzugefuegt.
Normale Bloecke bleiben geschlossene gemeinsam gemeshte Cubes ohne GLBs.

Reine Renderpayload-Transfers invalidieren nicht mehr den ChunkStore;
Prop-Uploads erzeugen keine Idle-Selbstinvalidierung. Begrenzte priorisierte
Jobauswahl, leerer Custom-Snapshot-Fastpath und gecachte Lichtquellen vermeiden
unnoetige Hauptthreadarbeit. Opt-in CPU-Zeitbereiche unterstuetzen die Diagnose.

Kontrollierter schneller Flug: 90.16 -> 105.09 FPS, p99 24.997 -> 14.551 ms.
Im 60-s-Paar 110.41 -> 124.88 FPS, aber ein einzelner 117-ms-Ausreisser und
253 MiB mehr Peak-VRAM. Keine vollstaendige Rucklerbehebung/Speicherparitaet.
Mit den neuen Assets erreicht der separate 60-s-Lauf 122.42 FPS und p99 13.993 ms.
Native Avatar-, Item-, Material- und Held-Item-Ansichten liegen im
[Phase-65-Bericht](tools/benchmarks/results/reference-streaming-2026-10-18/README.md).
Die Modelle folgen den Referenzen, erreichen aber keine pixelidentische Kopie;
Laub und Relief bleiben geometrisch einfacher. Langzeitabnahme bleibt offen.

991 Tests bestanden, ein bestehender Soak ignoriert. Fmt, Workspace-Check,
Clippy mit allen Targets, Workspace-Build und Headless-Serverstart bestanden.

## 66. Charakterdetails, Ego-Hand, Tiere Und Fackellicht

Weibliche und maennliche Avatare erhalten geformte Gesichter, unregelmaessige
Haarbueschel, Kleidung, Finger/Zehen und echte Ellenbogen-/Kniebewegung. Zehn
gemeinsame Koerpermeshes und ein Blinzelmesh ersetzen die sechs starren Teile.
Die Ego-Perspektive zeigt Hand und Unterarm; Item-/Handgriffpunkte stimmen auch
in F5 ueberein. Lauf-, Schwimm-, Ruhe- und Schlagposen verwenden weiterhin die
bestehende prozedurale Animation, keine behaupteten Motion-Capture-Clips.

Schaf, Kuh, Schwein, Biene und Clownfisch erhalten facettierte Formdetails,
Schnauzen, Augen, Hufe, Fell-/Fluegel-/Flossendetails und weichere Bewegung.
Der Flussfisch bleibt unveraendert. Bienen zeigen ihren Ruessel nur beim
Nektarsammeln. Bestehende Mesh-Teilbudgets, KI und Kollisionsformen bleiben.

Fackel und Metallbarren passen zum plastischen Itemstil. Welt-/Inventar-/Hand-
und Dropfackel teilen dieselbe Geometrie; ein gebackener, nicht emissiver
Item-Batch verhindert den weissen Eigenlicht-Hotspot. Gesetzte Fackeln behalten
animierte Flammen und PBR-Materialien. Die explizite JSON-Soft-Core-Lichtkurve
daempft nahe Spitzen und verbreitert die Ausleuchtung im vorhandenen Lichtpool,
ohne weitere Renderpaesse. Lampentausch an derselben Koordinate uebernimmt
nun auch die neue Lichtdefinition. Schadensraender sind schmaler, wesentlich
transparenter und trefferabhaengig; kritische Gesundheit pulsiert dezenter.

Referenztreue, GPU-Bilder, Geometriekosten, Messbedingungen und Grenzen stehen im
[Phase-66-Bericht](tools/benchmarks/results/character-light-creatures-2026-10-18/README.md).
Keine pixelidentische Kopie, allgemeine FPS-Paritaet oder vollstaendige
professionelle Animationsabnahme behauptet.

Finale Avatare: 36 860 beziehungsweise 38 896 Dreiecke. 1007 Tests bestanden,
ein bestehender UDP-Soak ignoriert; Fmt, Workspace-Check/-Build, Clippy mit allen
Targets und Headless-Serverstart bestanden. Native Licht-, Avatar-, Item-, Hand-,
Tier- und Survival-HUD-Proben erfolgreich. Erstes 60-s-Flugpaar 120.35 -> 88.58
FPS, zeitnahe Wiederholung 96.88 -> 99.16 FPS; die nicht isolierte Laufzeitdrift
und alle vier Messungen bleiben transparent dokumentiert.

## 67. Baeren, Fackelbelichtung Und Randverankerte Ego-Items

Baer als zusaetzliche datengetriebene Art im bestehenden autoritativen Tierkreis:
ufergebundene einzelne Spawns, Hunger 45, Durst 20, Fischjagd ab Hunger 15,
Trinken, Schlafen, lokale Wege und gelegentliches gestuetztes Baumklettern.
Servervalidierte Nahkampfangriffe auf Survival-Spieler verwenden Sichtpruefung,
Cooldown, Respawn-Schutz und den bestehenden Todesablauf. Neue Arten-/Statuswerte
sind append-only, Netzwerkprotokoll 39; gespeicherte Habitate werden nicht ersetzt.
Gemeinsame detaillierte Baerenmeshes und eigene Bewegungsposen ohne Detailentities.

Fackellicht wird erst nach der endgueltigen Hoehlenbelichtung abgestimmt.
Das beseitigt die falsche Aussenlicht-Verstaerkung bei laufender Weltzeit;
eine neue native Kammerprobe prueft pausierte und laufende Mittagsuhr.
Lichtkurve, Aussenreichweite und Quellenbudget bleiben unveraendert.

First-Person-Hand/Unterarm entfernt; Modellbounds und Kameraprojektion verankern
Items am unteren Rand. Werkzeuge sind um eine Vierteldrehung gedreht.
F5-Avatare und deren Handgriffe bleiben erhalten.

Geometrie, native Ansichten, Validierung und Grenzen stehen im
[Phase-67-Bericht](tools/benchmarks/results/bears-light-viewmodel-2026-10-18/README.md).

1024 Tests bestanden, ein bestehender 180-s-UDP-Soak ignoriert. Fmt, Workspace-
Check/-Build und Clippy mit allen Targets ohne Warnungen bestanden; nach der
finalen Testszene-Korrektur nochmals 29 Clienttests. Headless-Serverstart sowie
fuenf Baeren-, sieben Item- und 16 Lichtansichten erfolgreich. Vollstaendige
Fackelkammer bei laufender/pausierter Uhr mit EV6 ohne Aussenlicht-Verstaerkung
geprueft. Kein isoliertes FPS-Paritaetspaar fuer diese Aenderung gemessen.

## 68. MiniMap, Beleuchtungsuebergaenge Und Tier-Bodenkontakt

Nordorientierte TiltUI-MiniMap mit autoritativer Spielerposition, Blickrichtung,
Zoom und einklappbarer Ansicht. `M`, `+` und `-` sind ueber Controls neu belegbar.
Der gemeinsame asynchrone Kartenatlas verwendet vorbereitete Terrainspalten,
keine zweite Kamera oder neue Noise-Abtastung. Wiederverwendbare 16x16-Tiles,
4096er-Cachelimit und maximal fuenf bedarfsabhaengige Bildaktualisierungen pro
Sekunde begrenzen die Arbeit. Erkundete natuerliche Oberflaechen werden binaer,
pruefsummengeschuetzt und nach Welt/Overworld/Spieler getrennt gespeichert.

Belichtungsnormierte Sonnen-, Mond- und Umgebungshelligkeit vermeidet die
zwischenzeitliche Verstaerkung des untergehenden Sonnenlichts. Der Horizontshader
mischt Daemmerung und Nachtpalette kontinuierlich; bestehende Tagesendwerte,
Hoehlenanpassung und lokale Fackellichter bleiben erhalten.

Gemeinsame schrittweise Fallauflosung korrigiert widerspruechliche Mittelpunkt-
und Vierfuss-Bodenproben bei Landtieren. Teilauflage, unbekannte Sections und
Bodenabbau unter schlafenden Tieren sind regressionsgetestet. Baeren behalten
ihre vertikale Fallgeschwindigkeit auch zwischen Ruhe-/Jagdentscheidungen.

1032 Workspace-Tests bestanden, ein vorhandener UDP-Langzeittest ignoriert.
Native Kartenansichten pruefen integrierten Server, Bildinhalt, Zoom, Einklappen
und UI-Skalierung; 25 Lichtansichten decken Tag, Daemmerung, Nacht, Unterwasser
und bewegte Schatten ab. Messbedingungen, Buildpruefungen und Grenzen stehen im
[Phase-68-Bericht](tools/benchmarks/results/minimap-daylight-grounding-2026-10-18/README.md).
Keine gemessene FPS-Paritaet behauptet. Essen und Ofen sind neue offene Etappen;
die FullMap verwendet kuenftig denselben Atlas. Spielerbauten/dynamische
Wasseredits und Hoehlenkarten sind noch nicht in der Oberflaechenkarte enthalten.

## 69. Raeumliche MiniMap, Hoehlen Und Gameplay-Einstellungen

Runde TiltUI-Kompasskarte mit 2,5D-Hoehenrelief und Oberseitenfarben aus den
tatsaechlichen Blockmaterialien und Texturen. Geladene Baumkronen, Bauten und
bestaetigte Wasseredits werden asynchron abgetastet. Biomgrass und Wasser
verwenden die vorhandenen Rendererpaletten. Farben werden gespeichert;
aktuelle Hoehen-/Materialdetails bleiben begrenzt im RAM.

Tierportraets, Mitspielermarker und der eigene Richtungspfeil verwenden echte
replizierte Positionen. Tote Tiere und Bienen im Nest sind ausgeblendet;
undurchsichtige Decken verhindern Marker auf falschen Hoehlenebenen.
Automatische Hoehlenansicht mit getrennten Viermeterbaendern, alternativ feste
Oberflaechen-/Hoehlenansicht. Unbekannte Bereiche bleiben dunkel.

Neue Kategorie Gameplay mit HUD Layout und MiniMap-Customize-Popup.
Elf Karteneinstellungen, bestaetigtes Apply, Reset, Verwerfen und Display-
Rollback; Speicherung in configs/gameplay.toml. Bisherige HUD-Positionen bleiben
kompatibel. Separat allozierte Detailtiles, Revisionspruefung und bedarfsweise
Darstellung begrenzen Arbeit und Speicher; keine weitere Weltkamera.

1043 Tests bestanden, ein vorhandener UDP-Soak ignoriert. Format, Check, Build
und Clippy ohne Warnungen erfolgreich. Native Oberflaechen-/Hoehlenkarte mit
zweitem echten Netzwerkclient, Gameplay-Popup bei 960x640, vollstaendiger kleiner
Menueablauf und Headless-Serverstart bestanden. Keine allgemeine FPS-Paritaet
behauptet. Nachweise und Grenzen im
[Phase-69-Bericht](tools/benchmarks/results/spatial-minimap-gameplay-2026-10-19/README.md).

## 70. FullMap, Fluessigere MiniMap, Tierkontakt Und HUD-Rahmen

FullMap auf dem bestehenden Atlas mit Verschieben, cursorbezogenem Zoom,
Spielerzentrierung, Oberflaechen-/Hoehlenansicht und ladenden/unbekannten Bereichen.
Benannte farbige Wegpunkte koennen erstellt, bearbeitet und bestaetigt geloescht
werden; Speicherung pro Welt und Spieler neben den vorhandenen Kartentiles.
`M` oeffnet die FullMap, `N` klappt die MiniMap ein/aus. Alte Standardbindings
werden kompatibel uebernommen; eigene Belegungen bleiben erhalten.

Gecachte 2,5D-Terrainbilder werden asynchron hoechstens fuenfmal pro Sekunde
erneuert. Ein Material an den vorhandenen TiltUI-Bildelementen reprojiziert sie
mit kontinuierlichem Kartenmittelpunkt und Drehung; ein separater transparenter
Layer aktualisiert Tier-/Spielermarker und Richtungspfeil mit einstellbaren
24, 30 (Default), 40 oder 60 Bildern pro Sekunde. Tierportraets skalieren von
1 bis 2,5; die eigene Richtungsmarkierung und Mitspieler behalten ihre Groesse.
Die unterste MiniMap-Steuerleiste entfaellt, restliche Labels sind kuerzer und
zentriert. Begrenzte 4x4-Fernkacheln erhalten den erkundeten Farbhorizont der
FullMap auch nach Verdraengen feiner Tiles; es gibt keine zweite Weltkamera.

Landtiere behandeln reine Wasserauflage nicht mehr als begehbaren Boden und
ueberbruecken keine tiefen Luftspalten durch entlegene Fusskontakte. Gemeinsame
Schwimmauflosung senkt den Unterkoerper ins Wasser, reagiert auf Stroemung und
laesst Tiere an tragfaehigen Ufern wieder herauslaufen. Ruhende Tiere schauen
nahe Spieler an; der Ausloeser fuer Flucht ist enger gefasst. Baeren sind in
Darstellung und Koerperabmessungen um 35 Prozent vergroessert.

Gemeinsame stein-/goldfarbene Rahmen als TiltUI-Komponente mit einer geteilten
transparenten Bitmap und Bevys vorhandenem Nine-Slicing. Inventare, Weltlager,
Rezeptansicht, Itembrowser, Menues, Dialoge, FullMap und Hotbar verwenden sie.
F1-Debug, Zielinfo und Chat bleiben ohne diese Verzierung. Das Stats-HUD hat
keinen aeusseren Hintergrund/Rahmen: grosses Herz, roter Lebensbalken, gelbe
Ausdauer und Hungerkarte mit fuenf wiederverwendeten Atlasstadien. Hunger zeigt
ab 90 Prozent die volle Keule, ab 75 den ersten Biss, ab 50 die Haelfte,
darunter ein letztes Stueck und bis 10 Prozent nur noch den Knochen.

1065 Tests bestanden, ein vorhandener UDP-Soak ignoriert. Format, Check, Build
und Clippy ohne Warnungen erfolgreich. Native FullMap-/MiniMap-Funktionen mit
zweitem echten Netzwerkclient, Wegpunkt-CRUD und skaliertem Editor geprueft;
Gameplay-Apply/Reset/Reload, Stats-HUD mit gespeicherter Verschiebung,
Inventarlayout bis 4K und Headless-Serverstart bestanden.
Implementierung, Budgets, Aufnahmen und Grenzen stehen im
[Phase-70-Bericht](tools/benchmarks/results/fullmap-ui-animals-2026-10-19/README.md).
Keine kontrollierte FPS-/Speicherparitaet behauptet; Langzeitprofilierung bleibt
im Backlog.

## 71. Kartenbewegung, Referenz-HUD Und Materialschritte

MiniMap-Reprojektion verwendet die interpolierte lokale Koerperposition und
aktuelle Kameradrehung in jedem Renderframe. Neue Terrainbilder und ihr
UV-Bezug werden zusammen uebernommen. Langsamere Markeroverlays behalten
einen eigenen Bezug; der eigene Pfeil entsteht live im vorhandenen UI-Pass.
24/30/40/60-Hz-Markerpresets und asynchrone Terrainrasterung bleiben erhalten.
Nordorientierte Caches, geographisches Relief/Schattierung und eine stabile
Hoehenbasis verhindern projektionsbedingte Spruenge beim Drehen oder Springen.
FullMap-Mausziehen berechnet den Ausschnitt aus dem beim Druecken fixierten
Mittelpunkt, Mausanker und Viewportmassstab.

Stats-HUD neu an der Referenz ausgerichtet: grosses oktogonales Herzmedaillon,
Smaragdverzierungen, zusammenhaengende Stein-/Goldleisten und Blitz. Echte
TiltUI-Progress-Bars zeigen Gesundheit, Ausdauer und Hunger; der aeussere
Container bleibt transparent. Fuenf vorhandene Keulenstadien und bedingte
Sauerstoffanzeige bleiben erhalten. Keine neuen Statuseffekte oder Spielregeln.
Zwei transparente Rasterassets und ihre Imagegen-Prompts sind archiviert.

Neun Materialfamilien mit 51 CC0-Kontakten ersetzen die kleine Schrittbank:
Gras, Sand, Stein, Erde, Schnee, Holz, Kies, Glas und Laub. Explizite Zuordnung
aller Blockmaterialien, Auswahl ohne unmittelbare Wiederholung je Familie
sowie geringe Tonhoehen-/Pegelvariation. Quellenmix und reproduzierbare
Aufbereitung stehen in den bestehenden Audio-Credits und im Phase-71-Bericht.

1072 Tests bestanden, ein bestehender UDP-Soak ignoriert. Format, Check, Build
und Clippy ohne Warnungen erfolgreich. Native MiniMap-/FullMap-Pruefung mit
echtem Netzwerkpeer, mehrteiligen Drag-/Haltetests und 146.83-m-Boostflug samt
Drehung/Hoehenwechsel bestanden. Neues Stats-HUD inklusive Hunger-/Ausdauer-
Animation, Respawn und HUD-Layout-Speicherung nativ geprueft. 27 eingespeiste
Materialkontakte ueber echten AudioSink abgespielt; 51 WAVs auf Signal,
Eindeutigkeit und Headroom geprueft. Headless-Serverstart erfolgreich.
Schnelle Reise in unbekanntes Gebiet kann der budgetierten Kartierung kurz
vorauslaufen. Keine FPS-/Speicherparitaet und keine menschliche Hoerabnahme
behauptet.
[Nachweisstatus und Grenzen](tools/benchmarks/results/map-motion-stats-steps-2026-10-19/README.md).

## 72. Wetter, HUD-Fuellung Und Wegpunkt-Labels

Serverseitiges deterministisches Wetter mit Dauer, 24-Sekunden-Uebergaengen,
Wind, Naesse und klimatischem Regen-/Schneewechsel. Replikation fuer Beitritt,
laufende Sitzungen und Reconnect; IDX4 speichert Wetter im bestehenden Weltindex
mit IDX3-/IDX2-/INDX-Lesern. Regionen und Spielerformate bleiben unveraendert.
Begrenzte persistente Schneeakkumulation, Schmelzen und Regenloeschen verwenden
vorhandene Oberflaechenzustaende: keine neuen Bloecke oder Collider, nur geladene
Spalten, maximal 65.536 Zustaende. Bei vielen Spalten kann Akkumulation nachlaufen.

Ein GPU-Batch animiert Regen, Schneeflocken, Aufschlagringe und bewegte Wolken;
Daecher/Glas, Hoehlen und Unterwasser werden beruecksichtigt. Qualitaeten Off,
Low, Medium, High und Ultra aendern keine Serverregeln. Sonne, Nebel, Materialien
und kreditierte Regenaufnahme reagieren gemeinsam. Keine neue Entity pro Tropfen,
keine Partikel-CPU-Vertexanimation pro Frame, keine volumetrischen Wolken.

Stats-Balken fuellen die abgeschraegten Rahmenenden korrekt. Das rote Herz
pocht unter 60/30/10 Prozent zunehmend schneller; Gameplay-Schalter mit Apply,
Reset und TOML-Persistenz. MiniMap ist im bestehenden HUD Layout separat
verschieb-/speicherbar. Hotbar bleibt fest. Wegpunkte besitzen waehlbare Symbole,
Namen, Rand-Pins, Meter-/Kilometerabstaende und eine optionale Weltanzeige;
alte Dateien bleiben lesbar. Budgetierte TiltUI-Labels statt weiterer 3D-Entities.

Nachweise, native Ansichten und Grenzen:
[Phase-72-Bericht](tools/benchmarks/results/weather-hud-waypoints-2026-10-19/README.md).

## Phase 73: Wetterbefehle, Serverkonsole Und Weltkarte

Gemeinsamer Befehlskatalog, `/weather set clear|rain|snow|storm` und Chat-Hilfe
mit fuenf Befehlen je voller Seite samt TiltUI-Pagination. Die Dedicated-Konsole
fuehrt Befehle ohne Slash aus, bietet Tab-Vorschlaege/Verlauf und explizite
Spieler-Kontexte. Unix-Terminalzustand wird restauriert; Windows und einfache
Zeileneingaben besitzen getrennte Backends. Protokoll 41, kompatibler IDX5-Index.

Dichtere Regenlayer, sichtbare mehrschichtige Wolken mit Wettervorlauf und
gemeinsamer Windrichtung/-staerke fuer Vegetation und Blaetter. Lokaler,
dachgeschuetzter PBR-Naessefilm ueber vorhandene Terrainbatches und gemeinsame
Materialhandles. Nahbereich und Hintergrund teilen dasselbe Oberflaechenbudget.

Rechteckige FullMap mit Werkzeugleiste, Kompass, Raster, Mob-/Layerumschaltung
und optionalem echtem 3D-Hoehenfeld. Kompakte persistente TRMAP002-Hoehen und
Cache-Merge verhindern den periodischen Verlust des MiniMap-Reliefs; TRMAP001
bleibt lesbar. Eigenstaendiger N-Wegpunktdialog, durchsuchbarer TiltUI-Iconpicker
und groessere Weltlabels; neue Weltanzeigen standardmaessig an. MiniMap-Default J.

Grenzen und Architektur stehen im Phase-73-Abschnitt der README. Storm besitzt
noch keine Blitze/Donner; Wolken sind nicht volumetrisch, die 3D-Karte ist eine
erkundete Hoehenkarte und keine zweite vollstaendige Weltinstanz.

Native Ansichten, Testprotokolle und gemessene kurze Framefenster:
[Phase-73-Bericht](tools/benchmarks/results/commands-weather-map-2026-10-19/README.md).

## Phase 74: Wetterstaerken, Voxelwolken Und Durchgaengige Karten

MiniMap-Wegpunkte zeigen Namen direkt beim Icon, am Rand nur das Symbol und
nirgends eine Meteranzeige. FullMap/Weltlabels behalten ihre Entfernungen.
Hoehen werden mit unveraendertem Snapshotbudget fuer alle gestreamten Spalten
erfasst; kompakte Fernuebersichten behalten Hoehen/Wasser. Kein 32-m-Reliefdeckel
und keine Pixelsplat-Luecken mehr. Geschlossene schattierte 3D-Seitenflaechen,
hoeher aufgeloestes rechteckiges Renderziel mit MSAA und zentrierte FullMap-Texte.
Stabile Materialalbedos, Quellrevisionen und unveraenderte Ladeergebnisse verhindern
fortlaufende Mapping-Invalidierung durch Wettertoenung und nutzlose IO-Jobs.

Asynchron gebatchte echte Voxel-Wolkenbaenke ersetzen die flachen Noise-Ebenen.
Starke kohaerente Sturmboeen mit passenden Render-/Picking-Grenzen. Schnee 1..5
und Blizzard, Thunder/Thunderstorm, synchronisierte Blitzereignisse und seltene
exponierte Einschlaege mit vorhandenen Schadens-/Feuerregeln. Entfernungsverzoegerter
Donner im bestehenden Audiomixer. Protokoll 42 und kompatibler IDX6-Wetterindex.

Weltkoordinatenbasierte, teilweise nasse Pfuetzenfilme statt gleichmaessiger
Blockabdunklung; exponierte Regenflaechen folgen dem gemeinsamen Wetterwert
statt der Section-Jobreihenfolge. Daecher und Hoehlen bleiben getrennt.
Gebatchter, auf-/ruecklaufender Wellensaum an flachen geladenen
Meeresufern und zurueckbleibender lokaler Nassfilm. Keine neuen Fluidzellen.

Alle fuenf Cargo-Pruefungen und 1154 Tests bestanden (ein vorhandener langer
UDP-Soak ignoriert); 52 native Kartenschritte und 29 Wetterphasen sowie Chat,
Linux-PTY/Pipe und fensterloser Dedicated-Start geprueft.
Architektur, Screenshots, kurze Framefenster und bekannte Grenzen:
[Phase-74-Pruefbericht](tools/benchmarks/results/weather-coasts-maps-2026-10-19/README.md).

## Phase 75: Wolkendecken, Sturmwasser Und Live-Miniatur

Zusammenhaengende Voxel-Puffbaenke mit groben Fernringen bilden fuenf
Wolkendecken. Sonne/Mond liegen korrekt dahinter, Terrain davor; keine erhoehte
Terrain-Sichtweite. Die bisherige Blizzard-Dichte ist die neue Schneestufe 1,
weitere Stufen reichen bis 45-facher lokaler Dichte mit festen Geometriebudgets.

Ab echten 100% blendet die TiltUI-Ladehuelle weich zur Welt ueber, bevor
Eingaben und Weltgeraeusche freigegeben werden. Unterwasserflaechen bekommen
keinen Regenfilm, auch nicht durch alte gespeicherte Regennaesse.
Interpolierte Kuestenfilme, rollende Schaumkaemme und sturmgesteuerte Wellen
verwenden weiterhin gemeinsame Section-Batches und Materialhandles.

Die FullMap-3D-Ansicht verwendet die echte geladene Welt mit Originalmodellen,
Materialien und Animationen; kein nachgebautes Hoehenfeld. Die zusaetzliche
Miniaturkamera teilt Weltmeshes und ersetzt waehrenddessen das verdeckte
Hauptwelt-Rendering. 2D/2.5D behalten die komplette Erkundungsuebersicht.
Geteilte gefilterte Himmelslicht-Texturen und fokussierte vorhandene Schattenkaskaden
erhalten die Spielbeleuchtung; ein separater Licht-Layer verhindert schwarze
Atmosphaeren-Cubemaps beim Ausblenden der Hauptwelt. Die Kamera-/Qualitaetswerte
werden beim Schliessen wiederhergestellt.
Keine neuen Dependencies, Generator-/Seed-/ID- oder Protokoll-/Dateiformataenderungen.

Validiert mit Format, Check, 1177 bestandenen Workspace-Tests, Clippy ohne
Warnungen und Build. Ein expliziter 180-s-UDP-Soak bleibt ignoriert.
Nativ: 45 Wetterphasen, echtes Lade-Reveal mit verspaetetem Welt-Audio,
52 Kartenstufen und fensterloser Dedicated Server erfolgreich.
Kurze Wetterfenster auf RX 6950 XT liefern 154.01 FPS fuer Schnee 5 und
175.47 FPS am stuermischen Meer; kein identisches Vorher-/Nachher-Paar und
keine allgemeine FPS-/Speicherparitaet. 3D bleibt auf geladene Weltbereiche begrenzt.

Technik und Grenzen:
[Phase-75-Pruefbericht](tools/benchmarks/results/cloud-decks-miniature-2026-10-19/README.md).

## Phase 76: Mobdichte, Hoehlenkarte Und Wetterkontakte

Neue natuerliche Habitatkandidaten werden serverseitig deterministisch um 40%
reduziert, ohne gespeicherte Tiere zu loeschen oder Arten-/Herden-IDs und lokale
konfigurierte Obergrenzen zu veraendern. Integrated und Dedicated teilen die Regel.

Die 3D-Miniatur ergaenzt erkundetes, entladenes Terrain aus dem bestehenden
Hoehen-/Farb-Atlas mit einem asynchronen, budgetierten Batch. Geladene Bereiche
behalten Originalmodelle; unbekannte Gebiete und entfernte Originalobjekte
werden nicht erfunden. Hoehlen erhalten einen Bodenschnitt mit Randwaenden,
AABB-basierter Pflanzenfilterung und exakt wiederhergestellten Miniaturlayern.
Hoehlenerkennung reicht bis zur echten Oberflaeche statt nur 20/64 m.
Replizierte Spieler benoetigen keine Detailkachel mehr und erhalten auf beiden
Karten Namenslabels aus dem bestehenden SocialState.

Schnee besitzt kleinere begrenzte Winkelgroessen, individuellere Fallbewegung
und volle raeumliche Streuung mit zellgenauen Daechern. Qualitaets-/Quadbudgets,
24 Dachabfragen je Frame und Uploadtakt bleiben begrenzt. Gewitter erhalten
horizontale verzweigte weiss/lila Wolkenentladungen mit lokaler Aufhellung.
Sturm erzeugt mehr Blaetter im weiterhin einzelnen lokalen Batch.
Unterwasser-Regenaufpralle benutzen den bestehenden Partikelpfad mit maximal
289 lokalen Ringen auf freien Wasseroberflaechen, ohne neue Renderpaesse.

Zwei bearbeitete CC0-Regenlagen folgen geladenen Aufprallflaechen und verblassen
beim Fliegen mit der Entfernung. Nasse Schritte behalten ihren Materialklang
mit gespeichertem Wasserfilm; vier Aufnahmevarianten und getrennte Kadenz
verbessern Schwimmkontakte. Spielerkoerper, Lade-Audiosperre, Dach-/Hoehlen- und
Unterwasserfilter bleiben erhalten; der zusaetzliche dekodierte Audiobedarf
liegt bei etwa 4.6 MB. Eine menschliche Hoerabnahme bleibt offen.

Keine Dependency-, Generator-/Seed-/ID-, Protokoll- oder Speicherformataenderung.
Format, Check, 1194 Workspace-Tests, Clippy ohne Warnungen und Build geprueft;
ein bestehender expliziter 180-s-UDP-Soak bleibt ignoriert.
Native Bilder, Messwerte und konkrete Grenzen:
[Phase-76-Pruefbericht](tools/benchmarks/results/weather-audio-map-2026-10-19/README.md).

## Phase 77: Flache Hoehlenkarten Und Stabile Wettermeshes

Hoehlenkarten sind ausschliesslich 2D, ohne Relief oder Miniaturkamera.
Zentrale Ebenenguards sperren 3D auch nach direkten Commands und automatischen
Wechseln; Kamera, Raster, Sichtbarkeitslayer und Schatten verwenden dieselbe
aufgeloeste Ebene. Die linke FullMap-Leiste besitzt sieben gleich grosse
quadratische Symbolbuttons mit echten TiltUI-Hovertooltips statt Labels.

Der fehlerhafte grobe Fernterrain-/Hoehlenschnitt aus Phase 76 ist entfernt:
gemittelte Kartenhoehen von Baumkronen erzeugten kuenstliche Hoehenwaende.
Oberflaechen-3D zeigt weiterhin die geladenen Originalmodelle, aber keine
erfundene Ferngeometrie. Entfernte Erkundung bleibt in 2D/2.5D erhalten;
eine belastbare entfernte 3D-Rekonstruktion bleibt ausdruecklich offen.

Leere Unterwasser-Regenbatches werden nicht mehr als Null-Vertex-Mesh an Bevy
uebergeben. Der bisherige gueltige Mesh bleibt verborgen und wird unter
derselben ID reaktiviert. Dies beseitigt den konkret nachgewiesenen
unallocated-key-Pfad des Slab-Allokators; kein Fehlerfilter oder Dependency-
Patch. Ein ECS-Test prueft leer/befuellt/leer/befuellt und die Assetlebensdauer.

Wetter wiederverwendet seine nahe-zuerst Roofqueue und berechnet Revisions-
signaturen sectionweise statt pro 1-m-Zelle. Fluessigkeitsepochen invalidieren
Regenaufpralle auch ohne Terrainrevision. Beide Karten teilen eine begrenzte
Auto-Ebenenprobe; erzwungene Modi, Teleports und Weltwechsel bleiben unmittelbar.
Die bestehenden Query-/Upload-/Partikelbudgets und Sichtweiten bleiben erhalten.

Format, Workspace-Check, 1201 Tests, Clippy ohne Warnungen und Workspace-Build
sind geprueft; ein expliziter 180-s-UDP-Soak bleibt ignoriert. Native Karten-
und Wetterbilder sowie transparente Messwerte und Restgrenzen:
[Phase-77-Pruefbericht](tools/benchmarks/results/map-streaming-fixes-2026-10-19/README.md).
Keine Dependency-, Generator-/Seed-/ID-, Protokoll- oder Speicherformataenderung.

## Phase 78: Unterwasser-Dekoration Und Marine Tierwelt

Sieben neue Arten erweitern das bestehende Prop-System: Seegras, Seegurken,
verzweigte Korallen, Roehrenkorallen, Plattenkorallen, Seeanemonen und kleine
Steine. Deterministische Bodenanker, Tiefen-/Untergrund-/Klimaregeln und
Riffgruppen verwenden vorhandene Generator-, Picking-, Drop-, Journal-,
Speicher- und Section-Batchpfade. Aquatische Props ueberstehen Wasserkontakt;
bewegliche Pflanzen verwenden den bestehenden Wind-/Stroemungslook.
Vier Korallenbloecke und Mud ergaenzen append-only die Block-IDs 44..48:
einzelne JSONs, matte Miptexturen und geschlossene Cube-Reliefs, keine GLBs.

Squid, Moon Jelly, Fire Jelly, Void Jelly, Sun Jelly, Crystal Jelly, Reef Shark,
Hammerhead Shark und Whale Shark erweitern die vorhandene Mobverwaltung.
Schwimmen, begrenzte lokale Umwege, Anheften/Bodenruhe, Fischjagd und Squid-
Flucht mit Tinte nutzen dieselbe Autoritaet und Replikation. Haie/Quallen
spawnen im Meer, Squid selten auch in Seen. Bestehende Fische werden nicht
ersetzt; neue Arten werden erst fuer neue Habitatkandidaten ausgewaehlt.
Tinte trifft den untergetauchten Spielerkoerper, laeuft im vorhandenen
Umgebungspass ab und funktioniert auch bei niedriger Wasserqualitaet.
Marine Foley und eigene Kartensilhouetten ergaenzen die bestehenden Systeme.

Hoehlenkarten behalten gehaltene Drags; gesperrte 3D-Werkzeuge sind auch beim
Hover grau und ihre Commands beenden keinen Drag. Fackel-Streulicht ist ab
vier Metern 20% heller, der Nah-Hotspot bis einem Meter bleibt unveraendert.
Wolkenbaenke variieren deterministisch innerhalb Y 200..312 inklusive Sturm-
Wachstum. Beim Aufklaren ergaenzt High/Ultra hoechstens einen Strahlenbatch
mit 72 Vertices: Weltgeometrie-Approximation, kein volumetrischer Zusatzpass.

1241 Workspace-Tests sowie Formatierung, Check, striktes Clippy und Build
sind bestanden; der regulaer ignorierte UDP-Langzeitsoak bleibt separat.
Native Riff-/Tiermodell-, Karten-/Wetter- und Headless-Pruefungen sind bestanden.
Konkrete Messnachweise sowie verbleibende Grenzen stehen im
[Phase-78-Pruefbericht](tools/benchmarks/results/underwater-ecology-2026-10-19/README.md).
Keine 1:1-Referenzqualitaet oder allgemeine FPS-/Speicherparitaet behaupten.
Netzwerkprotokoll 43 erfordert gemeinsam aktualisierte und neu gestartete
Clients und Server. Generatorversion, Seeds und bisherige IDs bleiben erhalten.

## Phase 79: Meeresbereiche, Hoheres Seegras Und Eis

Der vorhandene Unterwasser-Prop-Generator verwendet zusammenhaengende
Klimabereiche statt des alten 12x12-Riffgitters: Coral Reef auf warmen
Felsruecken, Seagrass Forest auf weichen Boeden und Cold Ocean ohne tropische
Korallen. Koloniefelder und freie Kanaele ordnen den Bewuchs; Tiefe, Neigung,
Lichtnaeherung und Untergrund bestimmen Art und Groesse.

Seegras besitzt hohe, gegliederte Halme und versetzte stumpfe Blaetter.
Staerkere Wasserbewegung bleibt an der Wurzel ruhig und ist zwischen Shader,
Picking und Renderbounds abgeglichen. Anemonen erhalten einen niedrigen
Koerper und bis zu 28 gekruemmte feine Tentakel. Section-Batches, gemeinsame
Vorlagen und die bestehenden Geometriebudgets bleiben erhalten. Solide
Blockplatzierung entfernt nur tatsaechlich geschnittene Pflanzen, auch wenn
deren Wurzel in der Section darunter liegt.

Neue Welten erhalten `ocean_revision = 1` fuer verbundene Riffvorspruenge,
Bodenmaterialgruppen und Eisdecken mit Wasserwegen. Fehlende Werte alter
Spielstaende bedeuten 0 und bewahren deren Geometrie. Append-only Block 49
`teramic::ice` verwendet eine eigene JSON-Definition und transparente
512px-Miptextur in der vorhandenen Cube-Pipeline, kein GLB. Gefrieren,
Schmelzen und Eis-Rutschphysik sind weiterhin offen. Der Squid-Kopf ist in
X/Z 20% kleiner; Hoehe und Tentakel bleiben unveraendert.

1249 Workspace-Tests, Format, Check, striktes Clippy und Build sind bestanden;
ein bestehender Langzeit-UDP-Test bleibt regulaer ignoriert. Sieben native
Naturansichten, Modellansichten und Headless-Start wurden geprueft. Das
30-s-Meerpaar liegt bei 186.90 -> 189.98 FPS mit rund 8 MiB mehr Peak-RSS;
ein separater dichter Seegraswald erreicht 182.76 FPS, ohne Vorher-Paar.
Keine allgemeine FPS-/Speicherparitaet oder Langzeitabnahme daraus ableiten.
Native Naturansichten, Messbedingungen und Grenzen sind im
[Phase-79-Pruefbericht](tools/benchmarks/results/ocean-habitats-2026-10-19/README.md)
festgehalten. Netzwerkprotokoll 44 wegen des erweiterten Weltvertrags:
Client und Server gemeinsam neu bauen und starten. REG9/PLY3, Seed,
Generatorversion 5 und alle bisherigen IDs bleiben erhalten.

## Phase 80: Hoehlenoekologien, Seen Und Abbaubare Segmente

Die vorhandenen Hoehlenregionen erhalten feuchte Salzraeume im bisherigen
Cliffted Desert, breite Boden-Decken-Tropfsteinpfeiler, hoeher gelegene
bepflanzte Kammern und Highland-Raeume mit Eiskristallen sowie verbundenen
acht Meter tiefen Wasserfall-Ueberlaeufen. Seen besitzen geschlossene Boeden;
mehrere vertikale Wasserintervalle trennen Oberflaeche und Untergrund.
Solver, Meshes, Schwimmen, Wasser-Foley und Regenausschluss verwenden dieselbe
Belegung. Salz tropft sparsam ueber den bestehenden Partikelbatch.

Neue native Segmentbloecke 50..56 ergaenzen die bestehende Registry mit
Seegras, Salz-/Tropfstein-/Eiskristallen, Ranken, Wurzeln und Hoehlenpilzen.
Einzelne Meterzellen sind abbaubar; Haltverlust entfernt nur abhaengige
Segmente, auch ueber Section-Grenzen hinweg. Abgetrennte Mineralspitzen
fallen im bestehenden Drop-System und verursachen bei Spielern Aufschlagschaden.
Salzhostbloecke liefern nichts, Kristalle das bestehende Salzitem.
Journal, binaere Regionen und vorhandene UDP-Replikation speichern die
Entnahmen; es gibt keinen parallelen Prop-/Drop-/Savepfad.

Kanonische Dreiecke dienen Rendering und Picking, passende Querschnitte der
Mineralkollision. Teilzellen bleiben auch zwischen transparenten Nachbarn
sichtbar. Material-/Section-Batches und geteilte Dreiecksvertices vermeiden
eine Entity pro Blatt oder Facette. Unterwasserwind bleibt zwischen Zellen
stetig. Hoehlengras behaelt seine Biomfarbe unterhalb des Meeresspiegels.
Natuerliches Meereis entfaellt; platzierter Eisblock 49 bleibt erhalten.
Groessere verbundene Korallenruecken tragen die vorhandenen Korallenprops.

Neue Welten speichern `cave_structure_revision = 1`, fehlende alte Werte
bleiben 0. Fuer neue Hoehlen und natuerlich segmentiertes Seegras eine neue
Welt anlegen; keine stille Weltmigration. Alte IDs, Seed, Generatorversion 5
und REG9/PLY3 bleiben erhalten. Netzwerkprotokoll 45 erfordert gemeinsam
aktualisierte Clients und Server. Keine Dependency-Upgrades oder zwingenden
Render-/Audio-Abhaengigkeiten im Dedicated Server.

Native Hoehlen-/Meeresbilder, Headless-Start, Cargo-Pruefungen und transparente
Messwerte stehen im [Phase-80-Pruefbericht](tools/benchmarks/results/cave-structures-2026-10-19/README.md).
1271 Workspace-Tests, Format, Check, striktes Clippy und Build sind bestanden;
ein vorhandener Langzeit-UDP-Soak bleibt regulaer ignoriert.
Der dichte neue Seegraswald bleibt im kurzen Kamerapaar rund 24% langsamer;
das ist keine bestandene allgemeine Performanceparitaet. Langzeitlasttests,
weitere GPUs, Tier-Aufschlagschaden und Wachstumszyklen bleiben offen.

## Phase 81: Groessere Hoehlen, Mineralbuendel Und Spectator

Neue Welten verwenden Hoehlenrevision 2: haeufiger grosse Kammern, bis zu
28% groessere Radien und durch massive Felsbaender unterbrochene Tunnelnetze.
Salzraeume enthalten zusammenhaengende Salzvorkommen zwischen normalem Stein,
geschwungene unterirdische Fluesse und trockene Terrassen. Seltene Wurzeln und
Pilze dekorieren auch Gaenge ausserhalb der grossen Kammern.

Salz, Tropfstein und Eis besitzen breite facettierte Schaefte, kurze Endfasen
und unterschiedlich lange Gruppen. Picking und gebuendelte Darstellung teilen
dieselbe Geometrie. Anklicken einer beliebigen Kettenzelle mit passendem
Kristallitem verlaengert das freie Ende; neue Nachbarketten entstehen beim
Anklicken des tragenden Felses. Authority prueft Halt, Reichweite des Treffers,
Endpunkt, Inventarverbrauch und Revision. Salzressourcen lassen sich einsetzen.

`/gamemode spectator|2` erlaubt kollisionslosen Flug und bessere Hoehlensicht.
Abbau, Platzierung, Inventar-/Containeraktionen, Angriffe und Itemaufnahme
sind serverseitig gesperrt; andere Spieler sehen keinen Spectator-Avatar.
Der Modus wird gespeichert. Rueckwechsel erfordert geladenen freien Raum.

Bestehende Hoehlenrevisionen bleiben erhalten; neue Landschaft braucht eine
neue Welt. Generatorversion 5, IDs und Speicherformate bleiben unveraendert.
Netzwerkprotokoll 46 erfordert gemeinsam aktualisierte Clients und Server.
Native Bilder, Messwerte, Tests und Grenzen stehen im
[Phase-81-Bericht](tools/benchmarks/results/cave-refinement-2026-10-19/README.md).

## Phase 82: Hoehlenbiome Und Connector-Integration

Neue Welten speichern Hoehlenrevision 3 mit einer gemeinsamen raeumlichen
Normal-/Salz-/Lush-/Cascade-Verteilung. Konfigurierbare Auswahlgewichte
45/20/20/15, unterschiedliche Regionsgroessen und weiche Mischzonen ersetzen
die reine Spezialkammer-Auswahl. Dieselben vorbereiteten Biomgewichte gelten
fuer Kammern, Noise-Verbinder und unterirdische Eingangsgaenge.

Tunnel erhalten passende Salz-/Moos-/Erdablagerungen, Kristalle, Wurzeln,
Ranken, Pilze und kleine vorhandene Props. Dichte, Feuchtigkeit und Bewuchs
gehen graduell ineinander ueber; Anker und freie Ganghoehe bleiben geprueft.
Geschlossene Aquifertaschen ergaenzen passende kleine Becken in Verbindungsgangen
ueber die vorhandene Fluessigkeits- und Renderpipeline. Laengere geschlossene
Passagen koennen ein sanftes Gefaelle mit Stroemung entlang freier Wasserzellen
besitzen; seitliche untergetauchte Oeffnungen werden vor der Fuellung verworfen.

Raumzentren, Hoehen, Radien, Ausrichtung, Seitenverhaeltnisse, Doppelkammern
und Boden-/Deckenrelief variieren. Positive Tunnelbreiten mit moderater
Dichtereduktion ersetzen die grossen geschlossenen Netzbaender von Phase 81.
F1 zeigt Region, Cave-Biome, Mischung, Feuchtigkeit, Grenznaehe und Connector.
Ein reproduzierbarer CPU-Test mit farbigem Datenquerschnitt ist enthalten.
1287 Workspace-Tests, Clippy, Check, Formatierung, Build und Headless-Smoke
sind erfolgreich. Acht native Hoehlenansichten wurden aufgenommen. Die
neue Generierung kostet im CPU-Schnitt 15.8% mehr; Langzeitabnahme bleibt offen.

`configs/world.toml` enthaelt die validierten `[cave_biomes.*]`-Tabellen.
Alte Weltvertraege 0/1/2, IDs und Speicherformate bleiben erhalten.
Keine neue Tunnel-Engine oder UI-/Speicherarchitektur. Netzwerkprotokoll 47
erfordert gemeinsam aktualisierte Clients und Server. Technische Details,
Messungen und Grenzen: [Phase-82-Bericht](tools/benchmarks/results/cave-biomes-2026-10-20/README.md).

## Phase 83: Mineralterrassen, Lush-Schichten Und Zellwasser-Kaskaden

Neue Welten nutzen Hoehlenrevision 4 mit eigenstaendigen Salzterrassen,
bewachsenen Lush-Inseln und mehrstufigen Cascade-Becken. Der gemeinsame
Regionssampler und bestehende Connector bleiben die Grundlage; Temperatur
ergaenzt die Klimaeignung. Alte Hoehlenvertraege 0/1/2/3 werden nicht migriert.

Salzbelag und Erz sind getrennt: Salt Stone 57 und Pink Salt Stone 58 sind
datengetriebene Cube-Bloecke ohne GLB. Regionale Belags-/Clusterbudgets
erhalten ruhige Steinbereiche. Breite abgestufte Kristallketten,
versetzte Cluster, seltene Saeulen, groessere Rankenblaetter und geschwungene
Wurzeln verwenden weiterhin gemeinsame Segment-Batches und Picking.
Lush nutzt vorhandene individuell entfernbare Props und gruenere Bodenauflagen.

Kaskaden wecken echte Auslaufzellen im vorhandenen autoritativen Fluid-System.
Wasser faellt, trifft auf tiefere Becken, schlaeft an ungeladenen Grenzen und
wird im vorhandenen Regionsjournal gespeichert. Die synthetischen senkrechten
Curtains entfallen dort; echte Aufschlaege nutzen den begrenzten Gischt-Batch.
Erzadern erhalten zusammenhaengendere Randformen, Salz seltene separate
Vorkommen mit teilweise vergrabenen Lagerstaetten.

Testregionen, Kompatibilitaet, Messungen und offene Art-/Langzeitgrenzen:
[Phase-83-Bericht](tools/benchmarks/results/cave-overhaul-2026-10-20/README.md).

## Phase 84: UTF-8-Editor Und Gleichmaessigeres Streaming

Programmatische TiltUI-Textersetzungen synchronisieren vor wartenden
Bevy-Edits das Parley-Layout und dessen Byteindex-Auswahl. Regressionstests
reproduzieren den gemeldeten Emoji-Absturz und pruefen Unicode, kuerzere
Texte sowie die reale Edit-Warteschlange. Keine Dependency-Aenderung.

Terrain-Meshauftraege behalten ihre priorisierte Kandidatenliste zwischen
unveraenderten Frames. Veraltete Worker-Ergebnisse werden erneut eingeplant,
auch bei anschliessend stehender Kamera. Dach-/Naesse-Snapshots werden fuer
LOD-Wechsel wiederverwendet und bei Welt-/Journal-Aenderungen invalidiert.
Kleinere Vertexquoten und begrenzte Batchzahlen verteilen Uploads; einzelne
uebergrosse Sections behalten Fortschritt. Sichtweite und Geometriequalitaet
bleiben unveraendert. Ladezeitkosten und verbleibende Frame-Ausreisser werden
nicht als vollstaendige Performance-Paritaet ausgegeben.

[Tests, native Chat-Probe und Flugvergleiche](tools/benchmarks/results/text-streaming-2026-10-20/README.md).

## Phase 85: Renderwarteschlangen, Wasserpuffer Und 4K-Profilierung

Persistente Terrain-/Prop-/Wasserwarteschlangen vermeiden unnoetige
Weltneusuchen. Ankuenfte erhalten noch wartende LOD-Arbeit; veraltete
Worker-Ergebnisse werden erneut eingeplant. Mikrodetail und Prop-LOD
nutzen dreidimensionale Section-Abstaende mit gebuendelter Neuselektion.
Terrain-Grund-LOD, Sichtweiten, Props und Generator bleiben erhalten.
Kartensnapshots teilen einen Spaltenindex; Audio-/Lichtabfragen verwenden
invalidierte Caches. Unveraenderte Windmaterialien erzeugen keine
erneuten Material-Modified-Events.

Vollstaendig verdecktes Zellwasser erzeugt keinen leeren WaterPatch mehr.
Alle betroffenen Uploadpfade lehnen leere Meshpuffer ab. Der konkrete
bekannte Slab-Allocator-Pfad wird damit verhindert; eine pauschale
Behebung aller Allocator-Fehler wird nicht behauptet. Chunk-Animationen
warten nicht mehr allein wegen sichtbarer Schatten-Proxys auf einen
unmoeglichen Color-Draw.

Wasser kann nativ den vorhandenen linearen Tiefensampler verwenden;
Software-/MSAA-/Web-Rueckfall bleibt erhalten. Das isolierte Paar zeigt
rund 9% weniger Wasserpasszeit, aber keine messbare Gesamt-FPS-Steigerung.
Sechs echte 4K-Wetterfluege und der autoritative Fluid-Edittest bestehen.
Die hohen Kosten bei Sichtweite 20 bleiben offen. Buildartefakte wurden
bereinigt, ohne Welten, Assets oder Dependency-Versionen zu veraendern.

[Messbedingungen, Regressionen, Tests und Restgrenzen](tools/benchmarks/results/performance-weather-2026-10-20/README.md).

## Phase 86: Gehaltenes Essen Und Mob-Spawn

Essen verwendet die bestehende Benutzen-Aktion im aktiven Hotbar-Slot.
Der Server misst jeden Bissen und prueft Inventarrevision, Spielmodus,
Schaden und fortgesetztes Halten. Loslassen, Menue und Slotwechsel
brechen ohne Verbrauch ab. Survival verbraucht genau ein Item;
Creative zeigt die Animation ohne Verbrauch oder Survival-Bonus.
Der alte Sofort-Essen-Aufruf umgeht die Haltezeit nicht mehr.

Zwoelf vorhandene Nahrungs-JSONs definieren Hunger, Saettigung, Essdauer,
optionale Regeneration und Restitem. Bestehende TiltUI-Tooltips zeigen
die Werte; Ego-/Avatarbewegung und leises vorhandenes Kau-Foley begleiten
den Ablauf. PLY4 speichert erweiterte Vitals und liest PLY3/PLY2 explizit
kompatibel. Inventar, Cursor und bisherige Statistiken bleiben erhalten.

`/spawn teramic::clown_fish` und die anderen vorhandenen Arten besitzen
Chatvorschlaege sowie denselben Konsolenparser. Der Server prueft Rechte,
geladenen Boden beziehungsweise Wasser und freien Raum. Command-Tiere
werden im bestehenden Populations-/Regionssystem repliziert und gespeichert,
ohne natuerliche Spawnzellen oder Nestbesiedlung zu blockieren.

[Architektur, Bedienung, Validierung und Grenzen](tools/benchmarks/results/food-spawn-2026-10-20/README.md).

## Phase 87: Oefen, Bellows, Rauch Und Place-Essen

Essen folgt jetzt derselben konfigurierbaren Taste wie Place Block; die bisherige
Benutzen-Aktion bleibt erhalten. Stationsinteraktion und gezieltes Fuettern haben
Vorrang. Extra Shift mit Place erlaubt weiterhin das Pflanzen essbarer Pilze.
Unabhaengige Niederschlagsauswahl und Fallphase verhindern die zuvor korrelierten
Regenbaender, ohne Partikel- oder Meshbudgets zu erhoehen.

Zweiblockhoher, ausrichtbarer Ofen 59 mit fuenf modularen Rohrformen 60 bis 64,
modellgenauer Auswahl/Kollision und gemeinsamem Inventar beider Objektteile.
Vier Bellows verwenden Leder, zwei Holz und ihr Stufenmaterial: Stone +10%,
Iron +20%, Gold +10%/5% Doppelchance, Ruby +15%/7,5% Doppelchance.
Acht Schmelz-/Kochrezepte und einzelne Brennstoffdefinitionen liegen in
`assets/smelting/` und `assets/fuels/`. Eisen-/Goldraffination nutzt den Ofen
statt der bisherigen Handwerksrezepte.

Autoritative Verarbeitung, sichere Ein-/Ausgabe-/Fuel-/Bellows-Filter,
TiltUI-Fortschritt, bestehende Mausgesten und TMI-Rezeptladen sind integriert.
Blockierte Ausgabe verbraucht keine neue Zutat und entzuendet keinen neuen
Brennstoff; bereits brennender Brennstoff laeuft ab. Unbeladene und offline
vergangene Zeit pausiert. Regionen speichern Inhalt, Fortschritt, Restbrennzeit,
deterministischen Bonuszaehler und Rauch; Abbau behandelt beide Teile und Drops.

Passende Rohranschluesse fuehren den Rauch zum Austritt. Geschlossene geladene
Raeume erhalten eine Rauchzelle pro Brennsekunde; Ueberschneidung verursacht
nicht-stapelnden Schaden von zwei Punkten pro Sekunde fuer Survival-Spieler
und Mobs. Begrenzte Raum-/Rohrsuche, Dissipation und geteilte Feuer-/Rauchbatches
verhindern unbeschraenkte Arbeit. Feuerlicht und vorhandenes leises Feuer-Audio
begleiten den aktiven Ofen. Dies ist eine budgetierte Blockzustandssimulation,
keine CFD-Simulation und keine exakte Erkennung beliebig grosser Gebaeude.

Native Vulkan-Pruefungen umfassen Modelle, Place-Essen, Wetter sowie echte
Serverplatzierung, TMI, Schnellverschieben, direkte Ergebnisentnahme,
Fortschritt, kompakte UI und Save/Reload. Referenzinspirierte Modelle sind
keine pixelidentische Kopie; keine ungemessene Performance-Paritaet.
[Tests, Aufnahmen, Bedienung und Grenzen](tools/benchmarks/results/furnaces-rain-2026-10-20/README.md).

## Phase 88: Rauchshader, Adaptive Rohre Und Bewegliche Oeffnungen

Rauchzellen bleiben autoritativer, gespeicherter Zustand, werden aber als
gebatchte Shader-Wolken und geglaetteter lokaler Tiefennebel dargestellt,
nicht als sichtbare Wuerfel. Der gerichtete Austritt folgt dem tatsaechlichen
Rohrport. Geschlossene brennende Raeume sammeln eine Zelle pro Sekunde;
ein offenes Fenster mit Aussenverbindung entlueftet 0,25, jede offene Tuerzelle
0,75 Zellen pro Sekunde. Diese Raten wirken gegen die laufende Erzeugung.
Ein freies Wandloch beziehungsweise offener Abzug baut zwei Zellen pro Sekunde ab.
Unbekannte und zu grosse Suchraeume werden nicht als sicher geschlossen behandelt.

Ein baubares Rohritem 60 waehlt anhand Anschluss und Spielerblick gerade,
gebogene oder schraege Posen. Fuenfzehn logische Portpaare bleiben am
Voxelraster; schraege Bogenmodelle aendern nicht die Nachbarschaftstopologie.
Alte Rohr-IDs 61 bis 64 behalten Modelle, Namen und Inventarmetadaten,
erscheinen aber nicht mehr als getrennte TMI-Angebote oder Rezepte.

Neue Oak-/Birch-/Spruce-Tueren 65 bis 67 belegen zwei Hoehenzellen,
Fenster 68 bis 70 eine. Autoritatives Use/Place schaltet ihre gespeicherten
Objektzustaende; Schliessen in einen Spieler wird abgelehnt. Sichtbare Fluegel
oeffnen und schliessen geglaettet in 0,45 Sekunden. Offene Durchgaenge erhalten
passende Kollisionen und beeinflussen den Rauchabzug nur mit echter Aussenverbindung.
Ofen, Rohre und Holzbauteile sind referenzinspiriert, keine behauptete 1:1-Kopie.

Creative kann mit R ueber einem TMI-Item das Rezept ansehen, ohne einen Stapel
zu erhalten. Der Ofen ordnet Eingang, Brennstoff, Bellows und Ausgabe in einem
kompakten, nicht intern scrollenden Layout an. Verdoppelte 80-Pixel-Kategorietabs
zeigen Building, Decoration, Materials, Animal Materials, Food und Tools;
eigene Modkategorien bleiben moeglich. Alte Rohrinventare behalten ihre Metadaten.
Alle UI-Aenderungen verwenden weiterhin TiltUI.

Netzwerkprotokoll 51, Regionsformat RGA0, Spielerformat PLY4 und Generator 5.
Keine neue Speicherung, Fluessigkeits-/Rauchengine oder Dependency-Version.
Der native Gameplaylauf besteht mit 13 Aufnahmen: autoritative Platzierung,
Ofenbedienung, scrollbarfreies normales/kompaktes Layout, animierte Tueren/Fenster,
Creative-R ohne Itemvergabe und gespeicherte Objekt-/Ofenzustaende.
Zehn weitere native Aufnahmen pruefen Modelle, zunehmenden Rauchnebel und
gerichteten Austritt; der Dedicated Server startet ohne Display.
Konkrete Grenzen, Workspace-Ergebnisse und historische Testkorrekturen:
[Phase-88-Bericht](tools/benchmarks/results/openings-smoke-2026-10-20/README.md).

## Phase 89: Ofendetails, Freie Oeffnungen Und Adaptiver Itembrowser

Tueren und Fenster blockieren beim Interaktionsklick weder Spielerbewegung noch
Maus-Capture. Der Klick wird trotzdem verbraucht, statt nebenbei einen Block zu
setzen. Der Server verhindert Fluegelbewegungen in Spieler auch beim Oeffnen;
reale Avian-Kapseltests pruefen Durchgang und Rueckzug fuer alle Holzvarianten.

Ofenbogen, Querbaender und Dachkurse haben keine koplanaren Deckflaechen mehr.
Gerade Rohre und der obere Ofenblock schliessen den Zwischenraum zu umgebenden
Vollbloecken mit deren jeweiligen Materialien, einschliesslich gemischter
Nachbarn. Diese Geometrie verwendet vorhandene Section-Batches, Auswahl und
Kollision; der innere Rauchkanal bleibt offen.

Oeffnungs-/Schliesssounds, leises Ofenfeuer und lokales Rauchhusten verwenden
den vorhandenen Audiomixer. Offene Fenster und echte Wandloecher erhalten
sichtbare, nach oben ziehende Shader-Rauchfahnen aus der bestehenden
budgetierten Raumpruefung. Keine zweite Rauchsimulation oder neue Speicherung.

Ofenslots zeigen Icons und Balken ohne Fuel-/Materiallabels oder Prozenttext.
TMI beginnt mit All und wechselt bei Platzmangel zu einer echten TiltUI-
Kategorieauswahl. Eigene Kategorien bleiben erhalten. Native Layoutpruefungen
erfassen auch ausgeblendete Popupoptionen und die gewaehlte Kategorie.

1449 Workspace-Tests bestanden, ein vorhandener Langzeit-UDP-Test ignoriert.
Formatierung, Check, Clippy mit strengen Warnungen und Workspace-Build bestehen.
Native Vulkan-Laeufe liefern 13 Gameplay- und 12 Modell-/Shaderaufnahmen;
der Headless-Server beendet 27 Sections erfolgreich. Menschliche Hoerabnahme
und langfristige Lastmessungen bleiben offen. Keine IDs, Generatorversion,
Protokollversion oder Dependencies geaendert.
[Nachweise, Aufnahmen und konkrete Grenzen](tools/benchmarks/results/furniture-fixes-2026-10-20/README.md).

## Phase 90: Kollisionsrevisionen Und Ofenpolitur

Tuer-/Fenstertoggles invalidieren jetzt die tatsaechlich betroffenen Sections
ueber den gemeinsamen asynchronen Objekt-Commit. Zuvor blieb deren Serverdaten-
Revision zurueck, wodurch die Kollisionsbereitschaft weitere Bewegung dauerhaft
verweigerte und der Client zurueckgesetzt wurde. Der native Survivaltest prueft
nach dem Oeffnen jetzt auch lokale und bestaetigte Serverbewegung.

Offene Aussenfenster entfernen 0,5 Rauchzellen pro Sekunde; mehrere Oefen teilen
diese Leistung weiterhin pro Raum. Die sichtbare Auslassstaerke waechst in
fuenf weich angeglichenen Zonen mit der angesammelten Rauchmenge, ohne hoehere
Partikel-/Suchbudgets. Drei leisere aufgenommene Husten-Takes ohne direkte
Wiederholung und ein besser hoerbarer Ofenloop verwenden den vorhandenen Mixer.
Fackelintensitaet steigt um 25% bei erhaltener Soft-Core-Kurve und Reichweite.

Tueren zeigen Planken, Holzdetails und Beschlaege auch auf der Rueckseite.
Gerade Rohre und der obere Ofenblock schliessen ihre Zwischenraeume jetzt schon
zu einzelnen opaken Nachbarbloecken mit deren jeweiligem Material. Fehlende
Nachbarn bleiben frei; Eckbesitz verhindert uebereinanderliegende Fuellungen.
Die Fuellung reicht bis zur Rohrwand, nicht in den Rauchkanal.

1453 Tests bestehen, ein vorhandener UDP-Langzeittest bleibt ignoriert.
Formatierung, Workspace-Check, Clippy mit `-D warnings` und Build bestehen.
Native Laeufe liefern 13 Gameplay- und 14 Modell-/Shaderaufnahmen; der Dedicated
Server startet ohne Display und beendet 27 Sections erfolgreich. Rueckseiten
und Einblock-Rohranschluss wurden als Bilder kontrolliert. Die Rueckseite
erhoeht die Tuergeometrie, nicht deren Materialbatchzahl. Keine neue FPS-Paritaet
oder menschliche Hoerabnahme behauptet. IDs, Seeds, Formate, Generator,
Netzwerkprotokoll und Dependencies bleiben erhalten.
[Pruefbericht und konkrete Grenzen](tools/benchmarks/results/furnace-contact-polish-2026-10-20/README.md).

## Phase 91: Simulations-Lifecycle Und Budgetierte Tiernavigation

Bestaetigte Replica-Imports und Section-Reloads erzeugen keine ungenutzten
Fluid-/Grasauftraege mehr im Client. Daten, Quellen, Revisionen sowie Terrain-
und Wasserinvalidierungen bleiben erhalten; autoritative Wiederaufnahme bleibt
aktiv. Der native Vorherlauf zeigte 1014 nicht abgearbeitete Clientauftraege;
im Nachherlauf bleiben alle 60 Langzeitproben bei null.

Blockierte Kuehe, Schafe und Schweine verwenden den vorhandenen Routen-Cache
und das globale Acht-Suchen-Budget. Anstehende Beduerfnisrouten werden fair
bedient, auch bei mehr als 64 blockierten Tieren. Die Wassersuche nutzt bereits
vorbereitete Spalten statt wiederholter Fluss-/Hoehlen-/Klimaabfragen.
Grafikqualitaet und Tierdichte werden dafuer nicht reduziert.

Rohrfuellungen entstehen jetzt erst ab drei opaken Nachbarn. Weniger Nachbarn
bleiben frei; gemischte Materialien, offenes Lumen und gemeinsame Modell-/
Auswahl-/Kollisionsgeometrie bleiben erhalten.

Der opt-in Langzeitmodus erfasst echte Framefenster, Ressourcen und CPU-Scopes.
Die Flugroute bewegt den echten Spieler mit Serverbestaetigungen statt nur die
Kamera; endliche Block-/Wasserzyklen pruefen echte Quelle, Fluss und Drainage.
Isolierte gespeicherte Welten verwenden die bestehende WorldLibrary, keine
zweite Speicherarchitektur. Ein Offline-Fixturegenerator erstellt einen
geschlossenen Raum mit regularem Ofen und Brennstoff fuer gezielte Rauchtests.

30 Minuten nativ (15 stationaer, 15 mit sieben vollstaendigen Hin-/Rueckwegen)
und 15 Minuten mit kopierter gespeicherter Welt sind ohne Renderfehler beendet.
Zusaetzlich bestehen 15 Minuten mit einem regulaer gefuellten Rauchraum
(256 Zellen, 107,55 FPS zum Ende) und zwoelf echte Quell-/Abfluss-/Terrainzyklen.
Der gemeldete Unter-18-FPS-Kollaps war bereits im Vorherlauf nicht reproduziert;
keine alleinige Ursache oder pauschale Behebung behauptet. Aufloesungsdrift am
Ende des Flugtests und erhoehter CPU-/GPU-Speicher sind transparent dokumentiert.
1480 Workspace-Tests und zwei Fixturetests sowie Formatierung, Check, Clippy
mit strengen Warnungen und Build bestehen; der Dedicated Server bleibt headless.
Seeds, IDs, Generator-/Speicher-/Protokollvertraege und Dependencies bleiben gleich.
[Messwerte, Regressionstests und Grenzen](tools/benchmarks/results/progressive-collapse-2026-10-20/README.md).

## Phase 92: Bodenstreaming, Wetterbindung Und Font-Lifecycle

Terrain und Props verarbeiten veroeffentlichte/entfernte Sections und
vorbereitete Spalten inkrementell. Wartende Kandidaten ueberleben belegte
Worker und abgelaufene Bevy-Meldungen. Kuesten- und Exposure-Caches werden
lokal invalidiert; echte globale Aenderungen behalten ihren sicheren
Vollpfad. Dispatch-Buchhaltung allein invalidiert keine Renderdaten mehr.
Animationsenden pruefen nur aktuelle betroffene Roots erneut. Ein weiches
2-ms-Vorbereitungsbudget gilt erst nach Anfangsbereitschaft, ohne das
Workerlimit der initialen Ladung zu senken.

Wind, Wetterfarbton, Regen-/Schneenaesse und Pflanzenwachstumsuhr nutzen
einen stabilen gemeinsamen 48-Byte-GPU-Puffer. Dynamische Werte erzeugen
keine Material-Modified-Ereignisse mehr fuer diese Kanaele und damit keine
zusaetzliche Extraktion saemtlicher zugehoeriger Meshes. Vorbereitete
Spalten beschleunigen trockene Stroemungsabfragen; Replica-Reloads erzeugen
keine ungenutzten autoritativen Segmentauftraege.

Ein nativer Image-Audit ordnete ueber 99,9% des CPU-Bildwachstums neuen
Systemfont-Blob-IDs und deren 1-MiB-Atlanten zu. Der bestehende Fontique-
Sourcecache ist jetzt shared; tatsaechlich verwendete Nicht-Asset-Fontdaten
bleiben in einem auf 32 Blobs/64 MiB begrenzten LRU erhalten. CSS und
Schriftdateien bleiben gleich. Cache-Pruning, Deduplizierung, Freigabe und
Budgets besitzen Regressionstests; ein nativer Systemfont besteht 256
Prune-/Reload-Zyklen ohne ID-Wechsel.

Die echten 4K-Gehpaare derselben gespeicherten Strecke liefern 73,05 ->
87,95 FPS (Weite 12) und 43,71 -> 63,79 FPS (Weite 20/Regen). Der p99 sinkt
von 28,615 auf 20,572 ms bzw. 42,805 auf 30,622 ms. Grafikqualitaet,
Vegetation, Wasser, Schatten und Sichtweiten werden nicht reduziert.
Zwischenstaende, Langzeitmessungen, Ressourcen und verbleibende Spitzen
stehen im [Phase-92-Pruefbericht](tools/benchmarks/results/ground-streaming-2026-10-21/README.md).
Der finale 15-Minuten-Gehloop besteht mit 35 bestaetigten Rueckwegen,
85,18 FPS und p99 21,559 ms. Alle 30 Proben bleiben bei 11 Schriftatlanten,
drei Font-IDs und zwei Pins; die Bildzahl bleibt bei 192..194. Keine Render-
oder Laufzeitwarnungen. Der Prozess-RSS steigt dennoch weiter; das ist
keine allgemeine Speicherparitaet und kein 30-Minuten-Nachweis.
1506 Workspace-Tests, Formatierung, Check, strenges Clippy und Build
bestehen. Der Dedicated Server startet ohne Display mit 27 Sections.
Kein Generator-, Speicher-, Protokoll- oder Dependency-Wechsel.

Erst nach den Messungen wurden zwoelf alte Testbinaries aus dem RAM-
Dateisystem auf die Datenplatte archiviert: rund 10,69 GiB `/tmp`-Belegung
entfallen, alte Pfade bleiben als Symlinks und alle SHA-256-Pruefsummen
erhalten. Daraus wird keine gemessene Spielbeschleunigung abgeleitet.

## 93. Auswahlbudgets, Schattenressourcen Und Laufpraesentation

Globale Terrain-/LOD-Abgleiche nach dem Initialladen auf kurze, koaleszierte
Slices verteilt; neue Sections separat, lokale Retrys dedupliziert und
Weltwechsel-/Eviction-Queues sauber bereinigt. Der gemeinsame Kartenatlas
haelt einen inkrementellen Metadatenindex statt ihn fuer jede Planung neu
aufzubauen; ueberholte Worker duerfen aktuelle Tiles nicht ersetzen.

Ausgeschaltete Schatten erzeugen und laden keine Proxy-Meshes mehr und
geben alte Proxies frei. Wieder angeschaltete Schatten verwenden sofort
korrekte Originalcaster; optimierte Proxies folgen bei regulaerer
Section-Komposition. Verspaetete Grasuploads uebernehmen die mittlerweile
sichtbare Wachstumsgroesse; Previous-Windgewichte sind zeitlich konsistent.

Sanfter distanzabhaengiger Geh-/Sprintrhythmus und Landefeder fuer die
Firstperson-Kamera, abschaltbar unter Gameplay / Walking Camera Motion.
Physik, Netzposition, Blickrotation und Chunk-Fokus bleiben unabhaengig.
GPU-Indirect-Diagnostik liest neun Phasen an den tatsaechlichen
Early-/Late-/Color-Grenzen pro Kamera statt nach spaeteren Bufferresets;
Readback bleibt auf 8 MiB, einen Auftrag und jeden 60. Frame begrenzt.
Eigene Hoehlen-/Unterwasser-/Rauch-Fullscreen-Draws werden direkt gezaehlt.
Private Tilt-/Gradient- und engineinterne Fullscreen-Draws bleiben offen.

1545 Workspace-Tests, Formatierung, Check, strenges Clippy, Build,
elf Messwerkzeugtests und der Headless-Serverstart bestehen. Native
Messungen und ihre Grenzen im
[Phase-93-Bericht](tools/benchmarks/results/render-walking-2026-10-21/README.md).
Kein Generator-, Speicher-, Protokoll-, Seed- oder Dependency-Wechsel.

## 94. Bauteile, Faerbbare Betten Und Jahreszeiten

24 datengetriebene Bauteile ergaenzen die bestehende Registry: dreistufige
Treppen, Halbplatten und verbindende Zaeune aus acht Materialfamilien.
Die gemeinsame Boxgeometrie speist Section-Meshing, Picking und Kollision;
normale Bauteile erhalten keine eigenen GLBs. Drei wiederverwendete
Bettmodelle belegen zwei horizontale Zellen. Sechs Blumen-/Mischfarbstoffe
faerben ihre Decken; Farbe, Ausrichtung und Holzart bleiben beim Abbau und
Save/Reload erhalten. Gewoehnliche halbe Stufen sind begehbar, ohne das
optionale Ein-Block-Auto-Climb einzuschalten.
Bettfarben werden ausschliesslich zugehoerigen Bett-Platzierungszustaenden
zugeordnet. Geplante Objektmetadaten werden vor der ersten Weltmutation
validiert; eine fremde Creative-Itemquelle kann keinen Ofen oder andere
Bauteile mit ungueltigen Bett-Facingdaten hinterlassen.

Autoritatives Schlafen respektiert die saisonalen Startzeiten (Fruehling/
Herbst 19:00, Sommer 20:00, Winter 18:00). Alle verbundenen lebenden
Nicht-Spectator-Spieler muessen eigene Betten benutzen; danach wird es
08:00 Uhr. Ungueltige Schlafplaetze werden entfernt. Die Liegepose ist
animiert; eine horizontale physikalische Schlafkapsel ist noch offen.

Der gespeicherte Weltkalender besitzt vier konfigurierbare Jahreszeiten
mit weichen Uebergaengen. Laub-/Grasfarben, Lichtanmutung, Temperatur und
Wetterwahrscheinlichkeiten nutzen denselben Kalender. Klima und Hoehe
bestimmen Regen, Schnee und Schmelzen ohne neue Winterbiome oder Retrogen.
Kontinuierliche Schneebedeckung umfasst auch exponiertes Gras. Natuerliche
Laubkronen werden saisonal samt Schatten ausgeduennt; Fichten und gesetzte
Blaetter bleiben erhalten. Schnee und Laubfall sind kosmetisch. Begrenztes
Klima-Catch-up ist keine vollstaendige historische Wettersimulation.
Blattdetails und Schatten behalten ihre urspruengliche Zellherkunft; der
Laubfall muss keine Provenienz aus versetzten Mikrooberflaechen erraten.
Ein eigener materiallokaler Blatt-Prepass korrigiert Bevys fehlende
Vertex-Alpha-Pruefung in Tiefe/Schatten. Unsichtbare Winterblaetter
verdecken dadurch keine Aeste mehr mit einer Geistermaske. Fasern,
Flammen und Standardterrain verwenden unveraendert ihre bisherigen Regeln.
Nachladende Dach-/Kronendaten aktualisieren gezielt gleiche-XZ-Besitzer:
begrenzte Maskenvergleiche, unveraenderte Revisionen ohne Remesh und eine
Pre-Upload-Barriere gegen veraltete Worker-Ergebnisse, keine globale
Wetter-Neukomposition.
`SeasonSample` bleibt der gemeinsame Erweiterungspunkt fuer Wachstum,
Ernten und Tierverhalten, keine bereits eingefuehrte neue Erntemechanik.

Creative erhaelt eine servervalidierte Muelltonne fuer Cursorstapel oder
ausgewaehlten Inventarslot, ohne Welt-Drop. Der alte Abbrechen-Footer
entfaellt, TMI-Inhalte werden zentriert. Chat bleibt bei Submit, Klick und
Fokusverlust offen und schliesst nur mit Escape. Grasblockabbau liefert
in Survival Dirt, Creative-Picking bleibt unveraendert.

Pruefstatus, native Aufnahmen und Grenzen stehen im
[Phase-94-Bericht](tools/benchmarks/results/seasons-construction-2026-10-21/README.md).
1624 Workspace-Tests, Formatierung, Check, strenges Clippy und Build bestehen;
zwei bestehende Tests bleiben ignoriert. Native Galerie, Jahreszeiten,
Survival-Bettablauf, Inventar mit echter OS-Maus, Chat und Gameplay-Menue
sowie der Headless-Serverstart sind bestanden. Elf Benchmark-Werkzeugtests
bleiben gruen. Winterfragmente wurden am nativen Vorher-/Nachherbild geprueft.
Keine exakte Referenzkopie oder neue FPS-/Speicherparitaet behauptet.
Generator-/Regionsversionen, Seeds und bestehende IDs bleiben erhalten;
neue Netzwerkzustaende benoetigen gemeinsam aktualisierte Builds.

## 95. Bauteilausrichtung, Materialdetails Und Pferde

Gleiche Halbplatten verbinden sich horizontal oder vertikal in einer Zelle;
Survival verbraucht genau eine weitere Haelfte und liefert zwei beim Abbau.
Obere und zusammengefuegte Platten bilden auch eine gueltige Bettauflage.
Treppen besitzen 24 Boden-/Decken-/Wandrotationen, spielerzugewandte Stufen
und gemeinsame Eckprofile fuer Meshing, Picking und Physik. Sandzaun 95
bleibt nur fuer bestehende Spielstaende lesbar. Der abweichende Farbstoff-
Klickbezeichner ist korrigiert und TMI-Kategoriehosts werden zentriert.

`/season set spring|summer|autumn|winter` nutzt Chat und Serverkonsole mit
autoritativer Kalender-/Speicherintegration; die Jahreszeit steht neben der
MiniMap-Uhr. `UI FPS` ist eine persistente Grafikoption (Default 60), die
retained Ansichtsaktualisierungen begrenzt, nicht die Spielschleife ausbremst.

Bestehendes Terrain-PBR um materialabhaengige analytische Bump-Normalen,
Rauheit und Farbdetails erweitert. Gemeinsame Nahgeometrie, unregelmaessige
Grasraender, feinere Sand-/Schneeprofile und Kontaktfarbsaeume behalten
geschlossene Zellhuellen und Materialidentitaeten. Eiche, Birke und natuerliche
Fichte nutzen begrenzte, einzeln initialisierte Kronenprofile. Echte
Qualitaetsregler verwenden die bestehende Grafikdatei und Renderrevision,
keine Weltedits, per-Detail-Entities oder zusaetzlichen Fullscreen-Passes.

Pferde erweitern die vorhandene Artliste mit 16 Fellvarianten und gemeinsamen
Modellteilen. Leben 20, Hunger 30, Durst 25; seedbasierte Plains-Herden und
`/spawn teramic::horse`. Stabile Kapazitaeten: Ausdauer 65..200, Sprunghoehe
1,1..2,25 m. Bestehendes Fressen, Trinken, Schlafen, Schwimmen und Navigation
werden verwendet; Hindernisspruenge sind ballistisch und kollisionsgeprueft.
Kein bereits fertiggestelltes Reit-/Zaehmungs-/Zuchtsystem.

Pruefungen, native Ansichten, Performance und konkrete Grenzen stehen im
[Phase-95-Bericht](tools/benchmarks/results/construction-materials-horses-2026-10-21/README.md).
1642 Workspace-Tests, Format, Check, strenges Clippy und Build bestehen.
Native Material-/Bauteil-, Pferde-, Saison-, Menue-/Inventarpruefungen und
Headless-Start sind dokumentiert. Zwei 15-Minuten-4K-Standlaeufe zeigen
108,43 -> 112,26 FPS im gewichteten Mittel ohne zeitlichen Kollaps in dieser
Szene. Erste Terrain-Komposition kostet dabei 20,4% mehr Workerzeit, spaetes
VRAM rund 279 MiB mehr. Keine allgemeine Speicher-/Streamingparitaet.
Keine behauptete 1:1-Referenzkopie, neue GI oder separate Normalmap-Bibliothek.

## 96. Gemischte Halbplatten, Winterdecke Und Stabile Einblendung

Halbplatten unterschiedlicher Materialien teilen eine Zelle in allen sechs
Ausrichtungen. Der vorhandene Facing-State speichert Material und Schnitt,
ohne bestehende IDs oder Regionsfelder umzudeuten. Survival verbraucht eine
weitere Haelfte und liefert beide Materialien beim Abbau. Seitenklicks oben
oder unten setzen horizontale Platten; die Mitte haftet vertikal an der Wand.
Treppenaufstieg beginnt vor starkem Abbremsen und die Kamerahoehe wird kurz
geglattet, ohne die physische Spielerposition oder den Serverstrahl zu aendern.

Pferde haben zusammenhaengende Koerper-/Fellflaechen statt sichtbarer Rippen,
gestufte Maehnen und differenzierte Abzeichen. Hindernisspruenge pruefen den
Aufstiegsbogen; Probe-, Sprung- und Landepausen verhindern schnelles Umschalten
blockierter Tiere. Stein-/Erdrelief und der gemeinsame Terrain-PBR erhalten
unregelmaessigere Vertiefungen, Risse und Klumpen bei unveraendertem Batching.
Dies ist keine behauptete exakte 1:1-Art-Kopie.

Schneeschicht 98 besitzt sieben partielle Hoehen in 1/8-m-Schritten; Stufe 8
wird zum bestehenden vollen Schneeblock 5. Rendering, Picking und Kollision
verwenden dieselben Bounds. Die vorhandene autoritative Wetterarbeit speichert
die Schichten ueber das Regionsjournal, erhaelt Schnee im Winter und laesst ihn
auch unter natuerlichen Baumkronen liegen. Gruene Grasraender werden weiss,
Gras unter Schnee bleibt erhalten. Highland-Niederschlag wird zu Schnee.
Spielerdaecher und fremde Blockedits werden nicht ueberschrieben; stabile
Schichten erzeugen keine neuen Revisionen oder Meshauftraege.
Tiernavigation und Bodenpruefung verwenden ebenfalls die echte Schneekappenhoehe,
damit Saeugetiere auf duennen Schichten nicht einen Block hoch schweben.

TiltUI behaelt den bestehenden HUD-Baum unter Pause, Optionen und Inventar.
Reine Hotbar-Anzeigeslots vermeiden doppelte Inventar-Interaktionsziele.
Noch nicht eingeblendete Chunks behalten ihren Reveal-Zustand bei nachladenden
Nachbarrevisionen. Der Wettershader trennt Klima-Vertexalpha von tatsaechlicher
Materialdeckkraft, statt Fade-Materialien versehentlich wieder opak zu machen.

Gezielte Regressionen, native Kurztests und Grenzen:
[Phase-96-Bericht](tools/benchmarks/results/slabs-snow-horses-2026-10-21/README.md).
1652 Workspace-Tests, Format, Check, strenges Clippy und Build bestehen;
zwei bestehende Tests bleiben ignoriert. Native Material-, Pferde-, Saison-,
GPU-Deckkraft-, Chunk- und echte OS-Inventar-/Pausen-/Optionsproben sowie der
Headless-Start sind dokumentiert. Der Bericht nennt auch einen abgebrochenen
OS-Zeigerversuch und den Screenshot-Callback beim schnellen Animations-Testende.
Auf ausdruecklichen Wunsch keine neuen 10-/15-/30-Minuten-Laeufe und keine
daraus erfundenen FPS-/Speichervergleiche. Generator und Speicherformate bleiben
gleich; Client und Server benoetigen gemeinsam Protokoll 54.

## 97. Sichtbare Platzierung, Fernschatten Und Kalender-HUD

Terrain-Uploads unterscheiden die letzte lokale Blockrevision von reinen
Nachbarinvalidierungen. Bauen wird sofort sichtbar, auch in zuvor leeren
Sections; erstmals geladene Chunks behalten ihre Einblendung. Ersetzte
Einblendungen geben temporaere Materialien und Readiness-Eintraege frei.

Sonnen-/Mondschatten folgen der Sichtweite bis maximal 160/256/512 m fuer
Low/Medium/High. Medium verwendet drei, High vier Kaskaden. Fernterrain wirft
wieder Schatten; Glas, Wasser und Fackeln bleiben ausgenommen. Die groessere
Reichweite kostet GPU-Arbeit und ist kein Performance-Fix.

Pferderumpf und Beinabstand sind 30% schmaler. Herdentrennung und Schlafbeginn
brechen einen laufenden Sprung nicht mehr ab; Schwerkraft wird innerhalb der
Kollisions-Teilschritte integriert. Ein autoritativer Regressionstest prueft
Anlauf, echten Hoehengewinn und Landung auf einer einen Meter hohen Stufe.

Gras, Buesche, Blumen und kleine Pilze verwenden dickere, gestufte kubische
Formen mit echten Zwischenraeumen, unveraenderter Artenfarbzuordnung und Wind.
Gemeinsame Render-/Picking-Geometrie und Section-Batching bleiben erhalten.
Die nativen Galerien zeigen die neue Formensprache, keine behauptete exakte
1:1-Referenzkopie.

Der TiltUI-Minimapring zeigt vier Jahreszeiten, Symbole, Teilstriche und den
Fortschritt des autoritativen Kalenders. Unterwasser ersetzt Luft die bisherige
Staminaschiene samt Symbol/Farbe. Stamina ist beim Tauchen unbegrenzt; Schwimmen
an der Oberflaeche verbraucht weiterhin Energie. Nach dem Auftauchen bleibt die
Luftanzeige bis zur vollstaendigen Auffuellung sichtbar und wechselt dann zurueck.

1656 Workspace-Tests bestehen; zwei bestehende Tests bleiben ignoriert.
Format, Workspace-Check, strenges Clippy und Client-/Server-Build bestehen.
Kurze native RX-6950-XT-Aufnahmen pruefen Props, Pferdemodelle, Fernschatten,
echten UDP-Bauablauf sowie alle vier Jahreszeiten mit realem Survival-Tauchen.
Headless-Serverstart ohne Display besteht. Keine neuen Langzeittests und keine
erfundenen Vorher-/Nachher-FPS-Werte. Details, Budgets, Bilder und Logs:
[Phase-97-Bericht](tools/benchmarks/results/placement-props-season-hud-2026-10-21/README.md).

## 98. Prop-Details, Weltregeln Und Kontaktkorrekturen

Gras erhaelt mehr kantige Sprosse, Seitenblaetter, Knospen und kleine Blueten.
Buesche besitzen mehrschichtige Blattbloecke und Beeren-/Bluetenvarianten.
Blumen verwenden mehrere Staengel, segmentierte Blaetter, Kelche, radiale
Blueten und offene Glocken. Kleine Pilze erhalten gestufte Kappen, Lamellen,
Flecken und Begleiter. Near-Details bleiben in gemeinsamen Section-Batches;
die billigen Middle-/Far-Stufen, Pflanzen-IDs, Dichte und Abbaubarkeit bleiben
erhalten. Native Aufnahmen dokumentieren den Stand, keine exakte Referenzkopie.

Tierbewegung liest die vorhandenen Kollisionsprofile fuer Halbplatten, Schnee
und andere Teilbloecke. Gedrehte Koerper und Hufproben ersetzen quadratische
Maximalradien; Richtungswechsel duerfen nicht in gesperrte Bereiche drehen.
Pferdespruenge enden erst bei Bodenkontakt, nicht allein nach Timerablauf.
Vertikale Slab-Platzierung benutzt client- und serverseitig denselben
Halbzellenraum; obere/untere Klicks setzen vorhandene vertikale Ebenen fort.

`/gamerule` und die Dedicated-Konsole unterstuetzen `sleep_percentage`,
`season_swap_interval`, `game_time_speed` und `list [page]` mit acht Regeln
je Seite. Validierte Werte werden autoritativ repliziert und im bestehenden
`world.toml` gespeichert. Die Zeitgeschwindigkeit veraendert nicht die aktuelle
Uhrzeit; 0 pausiert sie. Schlafquoten runden auf ganze Spieler auf. Auch 0%
benoetigt mindestens eine zwei Sekunden alte Schlafstimme.

Der Katalog zeigt nur weisse Grundbetten. 18 einzelne shapeless JSON-Rezepte
faerben beziehungsweise faerben die vorhandenen Holzbetten neu. Bestehende
gefaerbte Inventarobjekte bleiben gueltig. Der schlafende Avatar liegt hoeher
auf der Decke. Saisonuebergaenge beginnen am letzten Tag. Natuerlicher Schnee
waechst nicht auf vollen Schneebloecken weiter; Spieler koennen weiter stapeln.

TiltUI-Tooltips werden ausserhalb scrollbarer Vorfahren absolut positioniert
und mit ihrem urspruenglichen Besitzer entfernt. Die Minimap besitzt einen
ornamentalen Jahreszeitenrahmen mit transparentem Kartenfenster, zentrierten
Himmelsrichtungen und dynamischer Kalendernadel.

Nachweise, Geometriebudgets, native Bilder und verbleibende Abnahmegrenzen:
[Phase-98-Bericht](tools/benchmarks/results/game-rules-botany-2026-10-21/README.md).
1666 Workspace-Tests bestehen; zwei bestehende Langzeittests bleiben ignoriert.
Format, Check, strenges Clippy, Build und die dokumentierten nativen Kurzproben bestehen.
Keine neuen Dependencies, Generator- oder Regions-/Spielerdateiformate.
Client und Server benoetigen gemeinsam Protokoll 55.

## 99. Verlaessliche Spalten-Caches, UI Und Wetter-Schnee

Der gemeldete `ColumnMismatch` ist reproduziert und behoben: vorbereitete
Terrain-Spalten werden nur gegen Terrain-Parameter geprueft. Live-Aenderungen
von Spielregeln, Kalender und Zeit benoetigen keine neuen Spalten und lassen
bereits laufende Worker, nachgeladene Hoehen sowie Abbau-/Platzierungs-Remeshes
weiterarbeiten. Andere Seeds, Welt-IDs und Terrain-Vertraege bleiben ungueltig.

Inventar und Itembrowser behalten ihren Slotbaum beim Schliessen. Tooltips
werden erst vermessen und im ersten sichtbaren Frame am Cursor positioniert;
sie bleiben ausserhalb von Scrollcontainern. Geschlossene Tooltips schreiben
keinen unveraenderten Sichtbarkeitszustand pro Frame. Bauablehnungen verwenden
wiederverwendete TiltUI-Warn-/Fehlertoasts oben rechts.

Neue Betten richten das Kopfende in Blickrichtung aus. Schlafen oeffnet den Chat
und blendet einen echten TiltUI-Old-Film-Filter mit bis zu 80% Abdunklung ein.
Unfertige Nachrichten bleiben beim Aufwachen erhalten. Kalendernadel und Kontur
sind silbern; die Buchstaben sitzen mittig in den tatsaechlichen Rahmenrauten.
Das Hauptmenue zeigt wieder sein Multiplayer-Icon.

Winter erzeugt keine spontane Schneedecke mehr. Nur Schneefall akkumuliert,
abhaengig von seiner Staerke; bestehender Schnee bleibt im Winter erhalten.
Betten, Oefen, Truhen und Werkbaenke erhalten weder Auflagen noch Shader-Schnee.
Alte wettermarkierte Auflagen werden dabei von Spielerbloecken unterschieden.

Native Kurzpruefungen, Regressionstests, Bilder und Grenzen:
[Phase-99-Bericht](tools/benchmarks/results/streaming-ui-snow-2026-10-22/README.md).
Keine Dependency-, Protokoll-, Generator- oder Speicherformat-Aenderung.
Keine Reduktion der Sichtweite oder der Grafikqualitaet; kein neuer Langzeitlauf.

## 100. FPS-Regression Durch Verstecktes Inventar Behoben

Die dauerhaft montierten Slots und der Itembrowser aus Phase 99 wurden trotz
`display: none` weiter von TiltUI verarbeitet. Die Vorlage behaelt jetzt nur
die leichte Inventarhuelle; Slots/Tooltips werden beim Schliessen freigegeben.
Gemeinsame Bilder, der Welt-HUD und der Schutz vor Layout-/Tooltip-Flackern
bleiben erhalten. Die native Probe prueft die Freigabe auch nach Wiederoeffnen.

Native Kurzpaare auf Ryzen 5900X / RX 6950 XT: Stand/12 steigt von 63,27 auf
111,09 FPS, normales Gehen/12 von 63,10 auf 114,62 FPS und 4K/Regen/20 mit Meer
von 34,53 auf 68,58 FPS. Eine Rueckkontrolle mit dem alten HTML stellt den
Verlust wieder her. Dritter Seed mit Schnee Stufe 3: 115,19 FPS, ohne Vorherpaar.
Keine Reduktion von Sichtweite, Weltgeometrie, Vegetation oder Wasserqualitaet.
Keine neuen 10-/15-/30-Minuten-Laeufe und keine allgemeine Langzeitfreigabe.
[Messprotokoll](tools/benchmarks/results/inventory-idle-regression-2026-10-22/README.md).

## 101. Bereinigter Backlog, Audiobusse Und Teilblock-Wasser

TODO.md trennt Features und Optimierungen mit stabilen IDs; elf neue
Featurebereiche sind nach Abhaengigkeiten eingeordnet. Alle 100 alten offenen
Eintraege bleiben durch eine vollstaendige Zuordnung und den unveraenderten
Altstand nachvollziehbar. Nicht implementierte Funktionen und externe
Abnahmen werden nicht als erledigt verschoben.

Die bestehende Mob-Registry liest marine Spawngewichte, Gewaessertypen,
Tiefen-/Biomfilter und optionale Herdengroessen aus den Arten-JSONs. Die zehn
marinen Defaults erhalten die alte Seed-Auswahl exakt. Ungeladene Zielkoerper
warten auf Nachladen, statt den Habitatbereich dauerhaft zu ueberspringen.
Gespeicherte IDs und Populationen bleiben erhalten. Altaufgabe 027 ist damit
umgesetzt, von 039 nur der Herdenteil; Nahrung/Verbrauch/Limits bleiben offen.

Tiere, Schritte und Umgebung besitzen getrennte live einstellbare Audiobusse
einschliesslich ihrer Hallfahnen. Master/Mute, Speichern, Cancel und Reset
verwenden die bestehende AudioConfig. Bereits stumme Kategorien bleiben beim
Decoderstart ab dem ersten Cue still. UI-/Wasserregler bleiben unabhaengig;
keine neuen Audioquellen oder Audio-Entities. Dies schliesst O12 und den
Reglerteil von Alt 042/082, nicht die menschliche Hoer-/Mixabnahme.

Die nativen Gegenproben finden zwei weitere konkrete Fehler: GPU-Readbacks
von Item-Vorschauen behalten jetzt ihre Screenshot-Entity bis zum Callback,
statt sie nach Wallclock-Timeout zu entfernen. Der einzelne Inflight-Slot
bleibt begrenzt, spaeter Erfolg liefert weiterhin das Thumbnail. Hohe
Wasserfaelle werden auch vom unteren Becken gehoert; ihr Oberlauf darf
oberhalb des normalen Hoer-Layer-Filters liegen. Raster, Klangposition und
Wanddaempfung bleiben erhalten. Die Menueprobe prueft Layoutwerte erst nach
PostUpdate; ihre Groessen-/Clippinggrenzen wurden nicht gelockert.

Sonnen-/Mondschatten werden weich auf maximal eine Werferhoehe Laenge
begrenzt; Himmelskoerper, Lux und Fernschatten bleiben erhalten (O02).
Der Wasser-Mesher verdeckt Wasserfallseiten neben Slabs/Treppen nur an den
tatsaechlichen festen ShapeBoxes. UV-/Gischtkontinuitaet, gespeicherte
Orientierungen und Anschlussaenderungen ueber Section-Grenzen sind abgesichert.
Nur Block-/Objektedits verwenden einen breiteren Wassergeometrie-Halo;
Fluidticks behalten ihre bisherigen Budgets. Vollstaendiges Waterlogging
bleibt unter O01 offen.

Alle fuenf Cargo-Pruefungen bestehen mit 1703 erfolgreichen Tests, null
Fehlschlaegen und zwei bestehenden ignorierten Tests. Native Menue-,
Audio-, Fluid- und Headless-Pruefungen sind bestanden. Kurze Kontrollen
auf drei Seeds messen 110,90 FPS beim Gehen, 89,52 FPS bei 4K/Regen/20
und 110,39 FPS bei Schnee/12; kein neues Vorher-/Nachherpaar oder Langzeitbeleg.

Implementierung, Regressionen und Validierung:
[Phase-101-Bericht](tools/benchmarks/results/technical-backlog-2026-10-22/README.md).
Keine neuen Dependency-, Netzwerk-, Generator- oder Speicherformatversionen.
Der gesamte alte technische Backlog ist damit noch nicht abgeschlossen.

## 102. Teilblock-Wasser, Streaming Und Render-Lifecycle

Wasser in Slabs/Treppen nutzt nur den freien Raum der bestehenden
Kollisionsprofile. Gemeinsame Oeffnungen bestimmen Zufuhr und Gravitation;
Wasserpolygone, Stroemung, Schwimmen und Unterwassereffekte beachten die
festen Teilvolumina. Explizite Quellen bleiben beim Ersetzen durch einen
Teilblock nicht aktiv. Restwasser drainiert, natuerliche Reservoirs werden
beim Nachladen korrekt in Wasserjobs aufgenommen. Keine neuen Fluidformate.

Streaming indexiert wartende Sections nach Spalten statt fuer jedes Jobbatch
die gesamte Warteliste zu durchsuchen. Vertikale Sichtweitenaenderungen
werden auch ohne Bewegung uebernommen. Kartografie begrenzt die Auswahl auf
vier Live-Snapshots/128 Arbeitseintraege und durchlaeuft bei grosser Historie
nur das kleinere Suchgebiet. Das neue WaterPublished-Ereignis verhindert
globale Fallback-Scans nach reinen Wasser-Mesh-Updates.
Section-Deltas markieren Ressourcen nur bei tatsaechlichen Aenderungen;
Duplikate/veraltete Daten bleiben validiert. SectionEditApplied lokalisiert
auch Prop-only-Importe statt globale Kartenindex-Rebuilds auszuloesen.

Simulationsarbeit ungeladener Besitzer wird freigegeben, gespeicherte Zellen
und geladene Grenzwarter bleiben erhalten. Leere Wasserpatches entfernen alte
Tiles auch bei ausgeschoepftem Uploadbudget. Prop-Windgewichte werden einmal
pro Template berechnet, Batches passend reserviert und Far-Props direkt
gepackt. U16-Indizes halbieren geeignete Indexbuffers ohne Dreiecksverlust;
grosse Batches behalten U32. Keine Verringerung von Dichte oder Detaildistanz.

Schatten-Off greift auch fuer bestehende Fackellichter. Getrennte Cave-/Mist-
Claims verhindern gegenseitigen Verlust gemeinsamer VolumetricFog-/Light-
Komponenten. Qualitaet, Daemmerung, TAA/MSAA und Soft-Core-Vertrag besitzen
zusaetzliche Regressionen und native Fixture-Ansichten.

Final geprueft: alle fuenf Cargo-Kommandos erfolgreich, 1734 Tests bestanden
(2 bestehende ignoriert), Headless-Server, echter Slab-Wasserdurchlass mit
Drainage und 31 native Licht-/AA-/Unterwasseransichten. Kurze Geh-/Flug- und
12/20/32er-Vergleiche mit drei Seeds enthalten auch Regen und Schnee.
32er-Kartenplanung Maximum 20,98 -> 1,23 ms; globale Kartenindex-Rebuilds
78 -> 1. Kein pauschaler FPS-/RAM-Gewinn: Schwankungen, ein schlechter
Regenlauf und hoehere 32er-RSS stehen unverkuerzt im Bericht.

Pruefstatus, Rohdaten, Vergleichsmessungen und Grenzen:
[Phase-102-Bericht](tools/benchmarks/results/render-water-2026-10-22/README.md).
Keine neuen Dependency-, Block-ID-, Generator-, Protokoll- oder Speicherformatversionen.

## 103. Inventar-FPS Und Reaktive UI-Kosten

Die native Inventarregression bei 4K/Sichtweite 20 wurde reproduziert und
im betroffenen Code korrigiert. `tilt-ui-runtime 0.1.0` bleibt versionsgleich
als lokaler Patch mit Lizenzen und originalen Testfixtures eingebunden.
Einfache und zusammengesetzte Bindings traversieren geliehene JSON-Pfade;
Shared-Snapshots werden inkrementell aktualisiert und Aliasse teilen ihre
Allokation. Flow-Nodes cachen ihre Tiefen-Sortierschluessel pro Update.
Die Vorschau-Bildbruecke verwendet einen quellenabhaengigen Index und
Change Detection statt linearer Itemsuche fuer jeden Bildnode pro Frame.
Opacity, spaete Vorschauen, Portraitwechsel und fremde Bilder bleiben erhalten.

Gleiche native Kamera und Qualitaet: offenes Inventar bei 4K/20 von 32,79 auf
68,38 FPS, p99 von 35,16 auf 18,15 ms; bei 2880x1620/12 von 43,22 auf
90,99 FPS. Kurzer Geh-/Rueckweg von 101,43 auf 116,08 FPS. Kein pauschales
Speicher- oder Langzeitversprechen; verbleibende UI-Kosten, einzelne Spitzen
und zwei ungueltige Schneeversuche sind ausdruecklich dokumentiert.

Alle fuenf Cargo-Pruefungen, 1742 Workspace- und 309 Runtime-Tests bestanden.
Native Creative-/Survival-Pruefungen testen echte Eingaben, Serverbestaetigungen
und Layout-Neuaufbau. Headless-Server und zusaetzliche Regen-/Schnee-Nachher-
Proben auf drei Seeds sind bestanden; kein Wetter-Vorherpaar behauptet.
Messungen und abschliessender Build-/Lintstatus:
[Phase-103-Bericht](tools/benchmarks/results/inventory-fps-2026-10-22/README.md).
Keine neuen Dependency-, Netzwerk-, Generator- oder Speicherformatversionen.

## 104. F01: Buffs, Debuffs Und Status-HUD

13 datengetriebene Einzel-JSON-Effekte mit begrenzten Timern, Leveln und
Stapelregeln. Schaden, Heilung, Hunger, Bewegung, Unterwasserantrieb,
Sauerstoffverbrauch, Nahkampf, Resistenzen und verbrauchbare Schildpunkte
wirken serverseitig; Bewegung und Timerauswahl werden konsistent vorhergesagt.
Der Katalog-Fingerprint im Protokoll-56-Handshake verhindert unterschiedliche
Client-/Serverdefinitionen. PLY5 liest PLY4/PLY3/PLY2/PLYR explizit weiter;
Timer/Schild bleiben erhalten, Rauchkontakt wird nach Laden neu ermittelt.

Alle Quallen vergiften bei Kontakt fuer 10 Sekunden. Rauch zeigt Poison nur
waehrend der Exposition ohne zusaetzlichen DOT; unabhaengiges Quallengift
bleibt erhalten. Erfolgreiche Schlafteilnehmer erhalten 5 Sekunden Regeneration.
Schlafen pausiert Gift und Atmung nicht; Netto-Heilung verschluckt keine
tatsaechlichen Schadensereignisse. Berechtigter `/effect`-Befehl einschliesslich
Vorschlaegen, Help-Pagination und Dedicated-Konsole.

Das verschiebbare HUD erhaelt Cards/Restzeiten/Staerke und einen echten
Schildbalken. Gift und Eis liegen direkt auf Herz und HP-Kanal im vorhandenen
UI-Material. Ein geteilter Symbolatlas enthaelt Regeneration als zwei pinke
Herzen; vier wiederverwendete Herzsprites steigen eine Sekunde auf und blenden
aus. Kompakte Ansichten behalten Ueberlauf und vermeiden die Block-Infokarte.

Alle fuenf Cargo-Pruefungen, 1778 Workspace- und 309 Runtime-Tests bestanden
(2 bestehende ignoriert), Headless-Server und acht echte UDP-/GPU-Ansichten
einschliesslich Clear und drei Fenstergroessen. Kein isoliertes FPS-Paar und
kein neuer Langzeittest; weitere natuerliche Ausloeser und Ruestung sind damit
nicht implementiert. Quellen, Nachweise und Grenzen:
[Phase-104-Bericht](tools/benchmarks/results/status-effects-2026-10-22/README.md).

## Kompatibilitaet Und Einordnung

Aktuell: Weltformat 2 (automatische Uebernahme von 1), Generatorversion 5,
Netzwerkprotokoll 56; neue Regionen RGA0 mit Lesern fuer REG9/REG8/REG7/REG6/REG5/REG4/REG3/REG2/REGN.
Spielerdateien verwenden PLY5 mit expliziten Lesern fuer PLY4/PLY3/PLY2/PLYR. Client und Server
gemeinsam aktualisieren. Bestehende Block-/Welt-IDs und Seeds bleiben erhalten.
Flussprofilkorrekturen aendern dennoch das generierte Grundterrain bestehender
`rivers = true`-Welten; gespeicherte Edits bleiben an ihren Koordinaten.
Phasen 25/26 aendern zusaetzlich uneditierte Hoehen, Biome, Hoehlen und Baumverteilung auch
ohne Fluesse. Prop-IDs enthalten die Bodenhoehe; geaenderte Hoehen koennen daher
neue Pflanzenkandidaten neben alten Entfernungsdaten erzeugen. Fuer die neue
Landschaft neue Welten verwenden; bitidentisches altes Terrain wird nicht behauptet.
Normale Cube-Bloecke nutzen die gemeinsame Registry/Meshing-Pipeline,
Spezialmodelle statische Custom-Meshes. Keine Dependency-Upgrades, kein zweites
Prop-/Speichersystem und keine zwingenden Render-/Audio-Abhaengigkeiten im Server.

Kurze native Tests ersetzen keine mehrstuendige Lastabnahme, lokale UUID-Sitzungen
keine verifizierten Online-Accounts und Screenshots keine vollstaendige Art-Abnahme.
Konkrete Restaufgaben und neue Inhalte bleiben in [TODO.md](TODO.md).
