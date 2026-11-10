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

## 105. F02: Ruestungen, Ausruestung Und Ruestungs-HUD

25 dreidimensionale Ruestungsteile in fuenf Sets und vier Accessoires mit
Einzel-JSONs/geformten Rezepten. Neun servervalidierte Plaetze behalten die
alten acht Indizes und ergaenzen Handschuhe. Cursor-/Shift-Transfers verwenden
atomare Inventarrevisionen; volle Inventare oder Spielerstatistikbudgets
verlieren keine Items. Ein Wechsel heilt nicht; Schutz und HP-/Stamina-Maxima
werden aus der getragenen Ausruestung abgeleitet. Tod behaelt die bestehende
Keep-Inventory-Regel, Reconnect und Respawn erhalten die Ausruestung.

Protokoll 57 prueft den Equipment-Katalog, PLY6 liest PLY5 und aeltere
Spielerformate explizit. Die bestehende 256-Byte-Grenze des Control-Enums
bleibt durch ausgelagerte Stats-/Pose-Payloads erhalten. Welt-/Block-IDs,
Generator und Containerformate bleiben unveraendert.

Gebuendelte Ruestungsteile folgen den vorhandenen animierten Gelenken.
120 gemeinsame Meshes und sechs Materialien werden beim Wechsel nicht neu
erzeugt; das Portraet verwendet seine zwei Renderziele erneut. Die silberne
Schutzleiste ist von Shield Power getrennt. Nur der erste Effekt der
Kartenreihe erscheint auf Herz/HP, ohne die Wirkung weiterer Effekte zu aendern.
Implementierung, Pruefungen und Grenzen:
[Phase-105-Bericht](tools/benchmarks/results/equipment-2026-10-22/README.md).

## 106. Capes, Crusher Und Integrierte Ruestungsleiste

Die Ruestungsleiste verwendet den Stamina-Rahmen samt ausgefuelltem Schild.
Capes ergaenzen einen zehnten Platz, fuenf weisse Materialstufen und tiererhaltende
Faerberezepte. Geteilte Voxel-Geometrie, dezente Bewegung und dieselbe Darstellung
in Welt, Portraet, Inventar, Hand und Drops.

F03 implementiert Block Crusher 99 mit Zweiblock-Footprint, 1,23 m Hoehe,
animierter Mechanik und budgetierten Partikeln. Private Ein-/Ausgabe,
revisionierte Transfers, pausierte ungeladene Stationen, 16 einzelne Rezepte
und Too-Many-Items-Integration. Neue farbige Blumen behalten ihre Pigmentzuordnung;
alte Handrezepte bleiben verfuegbar. Gemeinsame Auswahlkontur statt interner
Kollisionsgitter, genaue Hopper-Physik auch ueber Section-Grenzen.
Dedicated-Autosave speichert Maschinenfortschritt auch bei angehaltener Uhr
ohne kuenstliche Terrain-Invalidierung. 1850 Workspace-Tests bestanden,
zwei bestehende Tests ignoriert; native Pruefungen mit 16 Ausruestungs- und
fuenf Crusher-Aufnahmen einschliesslich aller 16 Verarbeitungsrezepte.

Protokoll 58, PLY7 mit PLY6-Migration und RGA1 mit Altformat-Lesern.
Implementierung, Nachweise und Grenzen:
[Phase-106-Bericht](tools/benchmarks/results/capes-crusher-2026-10-22/README.md).

## 107. Handkurbel, Ruecken-Capes Und Inventar-Korrekturen

Handkurbel 100 dockt ohne Bodenpflicht an die Zahnradseite des Crushers an.
Halten der konfigurierten Platzieren-/Benutzen-Taste treibt servervalidiert
die Mechanik und Verarbeitung an; Loslassen, fehlender Sichtstrahl, Menues
oder ungueltige Spielerzustande stoppen sie. Leeres Kurbeln spielt Mechanik,
Zerkleinern zusaetzlich Mahlgeraeusche und fallende Spaene. Geteilte Modelle,
ein budgetierter Stereo-Mix und vorhandene Maschinenbudgets bleiben erhalten.
Ungeladene Kurbelanhaenge verhindern Teilabbau und verwaiste Objekte.

Capes liegen nur hinten, mit verbundenen Gelenken fuer geglaettete Gang-/
Windbewegung. Spielerinventar hoeher und fuer kleine Fenster kompakter.
Persistente, deaktivierbare Slot-Tooltips oeffnen direkt bei Hover, ohne
leere Popups. Picking, Monitor-DPI und UiScale sind konsistent; gleiche
Positionierungsrechnungen verhindern staendige gegenseitige Layoutupdates.
Ruestung maskiert fremde Atlas-Rahmenpixel, Shield Power liegt im bestehenden
Rahmen, Gift tropft im vorhandenen HUD-Shader. Der Portalpfad erneuert geerbte
Opazitaet, sodass Beschreibungen nicht unsichtbar bei Alpha 0 zurueckbleiben.

1873 Workspace- und 315 Runtime-Tests bestanden, zwei bestehende Tests ignoriert.
Alle fuenf Cargo-Pruefungen, Headless-Server und 21 native Aufnahmen bestanden;
Tooltips einschliesslich echter Textpixel bei 80/150 Prozent UI-Skalierung,
Kurbel einschliesslich aller 16 Rezepte und Mute/Unmute. Keine langen Soak-Tests.
Protokoll 59, bestehende Welt-/Spieler-/Regionformate und IDs erhalten.
Implementierung, native Pruefungen und Grenzen:
[Phase-107-Bericht](tools/benchmarks/results/manual-crusher-2026-10-22/README.md).

## Phase 108: Geschlossener Crusher, Werkzeuge Und Farben

Crusher mit geschlossener Rueck-/Seitenverkleidung, Trennwaenden und
Einzugsschaechten; breite Metallbaender, freie Auswuerfe und Kurbelanschluss.
Neue feste Flaechen besitzen Kollisionsboxen. Vierzehn Materialgruppen bleiben,
Geometrie steigt nur um rund 2,2 Prozent. Der neue Stereo-Klang mischt acht
vorhandene Materialaufnahmen mit unregelmaessigen Kontakten, maximal zwoelf
Stimmen, echtem Mute und stillem Idle-Fastpath.

20 Werkzeuge: Schwert, Spitzhacke und Doppelaxt in sechs Materialstufen sowie
die beiden vorhandenen Schaufeln. Gemeinsame facettierte Modelle fuer Inventar,
Hand und Drops; Schwerter 1,5 m, Aexte/Spitzhacken 1,25 m. Ausgepraegte
Kopfruecken und zusammenhaengende Schleiffacetten statt flacher Platten.
Native Schraegansichten haben ueberlappende Schneiden entdeckt; doppelte
Randextrusionen sind entfernt und durch Punktueberdeckungstests abgesichert.

13 Farbstoffe, 65 Capevarianten, alle Bettfarben herstellbar. Neue Pigment-
und Werkzeugrezepte liegen einzeln im bestehenden Katalog und erscheinen in
Too Many Items. Alte Dye-/Tool-Tags und Bettposen bleiben stabil; der
Bett-Schlafpfad akzeptiert auch die neuen Farben. 69 Blockdefinitionen haben
gerade Kanten statt Bevels, ohne Material-/ID-/Weltgeneratorwechsel.
Mixer und Kinetik mit Zahnraedern, Wellen, Fliessbaendern und Wasserrad stehen
als F25/F26 in der TODO, nicht als bereits funktionsfaehiges Techniksystem.

1896 Workspace-Tests bestehen, drei bewusst ignoriert; Audioexport separat
bestanden. Native Werkzeug-/Dye-/Blockansichten, manueller Crusher mit allen
18 Rezepten und Headless-Server geprueft. Keine langen Soak-Laeufe oder
erfundenen FPS-Werte. Quellen, Hoerprobe, Bilder und technische Grenzen:
[Phase-108-Bericht](tools/benchmarks/results/materials-crusher-2026-10-23/README.md).

## Phase 109: Ernte, Anbau Und Werkzeuggriffe

F04 fuehrt Weizen, Mais und Salat in die vorhandene Prop-/Inventararchitektur
ein: acht sichtbare Wachstumsstufen, Samen aus Wildpflanzen, Aussaat mit der
konfigurierten Platzieren-Taste und automatisch vorbereiteter Ackerboden 101.
Feuchtigkeit aus Regen/Bewaesserung, Tageslicht und SeasonSample steuern den
gespeicherten Fortschritt. Reife Kulturen geben Ertrag und Samen, unreife nur
die Saat zurueck. Mais, Salat und Brot sind essbar; fuenf neue Einzelrezepte
und alle sieben Items erscheinen im vorhandenen Too-Many-Items-Browser.

Geteilte, gebuendelte Voxelmodelle zeigen Aehren, Maiskolben, breite Blaetter
und gefaltete Salatkoepfe. Ackerfurchen verwenden die bestehende Cube-Pipeline.
Die Simulation verarbeitet maximal 128 Pflanzen und 128 Wildfutter-Timer je
Sekunde, pausiert ungeladene Felder und erneuert Prop-Geometrie nur bei einer
sichtbaren Stufenaenderung. Gefressene Kulturen beginnen erneut zu wachsen;
Wildfutter verwendet gespeicherte Nachwachstimer. Stabile IDs verhindern
wachsende Tombstone-Listen bei wiederholtem Nachpflanzen. Kapazitaetspruefung
vor Bodenaenderungen und atomar reservierte Erntedrops verhindern Teilaktionen.

Regionen RGA2 speichern Fortschritt, Feuchtigkeit und die bei Aussaat geltende
Wachstumsdauer; RGA1 und aeltere Regionen bleiben lesbar. Netzwerkprotokoll 61
erfordert gemeinsam aktualisierte Clients und Server. PLY7, Weltformat 2,
Generator 5 und bestehende IDs bleiben erhalten. Werkzeuggriffe richten Arbeitskante und
breite Flaeche zur Ego-Kamera aus; Griffpunkte und Vorwaertsschlag bleiben.

Native Survival-Pruefung mit echter Aussaat aller drei Arten, Reifung, Ernte,
Erntedrops und erneut geladenem Spielstand bestanden. Werkzeugansichten und
Vorwaertsschlag separat nativ geprueft. Ausfuehrliche Testnachweise, Bilder,
Grenzen und Reproduktionsbefehle:
[Phase-109-Bericht](tools/benchmarks/results/farming-2026-10-23/README.md).

## Phase 110: Farming-Werkzeuge Und HUD-Korrekturen

Giessekannen, Hacken und Sensen in Holz, Eisen, Gold, Diamant und Rubin
ergaenzen 15 Werkzeuge und 16 einzelne Rezepte einschliesslich Vogelscheuche.
Die konfigurierte Platzieren-/Benutzen-Taste bereitet Ackerboden vor, fuellt
Kannen an echtem Wasser auf, bewaessert Kulturen oder erntet reife Pflanzen
auf 3x3 beziehungsweise 5x5 Feldern. Server pruefen Inventarquelle, Revision,
Reichweite, Sicht und Aktionsabstand. Mehrteilige Ernten reservieren Drops
vorab; Wasser aendert gespeicherte Feuchtigkeit ohne Terrain-Neumeshing.

Neue gemeinsame Modelle, Furchen und 512px-Ackertexturen folgen der Vorlage.
Die Vogelscheuche besitzt Hut, Schal, Stoff/Strohdetails und Laterne, drei
LODs sowie genaue Auswahlgeometrie. Sie bleibt ein passives Objekt ohne
Vogel-KI. Werkzeugkanten zeigen in die Welt; Sensen schneiden seitlich,
Kannen kippen um den Griff. Der Jahreszeitenzeiger ist ein gefuellter
silberner Pfeil mit dunklem Rand.

Die Pflanzen-HUD zeigt replizierte Wachstumsprozente als Balken. Einmal pro
Sekunde werden maximal 256 nahe Datensaetze uebertragen. CreatureStates
werden auch vor dem abgeschlossenen Weltbeitritt entgegengenommen, damit
Lightyear keine unbehandelten Pakete verwerfen muss. Strg + Muelltonne leert
Creative-Inventar, Cursor, Ausruestung und persoenliche Crafting-Reservierungen
serverautoritativ, ohne Drops oder Zugriff auf fremde Weltinventare.

Protokoll 62; PLY7/RGA2, Weltformat 2, Generator 5 und bestehende IDs bleiben.
Die Wasserreserve ist sitzungsgebunden, Pflanzenfeuchtigkeit persistent.
Implementierung, native Ansichten, Pruefungen und Grenzen:
[Phase-110-Bericht](tools/benchmarks/results/farming-tools-2026-10-23/README.md).

## Phase 111: Spinnen, Pflanzenuebergaenge Und Hoehlenlicht

Aussaat auf vorhandenem Ackerboden verankert den Boden korrekt im Journal;
Salatsamen lassen sich damit auch nach vorbereiteten/replizierten Feldern
pflanzen. Kulturueberhaenge blockieren die Bodenauswahl nicht mehr.
Ackerboden zeigt Wachstum oder fehlende Samen. Die acht Pflanzenstufen
verwenden wurzelfeste Groesseninterpolation und eine 2,4-s-Ueberblendung mit
uebereinstimmender Farb-/Tiefendeckung; alte Geometrie wird danach freigegeben.
Die Minimap korrigiert Boden- und Markerhoehen getrennt vom FullMap-Bezug.

Drei JSON-Spinnenarten erweitern die vorhandene Tier-KI mit Boden-/Wand-/
Deckenbewegung, Abseilen, neutraler Gegenwehr beziehungsweise Aggression
und artspezifischem Gift. Zehn verankerte Netz-Konstruktionen verwenden die
bestehende Prop-, Auswahl-, Abbau- und Speicherpipeline. Modelle, Schritte,
Idle und Fauchen folgen den Referenzen; eigene PointLights je Tier entfallen.

Geschlossene Hoehlen erhalten kein direktes Himmelslicht mehr. Tauchen stellt
die Aussen-Ambientbeleuchtung nicht wieder her, solare Unterwasser-Kaustik
bleibt dort aus. Hoehlen-Hintergrundlicht ist 35 Prozent dunkler, Fackeln
bleiben lesbar. Sonnen-/Mondlicht wird stetig uebergeben. Groessere gerichtete
Schwimmspuren verwenden begrenzte gemeinsame Shader-Daten; Wasser-Foley ist
ueberarbeitet. F27/F28/F29 halten die gewuenschte Wassertechnik als offene
Features fest.

Protokoll 63, unveraenderte Welt-/Speicherformate. Details und Pruefnachweise:
[Phase-111-Bericht](tools/benchmarks/results/spiders-farming-light-2026-10-23/README.md).

## Phase 112: Platzierung, Hoehlenschalen Und Tiernavigation

Die Acker-HUD zeigt echte Bodenfeuchtigkeit und abgestuften Wasserbedarf.
Neue Pflanzen-IDs hashen Art und alle Koordinaten getrennt: benachbarte
verschiedene Kulturen koennen sich nicht mehr durch XOR gegenseitig dieselbe
Identitaet geben. Bestehende Pflanzen behalten ihre gespeicherten IDs.

Hohe Hoehlenkammern laden ihre tatsaechlichen Boden-/Dach-Sections gezielt
nach. Neue Welten mit Hoehlenrevision 5 bekommen unregelmaessigere Konturen,
Felsvorspruenge und kleine reale Wasserkanalauslaeufe. Revisionen 0 bis 4
bleiben erhalten. Hoehlen werden gegenueber Phase 111 um 12,5 Prozent heller;
Fackel-Streulicht um 30 Prozent, ohne den direkten Nahbereich zu verstaerken.
Blockige Mineralsegmente und gezielte Tragkettenpruefung verhindern Einsturz
beim Abbau lediglich benachbarter Bloecke.

Gesweepte Tierbewegung prueft Hindernisse und reale Unterstuetzung, auch an
Slabs/Treppen und Wassereintritten. Spinnen haben weitere Entscheidungen,
60 Prozent mehr reale Jagd-/Fluchtgeschwindigkeit und dunklere braune Baender.
F1 + E zeigt auf Anfrage Absicht, Ziel und vorhandene Routenpunkte; F3 + E
bleibt gueltig. Keine globalen Pfaddaten fuer alle Tiere pro Snapshot.

Eine gemeinsame Platzierungsaufloesung liefert die echte Modellsilhouette
fuer ausrichtbare Bauten. Ein wiederverwendetes Vorschau-Mesh bleibt begrenzt.
Leere Haende in einer Sitzung fallen nicht mehr auf die Offline-Baupalette
zurueck. Offene/bewegte Truhendeckel zeigen keinen falschen geschlossenen
Auswahlrahmen.
Truhendeckel oeffnen/schliessen weich anhand autorisierter Betrachter, auch
fuer Beobachter und Doppeltruhen ueber Section-Grenzen. Crusher-/Vogelscheuchen-
Laternen nutzen den vorhandenen budgetierten Lichtpool.

2019 Workspace-Tests und kurze native Farming-/Bau-/Hoehlen-/Spinnen- sowie
Headless-Pruefungen bestanden. Protokoll 64; bestehende Block-/Welt-IDs,
PLY7/RGA2, Weltformat 2 und
Generatorversion 5 bleiben erhalten. Umsetzung, kurze native Pruefungen und
Grenzen: [Phase-112-Bericht](tools/benchmarks/results/building-farming-caves-2026-10-23/README.md).

## Zaehmung Und Interaktionslatenz (Phase 113)

Schafe, Kuehe, Schweine, Pferde und Spinnen verwenden echtes artspezifisches
Futter, gespeichertes Vertrauen und die vorhandenen Besitzer-UUIDs. Besitzer
koennen mit leerer Hand Folgen/Warten umschalten; das Ziel-HUD zeigt den
Zustand. Hoehlenspinnen bauen priorisiert ein gueltiges Netz, suchen bei Hunger
im 20-m-Radius nach echter Nahrung und besitzen 50 Nahrung ohne Durst.
Netz-Neubau und physische Befestigungen bleiben auch nach Entfernen/Fuettern
gueltig.

Der direkte Befehl `/effect [@Name] <Effekt> <Stufe> <Sekunden>` erweitert die
vorhandene autoritative Befehlsverarbeitung. Night Vision ergaenzt die
Effektregistry mit kamera-lokalem Licht. Creative-Pick erzeugt ein Exemplar;
nahe Truhen erhalten bestaetigte raeumliche Oeffnungs-/Schliessgeraeusche.

Bevorzugte Remesh-Aufgaben fuer geladene Edits, kooperativ wartende
Spaltenjobs, gezielte Render-Publikation und einmalige TiltUI-Zielaufloesung
beheben konkrete Warte-/Aufbaupfade. Ein einzelnes vorgewarmtes Inventar wird
wiederverwendet und geschlossen reaktiv geparkt; kalte CSS-Slot-Scopes erfassen
ihre Selektordaten gemeinsam. Grafikqualitaet und Sichtweite werden
nicht reduziert. Kurzer nativer Inventarvergleich: erste Oeffnung 325 -> 65 ms,
laengster Oeffnungsframe 166 -> 25 ms bei 2880x1620/Sichtweite 12; die offene
Dauerlast bleibt vergleichbar. Neue TODOs F30-F32 beschreiben Alchemie, Lagerfeuer und
Temperatur; diese weiteren Systeme sind noch nicht implementiert.

Details, Regressionstests, native Pruefungen und Grenzen:
[Phase-113-Bericht](tools/benchmarks/results/taming-effects-interaction-2026-10-23/README.md).

## Blockoberflaechen, Mob-Balken Und Netzkonstruktionen (Phase 118)

Durchgehende Grasoberseiten und dezenter weltverankerter Materialrelief statt
Rasterrillen. Der klar benannte Graphics-Schalter `Block Surface Shader`
verwendet die bestehende Material-Detailoption ohne neue Konfigurationsdatei,
Materialvermehrung oder Abschalten von Regen/Schnee.

Projizierte Mob-Lebensbalken mit Gruen/Rot, Namen und HP-Wert; Gameplay-Modi
Looking At / All In View / All Time / None. Faire begrenzte Sichtpruefungen
verwenden reale Block-/Modellgeometrie; All Time ignoriert absichtlich Waende.
Stalker-Flackern durch koplanare Detailquader ist geometrisch korrigiert.

Vier groessere verankerte 3D-Netzkonstruktionen nutzen bestehende Prop-IDs,
Section-Meshes und Entfernungsdaten. Spinnen suchen reale obere Gegenanker;
Seilverlaeufe, Durchgangsraum, Cross-Section-Abbau und Reload werden gemeinsam
validiert. Neue Hoehlenrevision 7 verteilt das bestehende breite Netz in
raeumlichen Clustern statt durchgehend hoher Gangdichte. Alte Revisionen bleiben
unveraendert.

Herbstblumen werden nach saisonaler Schwelle innerhalb zehn Echtzeitsekunden
braun und kleiner und verschwinden. Feste Zeit-/Rangdaten und bestehende
Root-Anker erhalten Batching, Picking, Bienen und Wiederkehr im Fruehling.
Keine neue Dependency oder Saveformatversion; Protokoll 69 schuetzt die neue
Netzvariantensemantik vor aelteren Clients.
Pruefungen und native Aufnahmen:
[Phase-118-Bericht](tools/benchmarks/results/render-health-webs-2026-10-24/README.md).

## Pferde Reiten, Schnee Und Autosaves (Phase 117)

F07 verwendet den bestehenden Pferde-/Voxel-Motor fuer autoritatives Reiten und
Client-Vorhersage. Nur eigene gezaehmte erwachsene Pferde lassen sich ohne
angebotenes Futter besteigen. WASD, Sprint und echte individuelle Spruenge
verbrauchen Pferdeausdauer; Shift sucht einen freien geladenen Ausstieg.
Pferd, Sitzpose und Kamera verwenden dieselbe Darstellung. Ein Pferd hat einen
Reiter; Besitzpruefung, Lebenszustand, Headroom und sichere Ausstiege werden
serverseitig geprueft. Keine automatische Pet-Folgefunktion.

Schnee akkumuliert pro globaler Blockposition mit stabilen unterschiedlichen
Raten. Bodenprops bleiben in den ersten beiden Schichten erhalten und werden
ab Schicht drei reversibel verborgen. Herbstblumen blenden gestaffelt aus;
Rendering, Picking und Bienen verwenden dieselbe saisonale Sichtbarkeit.
Die alte globale Schneeaufhellung ist in normalen Welten abgeschaltet.
Bestehende eindeutige Spielerentfernungen bleiben erhalten.

Neue Welten verwenden breitere bestehende Hoehlenconnectoren in Strukturrevision
6; gespeicherte Revisionen 0..5 werden nicht still veraendert. Stalker-Atem nutzt
trockenes bearbeitetes CC0-Atem-Foley statt Rausch-/Sinussynthese.
Die Population validiert natuerliche Hoehlen-Stalker unabhaengig von noch nicht
besuchten Oberflaechenhabitaten. Kinematische Spinnen-Kanten-/Uferwechsel erzeugen
keine ungueltigen Geschwindigkeiten mehr. Speicherpruefungen bleiben aktiv.

Protokoll 68 erfordert gemeinsam aktualisierte Clients und Dedicated Server.
Keine neuen Dependencies, Block-IDs, Welt-IDs oder Speicherformatversionen.
Neue Biome/Mobs aus der Anforderung sind als F34-F39 im Backlog erfasst, nicht
als bereits implementiert ausgewiesen. Native Reit-/Saisonpruefungen, Tests und
verbleibende Grenzen:
[Phase-117-Bericht](tools/benchmarks/results/riding-snow-autosave-2026-10-24/README.md).

## Stalker, Kletterwege Und Streaming (Phase 116)

Spinnen bewegen sich ueber kontaktgepruefte gestufte Waende und konvexe Kanten
an Decken. Oberflaechenrouten bleiben im bestehenden Navigationsbudget. Reale
Fuss-/Koerperauflagen beenden Seilabstiege; entfernte Stuetzen und blockierte
Seile koennen keinen dauerhaften schwebenden Descend-Zustand erhalten.
Pet-Folgen bleibt deaktiviert.

Alle Tiere besitzen ein stabiles Geschlecht aus ihrer unveraenderlichen
Spawn-/Geburts-ID. Neue F06-Paarungen erfordern Maennchen und Weibchen;
Nachwuchs nutzt seine eigene Auswahl. Alte Familien bleiben gueltig.

`teramic::stalker` ist ein seltener Hoehlengegner mit echten Hoehlenrouten,
Ecken-Peeks, Blickreaktion, Verfolgungsangriff und begrenztem Spielerzug.
Fackeln eigener/anderer Spieler, platzierte Lampen, brennende Oefen und
natuerliche Leuchtpilze schrecken ihn bei freier Lichtverbindung ab.
Der Griff verwendet normale autoritative Terrain-Kapselkollision und dieselbe
Client-Prediction, keine Flugfreigabe oder Teleports. Der wirkliche Spielerweg
begrenzt das Entfuehren auf 40 m; alte Griffziele verfallen nach 350 ms.
Gemeinsames Rinden-/Moosmodell, gegliederte Arme/Haende und vier situations-
abhaengige Audioclips ergaenzen das vorhandene Mob-System.

Der nach dem Fliegen grosse Terrain-Arbeitsvorrat berechnet seine teuren
Sortierschluessel einmal je Kandidat statt bei jedem Vergleich. Wetter- und
Kuestensnapshots invalidieren nur relevante lokale Spalten; unveraenderte
oder leere Geometrie benoetigt keinen neuen Snapshot. Grafikqualitaet und
Sichtweite bleiben unveraendert. Niederschlagsarten verwenden Boden-/Meerhoehe
statt Kamerahoehe und beruecksichtigen maritim temperierte Fruehlingskuesten.

Neue Protokollversion 67, bestehende Block-/Welt-/Tier-IDs und Generatorversion
bleiben erhalten. Konkrete Messungen, Regressionen und Grenzen:
4K/Regen/Sichtweite 20 im kurzen nativen Flugvergleich 39,18 -> 71,33 FPS,
p95 43,87 -> 25,16 ms; 1080p/Clear/Sichtweite 12 ohne behaupteten FPS-Gewinn
(123,61 -> 121,73 FPS). Keine Langzeitabnahme aus diesen 120-s-Laeufen.
[Phase-116-Bericht](tools/benchmarks/results/stalker-climbing-streaming-2026-10-24/README.md).

## Nachwuchs, Slabs Und Spinnen (Phase 115)

F06 verwendet bestehende Nahrung, Besitzer, Tier-IDs und Populationsdaten fuer
ausdruecklich ausgeloeste Zucht. Erwachsene gleichartige Tiere desselben
Besitzers bekommen nach je einer zusaetzlichen Fuetterung bei sicherem Platz
ein Jungtier. 60 s Bereitschaft, 300 s Eltern-Abklingzeit, 1200 aktive
Wachstumssekunden; begrenzte Geburt-/Suchbudgets und lokale/Besitzer-Limits.
Wachstum, jugendliche Proportionen/Gang, physische Groesse und Ziel-HUD sind
integriert. Eltern, UUID, unabhaengige Fuetterungsuhr und High-Water-Geburtsnummer
bleiben erhalten. RGA4/IDX7 ergaenzen alte Leserpfade; Protokoll jetzt 66.

Spinnen koennen neue Verankerungen ueber Hoehlenstufen erreichen und halten
Kletterziele begrenzt fest. Unter einem Dach bleiben sie tagsueber nicht
unbegrenzt stehen. Gezaehmte Spinnen nutzen Netzbau/Nahrung autonom ohne
Spielerziel; kein erneutes Folgen oder Leerhand-Umschalten.

Vertikale Slabs sind auch auf Boden-/Deckenmitte erreichbar; Randklicks bleiben
horizontal. Gemischte Slabs werden nach der wirklich anvisierten Haelfte
abgebaut, mit eigener Werkzeugpruefung, eigenem Drop und stationaerer Resthaelfte.
Hover-HUD und Abbaueffekte verwenden dasselbe Zielmaterial. Alte Facing-9-Paare
bleiben als horizontaler historischer Zustand lesbar. Identische Paare behalten
gemeinsamen Abbau mit zwei Items.

Helleres Orange und exakt zwei Pixel breite, ungefuellte Modellkanten ersetzen
die ein Pixel breiten Linien; Mesh/Material bleiben im Cache, nur der
Deckkraft-Uniform pulsiert. Umsetzung und Pruefergebnisse:
[Phase-115-Bericht](tools/benchmarks/results/babies-slabs-spiders-2026-10-23/README.md).

## Platzierungskonturen Und Pet-Befehle (Phase 114)

Automatisches Folgen und Leerhand-Moduswechsel fuer gezaehmte Tiere sind
vorerst deaktiviert. Bestehende Besitzer und Follow/Wait-Speicherwerte bleiben
kompatibel; alte Follow-Werte koennen keine Bewegung reaktivieren. Das HUD
zeigt Vertrauen und Besitzer, keinen Begleitermodus. F33 plant die Befehls-UI.

Platzierungsvorschauen sind nur fuer ausrichtbare Bauten sichtbar. Statt
gruener/roter Flaechen zeichnet ein einzelnes gecachtes LineList-Mesh die
orangefarbenen Modellkanten mit weicher Pulsierung. Coplanare Dreiecksdiagonalen
werden entfernt; normales Stein-/Dirt-Placement bleibt ohne Vorschau, auch mit
F1+B. Keine Material-/Mesh-Neuerzeugung pro Pulsframe, keine Schemaaenderung.

## 124. Dorffelder, Textilien Und Verbundene Fenster

Weisser Teppich und weisse Wolle sind im Katalog, die 13 Farbvarianten entstehen
durch echte Farbstoffrezepte. Teppiche sind 1/8 Block hoch. Neue Blumentoepfe
halten vorhandene Blumen-/Pilzarten mit autoritativer Inventartransaktion,
Einzel-Drops und gespeichertem Inhalt. Bestehende Block-IDs bleiben erhalten.

Gleichartige Fenster verbinden sich zu 1x2, 2x1 und 2x2, mit durchgehendem
Rahmen und passenden Scharnieren. Kollision, Picking, Animation und atomare
Interaktion teilen dieselbe begrenzte Gruppengeometrie. F9-Linien sind 30 %
breiter (3,51 Pixel). Neue Wasserfall-Terrainrevision 1 oeffnet lokal felsige
Gefaelle und erhaelt das vorhandene Wasserfluss-System; alte Welten bleiben
bei Revision 0, wenn das Feld in ihrer Konfiguration fehlt.

Reiner Wachstumsfortschritt und prop-unveraenderte Replikate loesen keine
erneute Pflanzenaufbereitung aus. Generierte Dorfkanaele erhalten gueltige
Wasserquellen. Der dominante stationaere Dorf-Engpass liegt jedoch in den
Fackel-Cubemaps, nicht allein im Weizen. Selektive Sonnenlichtpruefung behaelt
Schatten in Hoehlen, unter Wasser, bei Nacht und bei unklarer Geometrie bei.
Im nativen kurzen Feldvergleich steigt die Framerate von 43,33 auf 55,98 FPS;
notwendige lokale Schatten bleiben teuer. 2416 regulaere Workspace-Tests,
striktes Clippy und Build bestehen; Fenster-/Materialgalerien und der
Headless-Server sind tatsaechlich ausgefuehrt. Kein Langzeit-FPS-Abschluss.
Messwerte, native Aufnahmen und Grenzen:
[Phase 124](tools/benchmarks/results/village-fields-decor-2026-10-25/README.md).

## 125. Baum-Modi, Struktursuche Und Cubemap-Vorbereitung

Shift + gehaltenes Abbauen mit Axt funktioniert an jedem natuerlich
zugeordneten Holz eines Baums, einschliesslich Aesten. Pro Holz gelten
0,1/0,0925/0,08/0,09/0,05/0,05 Sekunden fuer Holz/Stein/Eisen/Gold/Diamond/Ruby.
Gold und Ruby sparen jeden dritten Block nur bei der Zeit, niemals beim
Entfernen oder den Survival-Drops. Creative verwendet die zeitbasierte Aktion
ohne Gratis-Drops. Grafikoptionen: Current, Hell Fall (gebuendelte fallende
Blockkoerper) und Magic (fuenf Sekunden Schrumpfen).

`/locate struct teramic::village`, `teramic::ruin` und `teramic::mine` suchen
vorhandene Strukturplaene. Creative darf auch freie Luft-/Wasserziele nutzen;
die serverseitigen Einmaltokens und erneuten Rechte-/Kollisionspruefungen
bleiben erhalten. Netze werden gegen echte Modellgrenzen und geladene
Stuetznachbarn geprueft, auch gespeicherte Netze. Fehlende Nachbar-Sections
werden nicht als Luft behandelt und erzeugen keine Save-Tombstones.

Neue Welten verwenden Strukturrevision 3 mit Innenraumteppichen und
bepflanzten Toepfen, korrigierten Maschinenfronten/Werkbank-Peers und
ausgerichteten Minen-/Ruinen-Truhen. Ruinen erhalten Stein-Zaeune, -Slabs
und -Treppen. Smooth Stone bleibt ID 12 und bekommt gemalte Mineralflaechen.
Topfinhalte werden nicht mit der Gebaeuderichtung gedreht und verwenden nur
gueltige Pflanzenarten. Bestehende Strukturrevisionen und Spielerbauten
werden nicht regeneriert.

Die gemessene Dorf-Renderlast entstand wesentlich durch wiederholte globale
Indirect-Puffer-Vorbereitung je Punktlicht-Cubemap-Seite. Vollstaendige
GPU-Culling-Cubemaps teilen nun diese globalen Schritte; jede Seite wird
weiterhin jedes Frame mit Bevys vorhandenem Schattenpass gerendert. Nur der
Core3d-View-Executor wird seriell, nicht Weltgenerierung oder die Main-/Render-
Vorbereitung. Ungeeignete Views behalten den normalen Pfad; Fehler spielen
alle Seiten sofort normal nach und schalten die Gruppierung fuer den Lauf ab.

Native Kurzvergleiche auf RX 6950 XT mit Regen und kompletter HUD:
1280x800/Sichtweite 12: 37,91 -> 56,88 FPS;
3840x2160/Sichtweite 20/Ultra-Wetter: 33,82 -> 47,48 FPS.
Keine verringerte Grafikqualitaet oder ausgelassenen Schatten-Draws. Der
kurze Waldflug verbessert sich nur 66,06 -> 70,06 FPS; verbleibende
Frame-Ausreisser sind kein abgeschlossener allgemeiner FPS-Fix.
Neue Weltgenerierungs-/Seen-/Fluss-/Kingdom-Wuensche stehen als F40-F43
in TODO.md, nicht als bereits fertige Spielsysteme.

2447 Workspace-Tests bestehen, striktes Clippy und Build sind sauber. Der
finale native GPU-Readback behaelt bei sieben Lichtquellen 7620 statt 7623
nichtleere Schatten-Draws in der laufenden Vergleichsszene. Alle drei
Baummodi, die Galerie mit 131 Bloecken sowie der Headless-Server bestehen.
Ein weiterer kurzer Regen-/Wasserflug mit Seed 7 wurde ausgefuehrt, ohne
einen Vergleichsgewinn zwischen unterschiedlichen Seeds zu behaupten.

Implementierung, Messbedingungen, Kurztests und Grenzen:
[Phase 125](tools/benchmarks/results/tree-structures-performance-2026-10-25/README.md).

## 128. Cherry Forest, Bewegliche Bewohner Und Lichtauswahl

F34 erweitert die vorhandenen Systeme um zusammenhaengende Sakura-Regionen,
verzweigte pinke/weisse Kronen, passende Blumen und dezent rosa Wasserfarben.
Schmale Baeche in Cherry Forest, Forest und Highlands verwenden dieselben
gecachten Drainagepfade, Wasserzellen, Uferschnitte und Gefaelleregeln wie die
vorhandenen Fluesse. Neue Blatt-IDs 132/133 nutzen neutrale Bluetentexturen
und dieselbe alpha-geclippte Blattgeometrie; neue pinke Blumen sind pflueckbar,
pflanzbar und im Crusher zu pinkem Farbstoff verarbeitbar.

Neue Welten speichern Ecology-Revision 1; alte Manifeste ohne das Feld und
Dedicated-Saves behalten ihre eigene Revision. Kein stilles Retrogen, keine
geaenderten bestehenden Block-IDs. Die gemeinsame sichtbare Biomkennung
meldet Ocean nur an echtem offenem Meereswasser, nicht mehr auf trockenem
Land mit hohem Ozean-Klimagewicht. `/locate biome cherry_forest`, Kartenfarben,
Kreaturen-Habitate und Debuganzeige verwenden diese Regeln.

Die gemeinsame Wegpruefung validiert auch die Koerperausrichtung am Anfang
einer Kante. Bewohner geben unerreichbare Dauerziele auf, koennen sich ohne
unsichere Drehung seitlich aus engen Kontakten befreien und mit fortlaufender
Schwerkraft niedrige Vorspruenge verlassen. Der generierte 17-Bewohner-Dorftest
fand zwei dauerhaft festgefahrene Bewohner; nach Korrektur bewegen sich alle
ueber echte Haeuser, Tueren und Wege ohne Teleports oder groessere Suchbudgets.

Die Mob-Wiedergabe nutzt hoechstens acht Serverzeit-Snapshots und begrenzte
Extrapolation. Verspaetete Paketbuendel starten die Wiedergabe nicht mehrfach
neu; groessere spaete Korrekturen werden geglaettet. Laufanimationen benoetigen
echte dargestellte Bewegung, statt in unveraenderten Positionen weiterzulaufen.

Fackeln erhalten ab vier Metern 50 Prozent mehr Umgebungslicht bei gleichem
Nahkern und Licht-/Schattenbudget. Sichtbare Weglichter werden nicht mehr von
einer Gruppe verdeckter Innenraumfackeln verdraengt. Strukturrevision 5 ergaenzt
bis zu 20 unterstuetzte Fackelpfosten neben den Dorfwegen; Revision 4 hatte
keine Weglichter. Alte gespeicherte Strukturrevisionen bleiben unveraendert.
Dachtreppen und Slabs
schliessen auch bei zwei gegenueberliegenden Profilen am Rohr an, mit freiem
Rauchkanal, passender Form und gemischten Materialien.

2492 Workspace-Tests bestehen, sieben sind bewusst ignoriert. Formatpruefung,
Workspace-Check, striktes Clippy und Workspace-Build sind erfolgreich.
Kirschwald, Bach und Dachgalerie sind im nativen Client aufgenommen und
betrachtet. Messwerte, weitere native Pruefungen, Save-Regeln und konkrete
naechste FPS-Vorschlaege stehen im
[Phase-128-Bericht](tools/benchmarks/results/cherry-ai-light-2026-10-25/README.md).
Keine behauptete allgemeine FPS- oder Langzeitabnahme.

## Kompatibilitaet Und Einordnung

Phase 119 korrigiert reale Bodenkontakte/Blockabstiege, lokale Befreiung aus
Kollisionen und gedrehte Large-Spider-Kletterproben. Natuerliche Cave-Spawns
steigen relativ fuer Stalker um 10 Prozent und Medium/Large Spiders um 25
Prozent, innerhalb derselben Limits. `X` steigt ab, `Shift` sprintet; gemeinsame
Sprungphasen/Sattelpunkte und fortlaufende Rotationsinterpolation glaetten das
Reiten. `F1 + H` zeichnet Mob-Hitboxen und Blickrichtung.

Live-Kartenmarker sind unabhaengig von Detail-Tile-I/O. Gameplay bekommt den
`Edit Bars`-Dialog mit vier Modi sowie drei gespeicherten RGB-Farbwaehlern.
Prop-LOD-Auswahl ist dedupliziert/budgetiert und auf vegetierte Sections
beschraenkt; Edit-Epochen und nachladende Stuetznachbarn bleiben unabhaengig von
anderen Streaming-Ereignissen erhalten. Ungestuetzte Boden-/Haengepflanzen
werden nicht mehr dargestellt beziehungsweise autoritativ korrigiert, ohne
fehlende Sections als Luft zu speichern. Keine Grafikreduktion oder neue
Wasser-/Navigationsengine. Details und Pruefprotokoll:
[Phase 119](tools/benchmarks/results/mob-movement-streaming-2026-10-24/README.md).

Aktuell: Weltformat 2 (automatische Uebernahme von 1), Generatorversion 5,
Netzwerkprotokoll 73; neue Zuchtregionen RGA4, sonst RGA3, mit Lesern fuer RGA3/RGA2/RGA1/RGA0/REG9/REG8/REG7/REG6/REG5/REG4/REG3/REG2/REGN.
Spielerdateien verwenden PLY7 mit expliziten Lesern fuer PLY6/PLY5/PLY4/PLY3/PLY2/PLYR. Client und Server
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


## TODO-Altstand Bis Phase 165

Am 2026-10-31 aus TODO.md hierher verschobene abgeschlossene Eintraege,
Historie, Messungen und Einschraenkungen. Angaben beziehen sich auf ihren
jeweiligen damaligen Stand und sind keine neue Abnahme. Offene Eintraege
stehen ausschliesslich in [TODO.md](TODO.md); bestehende Ergebnisdateien
und ihre unverkuerzten Rohdaten bleiben unveraendert.

Die urspruengliche [Altstand-Zuordnung](tools/benchmarks/results/technical-backlog-2026-10-22/migration.md)
und der [damalige vollstaendige Backlog](tools/benchmarks/results/technical-backlog-2026-10-22/previous-todo.md)
bleiben ebenfalls als historische Quelle erhalten.

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

#### Features

##### Neue Features In Abhaengigkeitsreihenfolge

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


##### Neue Biome Und Natur

F34 ist mit Phase 128 implementiert: zusammenhaengende Sakura-Regionen mit
pinken/weissen Kronen und Blumen, dezent rosa Wasser, Kartenfarben und schmalen
Baechen mit natuerlichem Gefaelle. Auch Forest und Highlands erhalten solche
Baeche ueber die bestehende Hydrologie. Neue Welten verwenden
`ecology_revision=1`; alte Weltvertraege behalten ihre gespeicherte Revision.
Generierte Regionen, Baeche, Save-Kompatibilitaet und Grenzen:
[Kirschwald-/KI-/Lichtbericht](tools/benchmarks/results/cherry-ai-light-2026-10-25/README.md).

- [x] **F40 Abwechslungsreichere Weltgenerierung:** Zusammenhaengende Ebenen, Huegelland, Plateaus, Felskaemme und Senken mit seltenen markanten Erhebungen. Bestehende Biome, Hydrologie und Struktur-Fundamente nutzen dasselbe Terrain; keine zweite Engine. Die spaeteren Revisionen bleiben fuer gespeicherte Welten erhalten; neue Standardwelten verwenden auf Wunsch wieder Main-Revision 1 (siehe unten). Determinismus, Grenzen, Ufer, Tunnel und Dorf-Navigation getestet; native Ansichten und CPU-Messungen fuer Seeds 42042, 71 und 918273 dokumentiert. [Urspruengliche Umsetzung](tools/benchmarks/results/landscape-streaming-2026-10-26/README.md), [Reliefkorrektur und Hydrologie](tools/benchmarks/results/hydrology-relief-performance-2026-10-26/README.md).
- [x] **F41 Groessere Verbundene Seen:** Landschaftsrevision 3 erzeugt unterschiedlich grosse, mehrlappige Seen mit natuerlichen Ufern und gelegentlichen Fluss-Zu-/Abfluessen. Gemeinsame Pegel, echte Wasserzellen, dichte Ufer-/Grenztests und native Seeansichten geprueft. Gespeicherte Revisionen 0/1/2 bleiben unveraendert. [Umsetzung und Grenzen](tools/benchmarks/results/hydrology-relief-performance-2026-10-26/README.md).
- [x] **F42 Laengere Fluesse:** Regionale zusammenhaengende Quelle-See-Meer-Kurse, variable Laengen, gepruefte Meeresanschluesse und natuerliches Gefaelle ueber die bestehende Hydrologie und Fluessigkeitslogik. Begrenzter gemeinsamer Cache statt zweiter Flussengine. Drei Seeds, Grenzen, Fluessigkeitszellen und native Ansichten geprueft; kein unbegrenzter globaler Flussgraph. [Nachweise und Einschraenkungen](tools/benchmarks/results/hydrology-relief-performance-2026-10-26/README.md).

Aktueller Standard auf Wunsch wieder wie `main` (`d2b7956`): neue Welten
verwenden Landschaftsrevision 1, Meeresspiegel 8 und die urspruenglichen
Terrain-/Hydrologiefelder. Die spaeteren Revisionen 2..5 bleiben nur fuer
bestehende Weltvertraege beziehungsweise explizite Konfigurationen erhalten;
keine Umstellung bestehender Saves oder Spieler-Edits. F35 bleibt offen:
Das optionale Terrainprofil Spirit Forest ersetzt nicht dessen noch fehlende Inhalte.
[Konfiguration, Debug-Karten und Grenzen](TERRAIN.md).
Die erweiterten JSON-Bergprofile sind nicht mehr der Standard; gespeicherte
Landschaftsprofile bleiben unveraendert. Der neue
[FIVERR-Grassblock](FIVERR/README.md) wird fuer ID 1 im Terrain verwendet:
aufbereitete Nahgeometrie mit echten Graslippen/Steinchen, Face-Culling,
geschlossene Nachbarkanten und feinere 1254x1254-PBR-Atlanten mit
512x512-Greedy-Fern-LOD. Die Quelldatei bleibt unveraendert.
Tannen besitzen jetzt einen
geschlossenen 3D-Kronenkoerper mit darin verwurzelten Nadelspitzen statt einzelner Sprays.
[Integration und Pruefungen](tools/benchmarks/results/fiverr-grass-fir-2026-10-27/README.md).
[HD-Gras und Main-Rueckbau](tools/benchmarks/results/grass-hd-main-terrain-2026-10-27/README.md).
Die Grasoberseite verwendet nun ein helleres, ruhigeres Gruen mit weniger
Albedovariation; Relief, Steinchen, Seiten und saisonale Effekte bleiben erhalten.

##### Kingdom


##### Mechanische Technik

- [x] **F25 Kinetik-System:** Zahnraeder und Wellen aus Holz, Stein und Eisen, verbindbare Antriebe, Drehrichtung, Drehzahl und begrenzte Belastbarkeit. Bestehenden Handkurbel-/Crusher-Anschluss weiterverwenden; Wasserraeder an das vorhandene fliessende Wasser anbinden, damit sie die Handkurbel als automatischen Antrieb ersetzen. Fliessbaender fuer budgetierten Itemtransport ergaenzen. Antriebsnetze autoritativ, zyklensicher und chunkuebergreifend speichern; kein Entity pro Zahn und keine unbegrenzte Netzsuche pro Frame. Umgesetzt und geprueft mit Phase 149; Bedienung und Grenzen im [Blockhandbuch](assets/terra/blocks/README.md#kinetic-drives-and-conveyors).
- [x] **F26 Mixer:** Kinetischer RGB-Mischer mit sechs Grundfarbstoff-Eingaben, Ziel-Farbwaehler, transparenten Rezeptvorschlaegen und echtem RGB-Ausgang. Gemeinsame Inventar-/Kinetikpfade, TMI-Baurezept und bedarfsgenerierte Mischrezepte per R, animiertes Holz-/Eisenmodell, Maschinenklang und gespeicherter Fortschritt. Betrieb nur bei gueltigem Antrieb; Phase 158 erweitert RGB auf Wolle, Teppiche, Betten und Bandflaechen. Bedienung und Kalibrierungsmodell im [Farbstoffhandbuch](assets/terra/items/DYES_AND_TOOLS.md#calibrated-rgb-dye).

##### Wassertechnik


##### Alchemie Und Temperatur

- [x] **F30 Traenke Und Kessel:** Glasflaschen und gefuellte Trankflaschen, ein Kessel sowie Loeffel aus Holz, Stein und Gold. Datengetriebene Braurezepte auf dem bestehenden Effekt-/Inventarsystem aufbauen; Einzel- und Mischtraenke wie Regeneration + Staerke mit getrennten Staerken/Laufzeiten. Zutaten, Wasser, Erhitzen und Flaschen autoritativ verbrauchen, Kesselinhalt/Fortschritt speichern und passende Rezepte in Too Many Items zeigen. Keine zweite Effekt-Registry; Materialunterschiede der Loeffel explizit festlegen. Umgesetzt in Phase 164; Bedienung und Grenzen siehe unten.
- [x] **F31 Lagerfeuer:** Platzierbares, anzuendbares Feuer mit echtem Brennstoffverbrauch, dezenter Animation, Licht, Rauch und Geraeusch. Kessel aus F30 erhitzen, zum Waermen an F32 anbinden und vorhandene Lebensmittel-/Garrezepte zum Braten nutzen. Abstand, Brandschutz, Loeschen, Mehrspielerzugriff und gespeicherten Brennzustand beruecksichtigen; vorhandene Ofen-/Rauch-/Lichtbudgets verwenden. Umgesetzt in Phase 164; menschliche Hoerabnahme bleibt A02, das vollstaendige Temperaturmodell bleibt F32.

##### Bestehende Erweiterungen Beibehalten

- [x] **F12 Bienen Und Honig:** Autoritative Honigentnahme mit Platzieren/Benutzen am echten Nest, Sicht-/Reichweiten-/Baurechte-/Inventarpruefung und unveraendertem Nest. Echte Nektarbesuche bestaeuben begrenzt geeignete feuchte Nutzpflanzen. Gesunde erwachsene Eltern unterschiedlichen Geschlechts erhalten durch Nektar finanzierten Nachwuchs ueber F06 mit Wachstum, Abklingzeiten, gespeicherten Familien und monotonen IDs. Bestehende Reifung, Nektar, Honig, lokale Moblimits und zehn Nestbewohner bleiben erhalten. [Umsetzung, Tests und Grenzen](tools/benchmarks/results/tree-ai-bees-water-2026-10-24/README.md).
- [x] **F13 Wassertransport (Eimer-Ausbaustufe):** Eisen-Eimer samt Werkbank-/Too-Many-Items-Rezept, leerer/gefuellter Inventar- und Handdarstellung sowie autoritativer Entnahme/Platzierung echter Quellen. Reichweite, Baurechte, gehaltenen Slot und Revisionen pruefen; keine Item-/Wasserduplikation. Quellen und explizit entnommenes Naturwasser ueber die bestehende Journal-/Region-Persistenz sichern. Pumpen sind ausdruecklich die spaetere Ausbaustufe zusammen mit F27; keine zweite Fluessigkeitssimulation. [Code, Tests und Grenzen](tools/benchmarks/results/cherry-water-transport-2026-10-25/README.md).

#### Optimierungen

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

Phase 133 migriert den Workspace auf Bevy 0.20.0, Avian3D 0.8.0,
Lightyear 0.31.0 und die gepinnte TiltUI-Revision aus preview-020.
Rust 1.97.1, WESL-Shader, neue GPU-Uniform-Allocation sowie die bestehenden
lokalen Renderer-/UI-Performance-Patches sind integriert. Native Physik-,
Inventar- und Headless-Servertests bestanden. Kein allgemeiner FPS-Fix:
kurzer Regenflug 97,50 -> 101,83 FPS, Dorf nachts 74,92 -> 59,74 FPS.
Diese Dorf-Regression und A05 bleiben ausdruecklich offen; keine
Qualitaetsreduzierung und keine neue Langzeitabnahme.
[Migration, Messbedingungen und Tests](tools/benchmarks/results/bevy-020-2026-10-26/README.md).

Phase 134 entfernt `vendor/` und lokale Bevy-Quellpatches; Bevy verwendet die
unveraenderten Registry-Quellen. TiltUI-Fixes liegen im echten separaten
`../../tilt-us/tilt-ui`-Repository, vorerst ueber lokale Pfadabhaengigkeiten
statt Git-Pin eingebunden, bis der Push freigegeben ist. Min-/Max-FPS erfassen
nur vollstaendige Frames nach zehn realen Sekunden ab Weltbeitritt, auch keine
ueber die Warmup-Grenze reichenden Haenger. Gebuendelte Mesh-Queue-Bereinigung
beim Entladen sowie SparseSet-Binding-/Shadow-Marker und stabile Schatten-Roots
vermeiden wiederholte Scans und wechselnde Query-Reihenfolgen. Der Hoehlenausblick
behaelt Sonnenlicht und sichtbaren Himmel ohne globale Nebelverdunklung.
2534 Workspace-Tests und 322 Tests der echten TiltUI-Runtime bestanden,
ebenso Formatierung, Check, striktes Clippy und Client-/Server-Build.
Native Hoehlen-/Portalpruefung und F1-Extrema bestanden. Kurze Regenfluege
bei Sichtweite 12/20 bleiben insgesamt nahezu gleich schnell; kein allgemeiner
FPS-Gewinn oder Abschluss der offenen Dorf-/Streaming-Regression behauptet.
A05 bleibt offen.
[Umsetzung und Validierungsstand](tools/benchmarks/results/cave-fps-no-vendor-2026-10-26/README.md).

Phase 135 behebt den Lade-Stall neuer Welten bei nahezu 100 %:
Portalbedingt unsichtbare CPU-Meshes gelten jetzt auch fuer den Ladefortschritt
als fertig. Fehlende Sections und laufende Spalten-Jobs blockieren weiterhin;
Renderer, Ladebildschirm und Smoke-Tests verwenden dieselbe Bereitschaftsabfrage.
Native neue Welten mit Seed 42042, 7 und dem Seed der Benutzerwelt Test
erreichen wieder 100 %, Fade-in und Spielseite; im reproduzierten Fall ca. 15
statt 74 s. 2540 Tests, Formatierung, Check, striktes Clippy und Workspace-Build
bestanden. Bestehende Welten, Portaloptimierung und Welt-Audio-Reihenfolge
bleiben erhalten.
[Ursache, Regressionstests und Nachweise](tools/benchmarks/results/world-loading-2026-10-26/README.md).

Phase 136 schliesst den nachgewiesenen Sonnenlicht-Durchtritt durch vorhandene
Hoehlendaecher: gemeinsame Material-Tiefenpaesse bleiben auch ohne Near-Proxy
und bei Middle/Far beidseitig, waehrend der Farbpass sein Culling behaelt.
Trockenes Terrain nutzt Stock-PBR mit bindless-faehiger leerer Depth-Extension;
Wetter-, Fade-, Draw-Readiness- und Freigabepfade sind integriert. Native
Near/Middle/Far-Referenz/Gegenprobe sowie heller Hoehlenausgang, Fackeln und
Unterwasser bestehen. Der begrenzte Lichtpool verwendet Entities wieder;
identische einblendende Mesh-Ergebnisse verbrauchen vor dem Verwerfen kein
Upload-Budget und erzeugen keine neuen Assets mehr. 2553 Workspace-Tests,
Formatierung, Check, striktes Clippy und Client-/Server-Build bestanden.
Kurze native Vergleiche ohne Profiler: integrierter Regenflug, Sichtweite 20,
85.51 -> 86.09 FPS; Wasser-/Regenflug mit anderem Seed 72.80 -> 77.30 FPS.
Die schlechtesten Frames bleiben etwa 33 bzw. 40 ms. Ein einmaliger 118-ms-
Ausreisser wiederholte sich auch im unveraenderten Ausgangsstand nicht;
kein behaupteter Abschluss des seltenen 9-FPS-Problems oder von A05.
Keine neuen Langzeittests oder reduzierten Qualitaetsoptionen.
[Code, verworfene Ansaetze und native Messungen](tools/benchmarks/results/streaming-cave-shadows-2026-10-26/README.md).

Phase 137 staffelt grosse Terrain-/Prop-Ergebnisse nach Materialbatches ueber
Frames, publiziert nur vollstaendige Ersetzungen und gibt abgebrochene
Staging-Assets frei. Die Prop-Auswahl bereinigt ihre Queue einmal statt bis
zu 96-mal. LOD-Wechsel und Crop-Growth sind abgesichert. Keine reduzierte
Sichtweite oder Detaildichte. Kurze native Paare belegen jedoch keinen
grossen allgemeinen FPS-Fix: Integrated/1080p 82.05 -> 82.82 FPS,
Wasser/800p 73.97 -> 73.30 FPS; p99 sinkt leicht, der schlechteste Wasserframe
wird schlechter. CPU-Renderarbeit und einzelne grosse Materialbatches bleiben
relevante Messpunkte. A05 bleibt offen. F40 kostet zusaetzliche CPU-Zeit und
wird getrennt von diesen identischen alten Landschaftsszenen bewertet.
[Messwerte, Profiling und F40](tools/benchmarks/results/landscape-streaming-2026-10-26/README.md).

Phase 138 reduziert wiederholte Asset-Bounds-Pruefungen fuer unveraenderliche
Terrain-/Prop-/Wasser-Meshes, erhaelt Displacement-Padding und Frustum-Culling.
Section-/Column-Tabellen nutzen Bevys Hash-Collections; Interest-Mengen werden
direkt statt doppelt aufgebaut. Render-/Voxel-Pakete sind auch bei normalem
`cargo run` mit Stufe 3 optimiert. Keine reduzierte Grafikqualitaet.
Im abschliessenden kurzen nativen Vergleich: Integrated/1080p/Sichtweite 20/
Regen 79,63 -> 83,44 AVG FPS; zweiter Seed/Wasser/Regen 76,84 -> 81,94 FPS.
p95/p99 verbessern sich, die schlechtesten Frames aber nicht: weiterhin
ca. 41 bzw. 40 ms. Die geforderte Mindest-FPS 32 ist **nicht erreicht**.
Ein wiederholter Ausgangslauf widerlegt einen behaupteten generellen Gewinn
aus dem einmaligen vorherigen 94-ms-Ausreisser. 2584 Workspace-Tests plus
ein ergaenzender Interest-Test, Formatierung, All-Targets-Check, striktes
Clippy und Workspace-Build bestanden. A05 bleibt offen.
[Code, saemtliche Vergleiche und Grenzen](tools/benchmarks/results/flight-frame-budget-2026-10-26/README.md).

Phase 139 setzt F41/F42 in Landschaftsrevision 3 um und erhaelt mehr lokales
Relief in Ebenen, Plateaus und Senken. Alte Landschaftsrevisionen bleiben
unveraendert. Streaming-Interest, Chunk-Animationsabfragen und Wasser-Uploads
vermeiden wiederholte Vollscans; der Channel-Mesher sortiert verdeckte/falsche
Hoehen vor den teuren Eckpunktsamples aus. Framegenaue native Diagnostik
belegt zusaetzlich einen unbudgetierten ca. 25-MB-Vollhimmel-Upload waehrend
des Flugs. Wiederverwendbare Wolkenbanken mit begrenzten Rand-Uploads ersetzen
diesen Pfad ohne weniger Wolkendetails. Feste kleine Cloudgruppen besitzen
shader-sichere Frustum-Bounds. Grosse stabile Prop-Batches werden im Worker
an Prop-Grenzen geteilt; Wachstumsuhren und atomarer Rootwechsel bleiben
erhalten. Dynamische Wasserdaten verwenden einen gemeinsamen GPU-Puffer statt
wiederholter Material-Asset-Aenderungen fuer alle Wassertiles. Formatierung,
All-Targets-Check, striktes Clippy und Client-/Server-Build bestanden;
2636 Workspace-Tests und nach dem letzten Render-Schritt 691 gezielte
Render-Tests bestanden. Native Regen-, Wasser- und Schnee-Laeufe verbessern
AVG/p99 gegen zeitnahe Kontrollen. Ein Wiederholungslauf enthaelt jedoch
weiter einen einzelnen 73-ms-Ausreisser; Schnee unterschreitet ebenfalls
weiter 32 FPS. Die feste Untergrenze und A05 sind nicht abgeschlossen.
[Umsetzung, alle Messlaeufe und native Aufnahmen](tools/benchmarks/results/hydrology-relief-performance-2026-10-26/README.md).

Phase 146 korrigiert die Startup-/Play-Umschaltung bei portalverdeckten
CPU-Meshes, begrenzt einzelne Worker-/GPU-Uebergaben samt Attributbytes,
cached authored Nachbarformen nur innerhalb einer Komposition und verwendet
validierte Schattenhuellen fuer das authored Gras. Bereits flache authored
Near-Faces verwenden jetzt ebenfalls Greedy-Meshing, ohne die Detaildistanz
oder Nahgeometrie zu reduzieren. Native Sichtweite-20-Vergleiche auf RX 6950 XT:
klarer Flug AVG 44.64 -> 46.72, Minimum 16.59 -> 24.49 FPS;
Regenflug mit zweitem Seed AVG 54.94 -> 55.37 FPS, kaum allgemeiner Gewinn;
integrierter Stand AVG 78.18 -> 80.02, Minimum 26.41 -> 34.82 FPS.
2749 Workspace-Tests bestanden, 7 ignoriert; Formatierung, All-Targets-Check,
striktes Clippy, Client-/Server-Build sowie drei native Grasaufnahmen bestanden.
Keine Bevy-/TiltUI-Patches oder Qualitaetsreduktion. Die feste Untergrenze
32 FPS wird im Flug weiter unterschritten; A05 bleibt offen.
[Messungen, verworfene Versuche und Bilder](tools/benchmarks/results/grass-palette-streaming-2026-10-27/README.md).

Phase 147 bearbeitet die gemessenen Render-Prioritaeten 1 bis 3:
Authored Terrain-Normalen/-Tangenten werden nativ komprimiert, flache
Fallback-Schatten bleiben gebuendelt ohne redundante Huellen und
Instanz-Ausgabepuffer wachsen mit begrenzter Kapazitaetsreserve.
Der eigene Bindingscache kostete im Flug mehr als er einsparte; normales
Spielen nutzt wieder den unveraenderten Bevy-Helper, der Cache bleibt eine
explizite Smoke-Testoption. Keine Vendor-Patches oder Qualitaetsreduktion.
Native Sichtweite-20-Vergleiche: klarer Flug AVG 47.25 -> 49.75 FPS,
Minimum 22.23 -> 23.43; zweiter Seed/Wasser/Regen AVG 55.42 -> 59.75,
Minimum 26.48 -> 30.05; integrierter Stand AVG 81.35 -> 84.90 FPS.
Die feste Untergrenze 32 FPS bleibt im Flug offen. Formatierung, Check,
striktes Clippy und Client-/Server-Build bestanden; finale Workspace-Suite
mit zwei Testthreads: 2766 bestanden, 7 bestehende ignoriert. Ein vorheriger
Standardparallelitaetslauf traf das bestehende Streaming-Test-Zeitlimit;
Assertions und Weltcode wurden dafuer nicht veraendert. Native Grasgalerie
inklusive drei GPU-Pixelpruefungen und Sichtkontrolle bestanden.
[Alle Messlaeufe, Einschraenkungen und Bilder](tools/benchmarks/results/render-preparation-2026-10-28/README.md).

Phase 148 entfernt ungenutzte Prop-Exposure-Snapshots und deren redundante
Mesh-Invalidierung; echte Support-, Surface-, Wachstums- und Schneeaenderungen
bleiben aktiv. Lampenfreie Sections umgehen die Einzel-Emitter-Suche.
Der vorhandene kompakte Prop-Attributpfad gilt jetzt auch fuer Near/Middle:
gleiche Geometrie und Distanzen, weniger Uploadbytes, quantisierte Attribute.
Ein erster klarer Vergleich zeigte weniger langsame Frames, aber die
Halbierung hielt im Kontrolllauf nicht stabil. Direkter finaler An/Aus-
Vergleich derselben Binary: AVG 50.19 -> 52.53 FPS, p95 32.58 -> 31.26 ms,
langsame Frames unter 32 FPS 117 -> 85; die schlechtesten Einzelbilder
verbessern sich nicht. Wasser/Regen bleibt mit Packing bei rund 59.8 AVG.
Die feste 32-FPS-Untergrenze und der kritische Min-FPS-Fix bleiben OFFEN.
Neue Flussrevision 2 erweitert die bestehende Hydrologie um unabhaengige
Land-Abzweige mit eigenen Muendungen; alte gespeicherte Revisionen bleiben
unveraendert. Ein reiner Unterwasserpfad gilt ausdruecklich nicht als Split.
Dorfbewohner tragen echte Spieler-Items mit gemeinsamer Griffpose am
animierten rechten Arm statt eingebauten Werkzeugen; beim Schlafen verborgen.
Native Fluss-/Dorfbewohner-Galerien samt Wasser-/Hand-Assertions und Bildern
geprueft; keine neue Berufs-KI oder Langzeit-Performance-Abnahme.
Formatierung, Check, striktes Clippy und Client-/Server-Build bestanden;
abschliessende gesamte Workspace-Suite: 2780 bestanden, 7 bestehende ignoriert.
[Messungen, Codepfade und Grenzen](tools/benchmarks/results/streaming-river-villager-2026-10-28/README.md).

Phase 149 lokalisiert die Beleuchtung grosser Greedy-Grasflaechen, statt einzelne
AO-Kanten als Farbverlauf ueber die ganze Flaeche zu ziehen. Dirt und seine
Bauformen teilen exakt das authored Gras-Erdmaterial samt Normal-/ORM-Texturen.
F25 ergaenzt acht Maschinenbloecke, Rezepte/TMI, sechsachsige Wellen/Kurbeln,
vorzeichenbehaftete Antriebsnetze, Lastgrenzen, reale Wasserstroemung und
budgetierten Transport bestehender Itemdrops. Speicherung nutzt den vorhandenen
Section-Journal; Netzsuche ist auf geladene indexierte Knoten begrenzt.
Streaming behaelt fertige Geometrie bei vorlaeufig ausstehender Exposure-Pruefung;
echte Aenderungen verwerfen sie weiterhin. Statische Terrain-Attributpaare
werden im Worker gepackt. Im gleichen nativen kurzen Klar-Flug sinkt der
CPU-Terrain-Upload von 1.716 auf 0.648 / 0.682 ms pro Frame. Die Wiederholung
zeigt aber keinen stabilen AVG-/Tail-FPS-Gewinn; 32 FPS werden weiter
unterschritten. Kein abgeschlossener allgemeiner FPS-Fix, A05 bleibt offen.
2814 Workspace-Tests bestanden, 7 bestehende ignoriert; Formatierung,
All-Targets-Check, striktes Clippy und Client-/Server-Build bestanden.
Native Gras-/Kinetik-Galerien und integrierte Crusher-Bedienung mit allen
19 Rezepten geprueft; keine neuen Langzeittests oder Qualitaetsreduktion.
[Codepfade, alle Messlaeufe, Bilder und Grenzen](tools/benchmarks/results/kinetics-grass-streaming-2026-10-28/README.md).

Phase 150 erweitert F25 um grosse Holz-/Stein-/Eisenzahnraeder (149-151),
Create-artige axiale/diagonale Verbindungen, 2:1-Uebersetzungen und
rechtwinklige grosse Zahnraeder. Drehzahl behaelt Nachkommastellen;
Leistungs-/Drehmomentgrenzen verhindern Energiegewinn und Ueberlast.
Platzierung, Ghost, Kollision und Wasser-Clipping beachten reale Ueberstaende
und den koaxialen Kurbelanschluss. Inventar-Icons zeigen die vollstaendigen
Modelle; laufende Getriebe verwenden einen begrenzten RPM-abhaengigen Audiomix.
Das runde Wasserrad misst 1 x 2.5 x 2.5 m, braucht reale Randkontakte und
unterscheidet Stroemung, Wasserfall und Meereswellen samt Sturmstaerke;
trockene Raeder und ruhige Seen treiben nichts an. Flussphysik +20%,
Seeoberflaechen mit dezenter Welle. Karten verwenden das helle authored Gras;
neue Cachedateien speichern rekonstruierbare Samples, alte reine RGBA-Zellen
bleiben bis zum erneuten Besuch erhalten. Gruene Props passen zur Graspalette.
Sand erhaelt feine goldene Koernung und sechs kleine echte Kieseldetails mit
148-Vertex-Flaechenbudget; plane Raender halten alle Nahtpruefungen ein.
2859 Workspace-Tests bestanden, 7 bestehende ignoriert; Formatierung,
All-Targets-Check und striktes Clippy bestanden. Nach den letzten Cache-/
Testkorrekturen weitere 243 Client-/Storage-Tests bestanden, 1 bestehender
ignoriert; abschliessendes Clippy und Client-/Server-Build bestanden.
Native Material-, Kinetik- und komplette 52-Stufen-Kartenpruefung auf der
RX 6950 XT bestanden, einschliesslich sechs verschiedener Zahnrad-Icons und
echtem Flug mit Nachladen. Screenshots und Logs im Bericht. Kein allgemeiner
FPS-Fix oder neue Langzeitabnahme.
[Codepfade, Pruefungen, Bilder und Grenzen](tools/benchmarks/results/kinetics-water-surfaces-2026-10-28/README.md).

Phase 151 korrigiert mechanische Port-Ziele an Wellen/Geraeten gemeinsam fuer
Vorschau und Authority. Gehaltene mechanische Bauteile unterdruecken dabei die
Geraete-UI/Kurbelbenutzung statt die Platzierung im PreUpdate zu blockieren.
Foerderbaender sind 0.625 m hoch, mit Achsen auf 0.5 m; kleine Zahnraeder
passen mit kreisfoermigem Aussenradius 0.5 m ohne Bodenpenetration daneben.
Sand ist komplett steinfrei, feiner gekoernt und hat dezente Windrippen;
Geometriebudget maximal 28 statt 148 Vertices je Detailflaeche.
Neues Werkzeug `teramic::wrench`: Rechtsklick/Place dreht ausrichtbare Bloecke,
Shift dreht rueckwaerts. Werkbank-Rezept aus drei Eisenbarren und einem Stock,
Tools/TMI-Eintrag, eigene offene 3D-Backen und regulaere Werkzeug-Haltepose.
Misch-Slabs, Moebelpaare, Kurbeln, Farben, Tueren/Fenster und private Inventare
bleiben erhalten; begrenzter atomarer Preflight ohne Weltkopie oder Drops.
Client/Server verwenden gemeinsam Protokoll 80; Block-/Generator-IDs bleiben.
2888 Workspace-Tests bestanden, 7 bestehende ignoriert; Formatierung,
All-Targets-Check, striktes Clippy und Client-/Server-Build bestanden.
Kurze native Wrench-, Kinetik- und Materialpruefungen auf der RX 6950 XT
bestanden; Bilder und Logs archiviert. Keine neuen Langzeit-FPS-Tests.
[Pruefungen, Bilder und Grenzen](tools/benchmarks/results/wrench-mounting-sand-2026-10-28/README.md).

Phase 152 erweitert die Kinetik um Handkraft 128, Verbrauch 1 je kleinem
Zahnrad/Welle, 2 je grossem Zahnrad und 20 je Crusher. Alte versteckte
Material-Drehmomentgrenzen entfallen. Ein gespannter Foerderband-Span kostet
einmal 2, nicht pro Zwischenzelle. Zwei parallele Wellen werden mit zwei
Klicks und einem Connector verbunden; bis 32 Zwischenzellen, horizontal,
45 Grad oder vertikale Kraftuebertragung. Echte Itemstapel folgen derselben
Geometrie; Eigentum, atomarer Aufbau/Abbau, erhaltene Endwellen und
Chunk-Grenzen verwenden das vorhandene Journal (Protokoll 81).
Eiche/Birke/Fichte/Kirsche fuer Holzraeder, grosse Holzraeder, Wellen und
Baender in Creative, Rezepten und TMI. Neue Item-Hopper (152) entnehmen nur
fertige Ofen-/Crusher-Ausgaben; Importer (153) lagern Bandstapel atomar in
Truhen/Doppeltruhen ein. Volle, blockierte oder ungeladene Ziele verlieren
keine Items. Wasserrad-Spielerkollision nur an der Welle; volle Auswahl-
und Wasserkontaktform bleibt. Zahnrad-Raender duerfen im Boden versinken,
ohne Wandkollision oder eingegrabene Achsen freizugeben. Sand mit feiner
mehrskaliger Koernung und granularer PBR-Wirkung, ohne Steine oder neue
Detail-Entities.
2932 Workspace-Tests bestanden, 7 bestehende ignoriert; Formatierung,
All-Targets-Check, striktes Clippy und Client-/Server-Build bestanden.
Kurze native Kinetik-/Materialpruefungen auf der RX 6950 XT bestanden,
einschliesslich tatsaechlich gerenderter Band-Items. Drop-Interpolation erkennt
Bewegung auch bei pausierter Item-Lebensdauer; Bilder und Logs archiviert.
Keine neuen Langzeit-FPS-Tests oder unbelegten Performance-Versprechen.
[Bedienung, Codepfade und Validierung](tools/benchmarks/results/owned-belts-logistics-2026-10-28/README.md).

Phase 153 macht Hopper/Tunnel und Funnel regulaer abbaubar, ohne angehaengte
Inventare als zweite Moebelhaelfte zu behandeln. Graue, mit Farbstoff
faerbbare Baender tragen Items mittig und aufrecht; kein versehentliches
Annaeherungs-Aufheben mehr, stattdessen gezielter Rechtsklick. Tatsaechtliche
Transportgeschwindigkeit und Snapshot-Interpolation beseitigen das
Bewegen/Pausieren-Muster; volle Geschwindigkeit bis 256 RPM mit begrenzten
Kollisionsschritten. Holz bestimmt nur das bodengestuetzte Gestell,
schwebende/schraege Baender bleiben ohne Rahmen. Geteilte Gestell-Meshes,
passende Picking-/Kollisionsformen und keine unsichtbaren Legacy-Rahmen.
Neue Holz-/Metallmodelle fuer Hopper als Tunnel und Importer als Funnel;
Richtungspfeile und Wrench-Umschaltung fuer Ein-/Ausgabe an Truhen, Oefen
und Crushern. Maschinen nehmen nur gueltige Input-/Fuel-Items an.
Doppeltruhen richten sich an der vorhandenen Haelfte aus; ein echter
Clearance-Selbstkollision-Bug bei angebauten Anschluessen ist behoben.
Inhalte/Holzarten bleiben beim Verbinden/Trennen erhalten. Kinetik,
Anschluesse und Crusher bekommen keine natuerlichen oder Shader-Schneekappen.
Block-/Welt-IDs bleiben erhalten; gemeinsames Protokoll 82.
2963 Workspace-Tests bestanden, 7 bestehende ignoriert; Formatierung,
All-Targets-Check, striktes Clippy und Client-/Server-Build bestanden.
Gezielter nativer Kinetiktest auf der RX 6950 XT bestanden, Bilder archiviert.
Die breite Materialgalerie publizierte alle 153 GPU-geprueften Blockvorschauen,
erreichte aber in zwei Laeufen ihr unveraendertes 120-Sekunden-Limit und gilt
nicht als vollstaendig bestanden. Keine neue Langzeit- oder FPS-Zusage.
[Bedienung, Pruefungen, Bilder und Grenzen](tools/benchmarks/results/conveyor-tunnels-2026-10-28/README.md).

Phase 154 korrigiert Tunnel-/Funnel-Platzierung an der tatsaechlich angeklickten
Zelle, erlaubt alleinstehende Anschluesse und begrenzte Standfuesse im Boden.
Wellen-/Zahnrad-Ausrichtung priorisiert den angeklickten Achsanschluss statt
entfernte Nachbarn. Gerade Baender funktionieren in allen achssenkrechten
Ebenen und erhalten Zwischenwellen; vertikale/stehende Varianten uebertragen
Kraft, aber keine Items. Neue horizontale/vertikale Gearboxen (154/155) haben
vier echte, passend animierte Anschluesse, Verbrauch 2 und 1:1-Uebersetzung.
Wasserrad-Grundkraft 128 treibt auch einen uebersetzten Crusher; Stillwasser
und ueberlastete Netze bleiben kraftlos. Detailliertere Holz-/Metallmodelle
und drei eigene Maschinentexturen, gemeinsamer TMI-Tab Technic.
Fenster-Gruppen kollidieren beim Erweitern nicht mehr mit sich selbst;
Kirschholz-Tueren/-Fenster behalten ihre Oeffnungs-/Render-Metadaten.
Entfernte Bloecke ueber Seen maskieren vorhandene Quellen nicht mehr als
trockene Luft. Echte Wasserentnahmen und vorhandene Speicherstaende bleiben.
Neue Flussrevision 3 nutzt vorhandene Nebenlaeufe auch im normalen Terrain;
drei deterministische Testgebiete zeigen zusammen 21.43% mehr wirklich nasse
Wasserlaufstrecke. Alte Weltrevisionen erhalten keinen stillen Retrogen.
Wind-Frames und HDR-Farben werden im vorhandenen Meshing-Worker kompakter
vorbereitet. Nach dem finalen Prop-Split entfallen wirkungslose Growth-Daten
nur bei nachweislich statischen Teilmeshes; keine weitere Entity oder
Geometrie-/Dichte-/Sichtweitenreduktion. Ein getesteter LOD-Cache wurde wegen
fehlendem messbarem Nutzen wieder entfernt. Kurze native Flugvergleiche mit
zwei Seeds/Klarwetter/Regen liefern keinen allgemeinen AVG-/Min-FPS-Fix;
32 FPS werden weiterhin unterschritten. A05 bleibt ausdruecklich offen.
Native Kinetik-/Fenster-Galerien sowie echte GPU-Pass-/Drawdaten liegen vor.
Block-/Welt-IDs und Save-Feldlayouts bleiben; gemeinsames Protokoll 83.
3010 Workspace-Tests bestanden, 7 bestehende ignoriert; Formatierung,
All-Targets-Check, striktes Clippy und Client-/Server-Build bestanden.
Zusaetzliches natives CPU-Sampling (18.010 s, 12264 Samples, keine verlorenen)
grenzt Mesh-Assetpruefungen, Sichtbarkeit und Speicherverwaltung ein, ohne
daraus erfundene Framezeit-Anteile oder einen fertigen FPS-Fix abzuleiten.
[Messlaeufe, Bilder, Codepfade und Grenzen](tools/benchmarks/results/kinetics-placement-water-fps-2026-10-29/README.md).

Phase 155 ueberarbeitet kleine/grosse Zahnraeder und Wellen als massive,
segmentierte Holz-/Stein-/Metallkoerper mit Naben, Beschlaegen und Schrauben.
Die mechanisch passenden 8/16 Zaehne, vorhandenen Kollisionshuellen und
Block-IDs bleiben erhalten. Native Modell-/Band-Galerie mit sechs Ansichten
und elf Icons bestanden; Farbstoff betrifft nur das Band, nicht Holzrahmen
oder Metall. Alle 14 Bandfarben und vier Holzvarianten sind gezielt geprueft.
Innere Bandsegmente melden keine falschen Wellenanschluesse mehr; damit
erzeugt ein kurzer gegenlaeufig uebersetzter Crusher-Abzweig keinen erfundenen
Richtungskonflikt. Echte widerspruechliche Antriebe bleiben ueberlastet.

Ein gemessener Terrain-Teilmesh-Reuse-Versuch spart Assetwechsel, verbessert
die FPS aber auch mit begrenztem Hashaufwand nicht ueberzeugend. Deshalb sind
Versuch, Zusatzmetadaten und direkte Hash-Abhaengigkeit wieder entfernt.
Die Sichtbarkeitspruefung behandelt vollstaendig opake authored Cubes korrekt
als geschlossen und unterscheidet echte Luftverbindungen innerhalb Sections.
Beim Fokuswechsel bleiben nachweislich bereits erreichte Sichtpfade stabil,
statt verdeckte Meshes unnoetig erneut anzufordern. Unbekannter Raum, partielle
Bloecke, Nahfeld und der Wand-/Schattenring bleiben konservativ sichtbar.
Allokator-, Worker-, Vulkan-Speicherblock- und Render-Scheduler-Gegenproben
wurden gemessen; mangels ueberzeugenden Nutzens werden daraus keine
pauschalen Produktionsaenderungen oder Abschaltungen.
Der neue, durch Gegenproben belegte Hauptbefund liegt bei den haeufigen
Wetter-/Terrainmaterialien: Alpha-Animationen verwendeten Non-Bindless-
Bindungen mit wiederholtem Ressourcenaufbau. Der eigene Materialpfad nutzt
jetzt Bevy-Bindless-POD-Daten mit unveraendertem Shader-/Materialvertrag und
gemeinsamem Hardware-Fallback beider Materialteile; kein Engine-/Vendorpatch.
Exakte opake LOD-Schatten werden pro Section gebuendelt, ohne Geometrie zu
vereinfachen. Einblendungen geben ihre eigene Batching-Sperre nach dem ersten
bestaetigten Draw frei; fremde Sperren und noch nicht gezeichnete Teilmeshes bleiben.
Kurze native 32-s-Flugpaare auf RX 6950 XT, Sichtweite 20, 1280x800 und High:
Seed 42042/klar AVG 53.15 -> 73.47 FPS, p99 31.77 -> 24.59 ms;
Seed 7/Regen AVG 61.49 -> 86.28 FPS, p99 27.73 -> 21.49 ms.
Animation, Dichte, Wasser und Schatten bleiben an. Die isolierte alte
Wet-Bindung reduziert den neuen klaren Lauf wieder auf 56.97 FPS; fruehere
Batching-Freigabe allein ist dagegen kein belegter AVG-Gewinn.
Kinetikgalerie und vier native Hoehlen-/Fackel-/Unterwasseransichten bestehen
auch mit dem neuen Binding-Pfad. 3051 Workspace-Tests bestanden, sieben bewusst
ausgelassen. Format, All-Targets-Check, striktes Clippy und Client-/Server-Build
bestanden. Zwei klare Frames bleiben unter 32 FPS;
allgemeine Min-FPS-Garantie und A05 sind deshalb weiterhin offen. Keine neuen
10-/15-/30-Minuten-Laeufe und keine unbelegte In-Game-/Mehrspieler-Abnahme.
[Native Gegenproben, Referenzbilder und technische Grenzen](tools/benchmarks/results/kinetic-reference-cpu-2026-10-29/README.md).

Phase 156 verbindet Tunnel mit Truhen und Maschinen auf gleicher Bandhoehe
und einer Ebene darueber/darunter. Freie End-Funnels geben reale, bestehende
Stacks kollisionsgeprueft aus. Funnel-Stuetzen entfallen ueber Luft auch in
der Kollisionsform. Neue Crusher belegen eine Zelle; alte gespeicherte Paare
bleiben lesbar. Der echte Modelleingang befuellt den privaten Input, faengt
schnelle Fallitems ab und behaelt unpassende/volle Stacks sichtbar auf dem
Modell. Einsammeln erfolgt explizit und hat vor dem Oeffnen der Maschine Vorrang.
Inventarzugriff verlangt fuer neue Crusher keinen erfundenen zweiten Block.
Die Sichtpruefung testet neben der Mitte vier echte Trichterrandpunkte, damit
die eigene montierte Kurbel eine sichtbare Maschine nicht unbenutzbar macht.
Erster tatsaechlicher Ray-Hit, Reichweite, Wandschutz und Ladepruefungen bleiben.
Der native Ein-Block-Crusher-Durchlauf besteht mit allen 19 Rezepten, TMI,
Kurbeln, Loslass-Pause, korrekten Outputs und fuenf Bildern; Audiozustaende sind
technisch geprueft, nicht als menschliche Hoerabnahme ausgegeben.
Das kompakte Crusher-Modell nutzt die vorhandenen Maschinentexturen und echte
Speichenoeffnungen. Das Wasserrad misst jetzt 3 m im Durchmesser bei 1 m Breite;
20 statt 30 U/min und hoeheres Drehmoment erhalten seine bisherige Leistung.
Zwoelf Schaufeln fuellen/leeren sich an tatsaechlichen geladenen Wasserzellen;
zwei gemeinsam genutzte Meshes, keine neue Fluidsimulation/Frame-Meshuploads.
Das Ziel-HUD zeigt Drehzahl, Netzstress/-kapazitaet und die verbleibende Leistung
entlang des Quellpfads. Starr gekoppelte Wellen behalten physikalisch gleiche
Drehzahl, Zahnraduebersetzungen bleiben erhalten; Netzwerkprotokoll 84.
Die Greedy-AO-Verfeinerung erkennt reale Yzx-Oberseiten statt falscher X/Z-
Annahmen, damit Kontaktlicht nicht ueber ganze Grasrechtecke gezogen wird.
Inventar-Preview-Komposition ist auf ein Item pro Frame und zwei vorbereitete
Szenen begrenzt. Die zweite Kamera pausiert in unveraenderten mittleren
Warmup-Frames; acht Frames Shader-Wartezeit, echte Bildpruefung und sichere
Readbacks bleiben erhalten. Das behebt unbudgetierte Start-/Previewarbeit,
ist aber kein Nachweis fuer die gemeldete einzelne 8.1-FPS-Spitze. A05 bleibt offen.
Ein konkreter Autosave-Fehler entsteht durch verhungernde schwimmende Tiere:
Null Leben ohne Dead-Zustand wurde beim fruehen Ruecksprung nicht korrigiert.
Beduerfnisschaden setzt jetzt sofort Dead und stoppt vor weiterer Schwimm-KI.
Regressionen pruefen Land/Wasser, wilde/gezaehmte Tiere sowie echten Save/Reload;
ungueltige Zustaende werden weiterhin abgelehnt, jetzt mit genauer Fehlerursache.
Die kopierte echte Testwelt laedt, laeuft 35 s und speichert erfolgreich;
Originalwelt und Profil bleiben unveraendert. Nach 10 s Spiel-Warmup misst
dieser andere, stationaere Ausschnitt bei Sichtweite 16 AVG 73.33/min 39.94 FPS,
p99 20.26 ms; er ist kein Vorher-/Nachher-Flugvergleich.
Native Kinetikgalerie: sechs Ansichten, elf verschiedene Icons, echte
Wasserquelle an einem Rad und stabile 43 gemeinsam genutzte Maschinenmeshes.
Ein dabei entdeckter Wasser-Shaderfehler in Nebenviews ist korrigiert:
Transmission-Imports/SSR folgen VIEW_TRANSMISSION_TEXTURE, mit echter
WESL-Praeprozessorpruefung aller acht Transmission-/Depth-/Native-Kombinationen.
Der bestehende Hauptkamera-Pfad mit Transmission bleibt unveraendert.

Kurze native 32-s-Flugmessungen, RX 6950 XT, 1280x800, High, Sichtweite 20:

| Szene | AVG FPS | Min FPS | p99 ms |
| --- | ---: | ---: | ---: |
| Seed 42042/klar, vor Phase 156 | 67.36 | 26.03 | 26.57 |
| Seed 42042/klar, aktueller Code | 70.60 | 31.46 | 26.02 |
| Gleiches Binary, alte Preview-Warmup-Kamera als Gegenprobe | 68.58 | 29.00 | 25.87 |
| Seed 7/Regen, aktueller Code | 80.30 | 15.27 | 24.26 |
| Seed 7/Regen, Wiederholung mit CPU-Zeitfensteraufzeichnung | 80.04 | 37.68 | 23.21 |

Im klaren Messfenster sinken Mainpass-Aufrufe pro Renderframe von 1.891 auf
1.481, ohne Geometrie, Dichte, Wasser, Sichtweite oder Schatten zu reduzieren.
Die Verbesserung ist begrenzt: Der erste Regenlauf enthaelt einen 65.47-ms-
Ausreisser, der in der zeitgenau instrumentierten Wiederholung nicht auftritt.
Keine belegte Erklaerung der gemeldeten 8.1 FPS und keine allgemeine
32-FPS-Garantie; A05 bleibt offen. Die Wiederholung ersetzt den schlechteren
Lauf ausdruecklich nicht. Keine neuen 10-/15-/30-Minuten-Laeufe.
3088 Workspace-Tests bestanden, sieben bestehende bewusst ausgelassen.
Format, All-Targets-Check, striktes Clippy und Client-/Dedicated-Server-Build
bestanden. Keine Abhaengigkeits-Upgrades oder Engine-/Vendorpatches.
[Crusher im Betrieb](tools/benchmarks/results/compact-crusher-save-preview-2026-10-29/crusher/crusher-running.png),
[Kinetikgalerie](tools/benchmarks/results/compact-crusher-save-preview-2026-10-29/gallery/kinetics-forward.png),
[klarer Flug](tools/benchmarks/results/compact-crusher-save-preview-2026-10-29/teramic156-after.log),
[Regenlauf mit Ausreisser](tools/benchmarks/results/compact-crusher-save-preview-2026-10-29/teramic156-rain7.log).

Phase 157 repariert die realen Inventarwege von Tunnel und Funnel und setzt
F26 auf derselben Kinetik-, Inventar-, Drop- und Speicherarchitektur um.
Freie Tunnel importieren vor ihrem Pfeil und exportieren dahinter; gleiche
Hoehe und eine Ebene darueber/darunter bleiben kollisions- und ladegeprueft.
Eine echte seitliche Kurbel kann genau einen Anschluss zum zugehoerigen Crusher
oder Mixer vermitteln, aber keine beliebige Welle oder Wand. Extraktions-Funnel
lesen den wirklichen Ergebnis-Slot und geben auch ohne angetriebenes Band nach
unten aus. Hineingeworfene Stapel passieren freie und extrahierende Funnel mit
gleicher Drop-ID/Menge; Inserter behalten ihre Inventarlogik. Volle/versperrte
Ausgaenge halten zurueck. Unbeteiligte Drops koennen Mundstapel nicht mehr vor
dem begrenzten Kandidatenbudget verdraengen. Der getestete Weg verwendet den
echten Kurbelsolver: Crusher -> Funnel -> getragenes Band -> Tunnel -> Truhe.

Der neue Ein-Block-Mixer (ID 156) nutzt wiederverwendbare Holz-/Eisenmeshes,
seitliche Welle, Paddel-/Zahnradanimation und vorhandenes Maschinenfoley.
Last 24, vier Sekunden pro Charge bei 30 U/min; ohne Antrieb, bei Ueberlast,
unvollstaendiger Eingabe oder blockiertem Ergebnis wird nichts verbraucht.
Der Farbwaehler bestimmt einen exakten RGB-Code. Ein begrenzter linearer
Pigment-Fit mischt die 13 Grundfarben in ganzzahligen Dosen, normalerweise
16 Eingaben fuer 16 Farbstoffe; passende Standardtoene brauchen eine Dosis.
Die sechs Eingabeslots zeigen fehlende Dosen als transparente, nicht echte
Items. R auf einem vorhandenen RGB-Stapel zeigt dessen Mischrezept, ohne
Millionen Katalogeintraege oder freies Erzeugen von Zutaten. Normale Creative-
Inventarslots behalten ihre bisherige R-Regel. RGB-Farbstoffe lassen sich
stapeln, transportieren, speichern und zum Faerben von Capes verwenden.
Betten, Wolle/Teppiche und Band-Posen bleiben vorerst auf der Standardpalette;
nicht unterstuetztes RGB verbraucht dort keine Farbe. Das ist ein kalibriertes
Spielrezeptmodell, keine physikalische Farbstoffchemie.

Regionen mit Mixern erhalten die rueckwaertskompatible RGA6-Erweiterung;
andere Regionen und bisherige IDs bleiben unveraendert. Protokoll 85 verlangt
gemeinsam aktualisierte Clients/Server. Abbau prueft freie Dropkapazitaet vor
jeder Mutation und erhaelt Zutaten, Ergebnis und Kurbel in allen vier Yaw-
Ausrichtungen. Dynamische RGB-Drop-/Cape-Modellcaches sind auf je 128 Varianten
begrenzt; aktive Entity-Handles bleiben beim Cache-Evict gueltig.

TMI-Kategorien sitzen vertikal links ausserhalb der Browser-Border mit
Scroll-Overflow. Das Crusher-Icon nutzt die aktuelle kompakte Modellgeometrie
ohne alte Doppelbreiten-Skalierung. Der Mixer hat einen deckenden Hintergrund,
ohne den Welt-HUD zu entfernen. Ein dabei nachgewiesener asynchroner Teleport-
Inputfehler ist korrigiert: Der Besitzer der Sperre gibt sie nach Bestaetigung
und Tastenrelease frei oder uebergibt sie an den offenen Dialog.

3132 Workspace-Tests bestehen, sieben bestehende bleiben bewusst ausgelassen.
Format-Check, Workspace-Check, striktes All-Targets-Clippy und Client-/Dedicated-
Server-Build bestehen ohne Dependency-Upgrades oder Vendor-/Enginepatches.
Der native Mixer prueft echten Farbwaehler/RPC, Kurbel-RPM, Loslass-Pause,
exakten RGB-Ausgang, Rezeptghosts, R-Rezept und Entnahme mit drei Bildern.
Der native Inventarlauf mit echter OS-Maus besteht mit zwoelf Aufnahmen,
Tool-/All-Kategoriewechsel, Einzelgrant, Drag/Drop, Suche, 960x640/960x360-
Layout, Scrollen, Muelltonne und Input-Rueckgabe. Ein vorheriger Lauf wurde
wegen eines nicht erreichten OS-Zeigerziels abgebrochen; die vollstaendige
Wiederholung ist separat dokumentiert. Keine neue FPS-Zusage oder Langzeit-
Performance-Abnahme aus diesen funktionalen Szenen ableiten.
[Mixer](tools/benchmarks/results/mixer-rgb-logistics-2026-10-29/mixer/mixer-world.png),
[Farbwaehler](tools/benchmarks/results/mixer-rgb-logistics-2026-10-29/mixer/mixer-color-picker.png),
[RGB-Mischrezept](tools/benchmarks/results/mixer-rgb-logistics-2026-10-29/mixer/mixer-rgb-output.png),
[kompaktes TMI](tools/benchmarks/results/mixer-rgb-logistics-2026-10-29/inventory/inventory-creative-compact.png),
[Logistiktests](tools/benchmarks/results/mixer-rgb-logistics-2026-10-29/teramic157-logistics-targeted.log),
[nativer Mixer](tools/benchmarks/results/mixer-rgb-logistics-2026-10-29/teramic157-mixer-complete.log),
[nativer Inventarlauf](tools/benchmarks/results/mixer-rgb-logistics-2026-10-29/teramic157-inventory-trace.log),
[Workspace-Tests](tools/benchmarks/results/mixer-rgb-logistics-2026-10-29/teramic157-tests-complete.log).

Phase 158 erweitert die vorhandenen Faerbe- und Logistiksysteme. Exaktes RGB
funktioniert auf Wolle, Teppichen, Bettdecken und Foerderbaendern sowie den
bereits faerbbaren Capes. Holzgerippe, Bettgestell und Metall bleiben dabei
unveraendert. Faerberezepte erhalten RGB und Holzart, auch von Wolle zu Teppich;
Platzieren, Creative-Kopieren, Abbau und Bettrotation erhalten diese Metadaten.
R auf einem vorhandenen RGB-Stoff zeigt sein konkretes Faerberezept, ohne
einen Katalogeintrag pro Farbe. Inventar-, Hand- und Drop-Vorschauen verwenden
begrenzte Caches. Bewegte Baender verwenden eine gemeinsame Materialinstanz;
Umfaerben setzt weder ihre Animation noch den Itemtransport zurueck.
Reine Farbaenderungen loesen keine Terrain-, Wasser- oder Collider-Rebuilds aus.

Filtered Funnel (ID 157) hat einen echten Ghost-Filter ohne Itemverbrauch,
standardmaessig Allowed und umschaltbar auf Blocked. Auswahl erfolgt aus
Inventar/Katalog oder mit dem Cursor; Rechtsklick/Muelltonne leert den Filter.
Der Vergleich verwendet die gesamte Itemidentitaet einschliesslich RGB und
Holzart. Ohne Auswahl passieren alle Items. Der Server prueft geoeffneten
Dialog, Sichtlinie, Reichweite und Revision; der Ghost ist kein zusaetzliches
Inventar. Abgewiesene Mundstapel bleiben physisch liegen, statt durch die
Oeffnung oder in die passive Spieleraufnahme zu fallen. Stau, Entladen und
gezielte manuelle Entnahme erhalten die wirkliche Drop-ID und Menge.

Belt Merger (ID 158) sitzt auf einem vorhandenen Foerderband und verbindet
drei Eingangsrichtungen mit dem ausgerichteten Ausgang. Ein Round-Robin-
Cursor verhindert, dass ein dauernd belegter Eingang die anderen verdraengt.
Ein stehendes, entladenes, versperrtes oder belegtes Ausgangsband haelt zurueck;
die bestehende Bandlogik kann den Zusammenfuehrer nicht geradeaus umgehen.
Kein zweiter Transportmotor, keine Kopien von Stapeln und kein neues Inventar.
Beide neuen Bauteile sind im Technic-Katalog und als Werkbankrezepte enthalten.

RGA7 umhuellt die bisherigen Regiondaten mit validierter RGB-/Filter-Metadaten-
Erweiterung. Die binaeren alten Section-Felder bleiben eingefroren; Regionen
ohne neue Metadaten behalten ihr bisheriges Format. Wiederholtes Save/Reload
erhaelt Farben und Section-Revisionen, auch an negativen Regionsgrenzen.
Protokoll 86 verlangt gemeinsam aktualisierte Clients und Server.

3193 Workspace-Tests bestanden, sieben bestehende bewusst ausgelassen.
Format, Workspace-Check, striktes All-Targets-Clippy und Client-/Dedicated-
Server-Build bestanden. Der neue native RGB-Lauf prueft gespeicherte Farben,
Creative-Pick, echten Ghost-/Switch-Handler, unveraenderten Inventarbestand,
autoritaetliches Faerben und die konkrete Rezeptanzeige mit vier geprueften
Bildern. Auch der native Mixer-Regressionslauf besteht. Keine neuen Langzeit-
oder FPS-Zusagen, keine Dependency-Upgrades und keine Engine-/Vendorpatches.
[Nachweise und Grenzen](tools/benchmarks/results/rgb-fabrics-filter-merger-2026-10-29/README.md),
[Filter Allowed](tools/benchmarks/results/rgb-fabrics-filter-merger-2026-10-29/rgb/filtered-funnel-allowed.png),
[RGB-Stoffe und neue Logistik](tools/benchmarks/results/rgb-fabrics-filter-merger-2026-10-29/rgb/rgb-world-filter-merger.png).

Phase 159 erweitert RGB auf Wolltreppen und Wollslabs, einschliesslich direktem
Faerben, Formrezepten und gemischten Materialhalbstufen. Nur die Wollhaelfte wird
gefaerbt; Abbau bleibt getrennt. Unterschiedliche RGB-Wollhaelften in derselben
Zelle werden ohne Verbrauch abgewiesen, da die bestehende Metadatenstruktur nur
eine Farbe pro Zelle speichert. Ein echter Bandfarbwechsel wird auch dann
uebernommen, wenn die Bandgeometrie unveraendert bleibt. Protokoll 88 erfordert
gemeinsam aktualisierte Clients und Server. Funnel-Fuesse erkennen beladene
flache Baender als Unterstuetzung; Kontaktstege schliessen die Merger-Luecken.
Nachbar-Load/-Unload erneuert betroffene Funnel-Snapshots und verwirft alte
Worker-Ergebnisse. Ungeladene persistierte Nachbarn gelten nicht als Boden.
Explizit RGB-weisse und ungefaerbte Wollhaelften verlieren beim Mergen keine
Metadaten; nach Entfernen der Wollhaelfte bleibt kein RGB auf Stein zurueck.
Zur aktuellen visuellen Einzelreferenz fehlt der Bildanhang.

Ein reproduzierbarer Grund fuer gemeinsam stehenbleibende Tiere ist korrigiert:
Nachwuchs wilder Bienen war im Serverzustand erlaubt, wurde vom Client aber
mangels Spielerbesitzer als ungueltig verworfen. Damit fiel das ganze Mob-Paket
aus. Die eng begrenzte Wildbienen-Ausnahme verlangt gueltige Kolonie-/Familien-
Daten; andere ungueltige Nachkommen bleiben gesperrt. Sichtbare Bienenmetadaten
werden im begrenzten Paket bevorzugt. Ablehnungen erhalten einen Diagnosegrund
und begrenzte Warnrate statt stillen Ausfaellen.

Der echte Zwei-Client-UDP-Test deckt einen zweiten grundlegenden Syncfehler auf:
PlayerStates und CreatureStates teilten denselben SequencedUnreliable-Kanal.
Eine neuere Spielermeldung konnte dadurch eine fertige Mob-Meldung verdraengen,
noch bevor der typisierte Empfaenger sie las. CreatureSnapshotChannel trennt
diese Zustandsfamilien bei unveraenderter Uebertragungsart und getrennten Ticks.
Die Regression prueft echte serverseitige Mob-Alterung und gleichzeitig
fortschreitende Spielermeldungen bei beiden Clients; keine gelockerte Validierung.

Die Performance-Arbeit entfernt Welt-Vollscans aus Maschinen-, Truhen- und
Tueren-/Fenster-Kandidatenabfragen. Vorhandene Kinetikindizes und ein gepflegter
Objektsection-Index ersetzen wiederholtes Durchsuchen saemtlicher geladenen
Terrainsections. Statische Fussspuren erzeugen keine identischen Mesh-Updates
mehr; echte neue Spuren, Fade und Ablauf bleiben erhalten. Im Original-TiltUI-
Repository werden Shared-Abhaengigkeiten mit dem vorhandenen Parser erfasst:
Minimap-Bewegung wertet unbeteiligte Inventar-/Menuebindungen nicht erneut aus.
Lokale Kontexte, Aufrufe und dynamische Imports bleiben konservativ.

Eine erweiterte Kurzzeitmessung belegt den vertikalen Streaming-Ausreisser:
1606 Render-Hierarchien wurden in einem 39,71-ms-Frame synchron freigegeben
(10,50 ms allein in den kleinen Retire-Callbacks). Sofortiges Ausblenden ist
nun von der budgetierten ECS-/Asset-Freigabe getrennt; Mesh-/Materialbesitz
bleibt bis dahin erhalten. Harte Queuegrenzen besitzen einen Overflow-Fallback.
Redundante Roof-Pruefungen bei fehlenden/leeren Sections entfallen. Die Prop-
Nachbarinvalidierung nutzt bei grossen Entladungen die kleineren Owner-Mengen,
statt fuer jede entfernte Section alle 75 Netz-Nachbarpositionen zu pruefen.

Ein GPU-only-Culling-Versuch wurde nach negativem Kurztest verworfen, statt ihn
als weitere unbelegte Optimierung auszuliefern. Im finalen instrumentfreien
4K-Kurzflug steigt AVG von 67,59 auf 72,82 FPS, MIN von 26,69 auf 36,98 FPS;
p99 sinkt von 25,62 auf 20,45 ms. Ein kontrollierter Entladungsburst mit
1606 Hierarchien sinkt von 34,99 auf 26,93 ms. Regen auf Seed 7 hat weiterhin
Ausreisser: instrumentiert AVG 69,30 -> 71,62, aber MIN 29,29 -> 21,78 FPS.
Auch ohne Profiler erreicht der neue Regenlauf nur MIN 24,70; kein allgemeiner
32-FPS-Fix und kein Abschluss von A05. Alle 3253 Workspace-Tests und 328 Tests
im Original-TiltUI bestanden, ebenso Format/Check/Clippy/Client-/Server-Build,
nativer RGB-Test mit vier Bildern und Headless-Serverstart. Messbedingungen,
Rohdaten, einzelne schlechtere Werte und genaue Grenzen stehen im
[aktuellen Bericht](tools/benchmarks/results/rgb-mob-ui-performance-2026-10-30/README.md).
Keine neuen nativen Langzeitlaeufe und keine pauschale Grafik-/Sichtweitenreduktion.

Phase 160 schliesst die verbleibende seitliche Bandluecke am Merger durch
Kontaktzungen im vorhandenen Ribbon-Mesh. Das offene Lager zeigt nun den echten,
drehenden Shaft statt Holz bzw. einer statischen Metallkappe. Vier benachbarte
Shafts lassen sich vom ausgewaehlten ersten bis letzten verbinden; zusaetzliche
Shafts koennen auch im bestehenden Band und an dessen Controller gesetzt werden.
Sparsame BeltOwner-Metadaten erhalten Bandholz und RGB unabhaengig vom Shaft.
Protokoll 89 verlangt gemeinsam aktualisierte Clients und Server; RGA8 erweitert
den Regionumschlag, ohne das eingefrorene alte Sectionformat zu veraendern.

Neue Welten speichern die tatsaechlich verwendeten Biomeinstellungen. Highlands
erhalten selection_weight 0.65; Flussrevision 4 nutzt die bestehende begrenzte
Bachgenerierung mit 416 statt 512 m Rasterabstand. Fuenf Seeds ergeben 16,15 ->
13,58 % Highlands pro Land und 0,326 -> 0,411 % tatsaechliche offene Flusswasser-
flaeche pro Land (+26,1 %). Alte Welten und Generatorrevisionen bleiben erhalten.

Ein echter 4K-Flugausreisser auf Seed 86420 ist im Render-Fensterpfad lokalisiert:
118,93 ms prepare_windows bei 17,37 ms Main und 130,23 ms Gesamtframe. Der isolierte
Mailbox-Gegenversuch mit demselben unveraenderten Programm verbessert MIN von
7,68 auf 31,60 FPS; Seed 12345 mit Regen von 20,85 auf 38,80 FPS. Das ist kein
AVG-Gewinn auf beiden Seeds und keine 32-FPS-Garantie. Die Anwendung bevorzugt
ohne VSync nun Mailbox ausschliesslich unter Linux/AMD/RADV/Vulkan; keine Vendor-
oder Rendererpatches, Grafikreduktion oder FPS-Limitierung.

Im finalen 4K-/Sichtweite-20-Anwendungstest ohne Treiberoverride steigt Xwayland-
MIN auf 34,98 (Seed 86420) und 41,99 FPS (Regen, Seed 12345); AVG bleibt mit
71,96 bzw. 70,43 FPS leicht unter den Ausgangswerten. Natives Wayland verbessert
AVG 59,98 -> 79,55 FPS und p99 26,58 -> 19,77 ms, hat aber weiter einen Frame mit
MIN 30,32 FPS. Eine wirklich neu erstellte und gespeicherte Welt auf Seed 99173
besteht Laden/Reveal/Audio/Save und den 20-m/s-Regenflug mit AVG 63,20 / MIN 31,94
FPS. Damit ist die feste 32-FPS-Untergrenze nicht erreicht. Alle 3277 Workspace-
Tests, Format/Check/Clippy/Build, der native RGB-Test mit vier Bildern und der
Headless-Serverstart bestehen. Messbedingungen, Kontrolllaeufe, Rohdaten und
Grenzen stehen im
[Phase-160-Bericht](tools/benchmarks/results/belt-owners-present-biomes-2026-10-30/README.md).
A05 bleibt offen; keine neuen nativen Langzeitlaeufe.

Phase 161 fuehrt echten Item-Rueckstau auf den vorhandenen Baendern ein. Eine
begrenzte raeumliche Belegungsabfrage verhindert ueberlappende Drops und weitere
Inventarausgaben bei vollem Auslauf. Ports exportieren/importieren genau ein
Item je Transfer; Reststapel und IDs bleiben erhalten. Tunnel 152, Funnel 153
und Funnel (Filtered) 157 teilen den vorhandenen autorisierten Ghost-/Allowed-/
Blocked-Filterpfad. Passende, bestaetigte Inventaranschluesse verlaengern das
vorhandene Bandmesh bis an die Inventarflaeche; unbekannte oder entfernte
Nachbarn erzeugen keine falsche Verbindung.

Formbloecke erhalten Schnee auf ihrer wirklichen Oberflaeche statt flacher,
schwebender Wetterkappen. Treppen, Slabs und Fences stapeln keine natuerlichen
Schneeplatten; Wege erhalten nur dezenten Reif, auch bei voller gespeicherter
Schneemenge. Duennes SnowLayer 98 ist ueberbaubar, voller SnowBlock 5 bleibt
geschuetzt. Die natuerliche Ansammlung ist viermal langsamer; Wintererhalt und
Spielerplatzierung bleiben bestehen. Benannte und gemischte RGB-Farbstoffe
stehen in der neuen TMI-Kategorie Rohstoffe, ohne RGB-Katalogvervielfachung.

Die Replica-Lichtsuche verarbeitet Section-Ereignisse statt jede Metadaten-
Aenderung als Vollscan zu behandeln. Neue Rohdatenabfragen bleiben budgetiert,
Unload/Reset und Nachbarverdeckung sind getestet. Die Portal-Wandringberechnung
nutzt bei dicht erreichbaren Welten das kleinere Komplement, bei exakt gleichem
Sichtbarkeitsergebnis. In zwei finalen 4K-/Sichtweite-20-Kurzfluegen sinken die
Lichtauswahlkosten etwa 59/63 % und Portal-Workerzeiten etwa 27/25 %. AVG geht
85,78 -> 88,75 bzw. 82,94 -> 87,47 FPS; MIN 7,67 -> 38,35 bzw. 45,53 -> 43,47.
Die Regen-Mindestframerate ist damit leicht schlechter. Ein frueherer Lauf
hatte weiterhin MIN 30,92 und keinen allgemeinen AVG-Gewinn. Saemtliche
Wiederholungen, hoehere RSS-Endwerte und Fensterpfad-Ausreisser sind dokumentiert;
keine garantierte 32-FPS-Untergrenze und kein Abschluss von A05.

Alle 3308 Workspace-Tests, Format/Check/Clippy/Build, der zusaetzliche CPU-
Vergleich, native Filter-/Bandtests mit fuenf Bildern, die 16-stufige Material-
Galerie mit 158 GPU-Previews und der Headless-Serverstart bestehen. Die neuen
Band- und Schneeansichten wurden tatsaechlich visuell geprueft; eine zuerst zu
weisse Wegoberflaeche wurde anschliessend begrenzt und erneut aufgenommen.
Details, Rohdaten und Bilder stehen im
[Phase-161-Bericht](tools/benchmarks/results/funnel-queues-snow-cpu-2026-10-30/README.md).
Keine neuen nativen 10-/15-/30-Minuten-Laeufe und keine Grafikreduktion.

Phase 162 erweitert dieselben Band-/Drop-Pfade um fortlaufende Junction-Routen.
Zusammenfuehrer bedienen die drei Eingaenge reihum, jeweils genau ein Item;
historische Stapel werden ohne Mengenverlust aufgeteilt. Wartende und
abbiegende Items bleiben vor automatischem Aufheben geschuetzt und koennen
weiter mit Rechtsklick genommen werden. Volle/gesperrte Ausgaenge stauen sich
zurueck, statt Items an den Auslass zu teleportieren.

Tunnel (Filtered) 159 verwendet denselben Ghost-/Allowed-/Blocked-Filter wie
Funnel (Filtered). Verteiler 160 besitzt einen Eingang und drei voneinander
unabhaengig gefilterte Ausgaenge. Die private Konfiguration ist autorisiert,
in RGA9 gespeichert und mit bisherigen Regionen abwaertskompatibel; alte
Block-IDs und RGA8-Payloads bleiben unveraendert. Spieler, gerittene Pferde und
andere aktive Mobs werden auf laufenden Baendern mit vorhandener gesweepter
Kollisionsbewegung transportiert, nicht durch Transform-Teleports.
Geneigte Band-Laeufe besitzen durchgehende, Section-geclippte Flaechen im
bestehenden Terrain-Collider statt kleiner AABB-Tritte. Wellen bleiben
kollidierbar; Picking und Mob-Abfragen behalten konservative Profilboxen.
Bandadvektion wird nicht als Sprung gespeichert, waehrend XZ weiterhin die
tatsaechliche Weltgeschwindigkeit fuer Replikation bleibt.

Farbstoffe einschliesslich RGB stehen wieder bei den Materialien; die
mitgelieferte Kategorie Rohstoffe entfaellt. Natuerlicher Schnee bleibt auf
zwei Achtelschichten begrenzt. Spieler koennen manuelle Platten bis zum vollen
Schneeblock stapeln oder Formen wie Treppen, Slabs, Fences und Wege direkt
beschichten. Neue manuelle Schichten haben unterscheidbare gespeicherte Posen;
alte Posen bleiben lesbar. Eine budgetierte Shape-Runde schliesst generierte
Dorfwege ein und laesst berechtigte Oberflaechen bei Schneefall innerhalb von
fuenf Minuten winterlich werden, ohne Schnee allein aus der Jahreszeit zu
erzeugen.
Eine rein optische Mindestablagerung fuer geeignete Formbloecke sichert diese
Frist auch bei schwachem tatsaechlichem Schneefall; sie beschleunigt keine
physischen Bodenschichten und rechnet keine vorherige Klarwetterzeit nach.

Sand-/Schneefussspuren erhalten echte vertiefte Boeden und Seitenwaende in
einem begrenzten gemeinsamen Mesh. Ein fester raeumlicher GPU-Lookup schneidet
die passende Originaloberflaeche in Farb- und Tiefenpass aus, auch trocken
unter Daechern. Unterstuetzung, Oberflaechenhoehe, negative Koordinaten,
Nachbaraenderungen und Unload werden beruecksichtigt; keine Terrain-Neumeshing-
Jobs oder ECS-Entities pro Schritt. Vergehende Spuren werden flacher und
kleiner.

Phase 162 ist mit 3359 bestandenen Workspace-Tests, 0 Fehlern und 8 ignorierten
Tests geprueft; Format, Check, strenges Clippy und Workspace-Build bestehen.
Die 13 echten Avian-Bewegungstests decken laufende Baender, Waende, Absaetze,
Rampen, Gegenlaufen und Springen ab. Native Logistik-/Filterpruefung mit sieben
Bildern, vollstaendige Material-Galerie mit 160 GPU-Previews und 15 Bildern,
vier Fussabdruck-Aufnahmen mit verifizierten vertieften Bodenpixeln sowie der
Headless-Serverstart mit 27 Sections bestehen. Filterbeschriftungen,
Schneeformen und Fussabdruecke wurden tatsaechlich visuell kontrolliert.
Details und lokale Bilder stehen im
[Phase-162-Bericht](tools/benchmarks/results/junction-transport-snow-relief-2026-10-30/README.md).
Keine neuen 10-/15-/30-Minuten-Laeufe und keine unbelegten FPS-Zusagen.

Phase 163 korrigiert nach den echten Spielaufbauten die Junction-Regressionen:
Der Zusammenfuehrer trennt Eingangszuordnung von Recovery, behaelt wartende
RR-Zuege und laesst fruehes kollisionsgeprueftes Nachruecken ohne Inline-
Vorfuellung zu. Echte verbundene Bandenden werden an ihrem tatsaechlichen
Drop-Zentrum uebergeben statt an einer ungueltigen 0.78-m-Probe. Verteiler
reservieren passende Allowed-Filter vor allgemeinen Ausgaengen; Links/Rechts
stimmen mit dem Spieler-Koordinatenvertrag ueberein. Gespeicherte abgebogene
oder blockierte Koepfe behalten pausierte Custody, ohne unabhaengige Ausgaenge
bei Recovery zu sperren. Keine neuen Block-IDs oder Speicher-/Protokollformate.

Wet-/Imprint-Shader aktivieren ihre eigene Bindless-Extension explizit. Die
neue Kompositionsregression prueft zwoelf Varianten und reproduziert den
urspruenglichen fehlenden Enable-Pfad als Negativkontrolle. Alle 3369 Workspace-
Tests bestehen (0 Fehler, 8 ignoriert), einschliesslich sechs echten Linked-
Band-/Kurbeltests, strikter Alternation und unveraenderter maximaler 0.5-m-
Ausgangsluecke bei 1 m/s fuer alle vier Ausrichtungen. Format, Check, strenges
Clippy und Build bestehen. Native GPU-Pixeltests bestehen in neun Phasen je
Bindless-/Legacy-Pfad auf der RX 6950 XT ohne Shaderfehler; Headless-Serverstart
mit 27 Sections besteht. Die frueheren Einzelzellen-/Filter-UI-Tests deckten
diese realen Endpoint- und Tiefenpassvertraege noch nicht ab.
Details, Rohlogs und GPU-Bilder stehen im
[Phase-163-Bericht](tools/benchmarks/results/junction-order-bindless-2026-10-30/README.md).

Phase 164 implementiert F30/F31 und die zufaellige Truhenentnahme fuer beide
Tunnelvarianten. Der gespeicherte Zufallsschalter waehlt einen belegten,
filterkompatiblen Slot aus beiden Haelften einer Doppeltruhe; Stackgroesse
gewichtet nicht, Maschinen liefern weiterhin nur ihren Ergebnisslot und
Rueckstau verbraucht keine Items. Pro Transfer bleibt es bei einem Item.

Kessel 161 und Lagerfeuer 162 ergaenzen die bestehenden Block-/Inventar- und
Effektsysteme. Sechs JSON-Braurezepte enthalten getrennte Effektstaerken und
Laufzeiten, darunter Regeneration + Staerke. Holz-/Stein-/Goldloeffel sind
wiederverwendbar und beschleunigen nur die Brauarbeit mit 1.0/1.2/1.5.
Wasserflasche, Eimer, Zutaten und leere Flasche werden autoritativ verrechnet;
Sand -> Glas im Ofen und Glas -> Flaschen schliessen die Survival-Rezeptkette.
Trinken verwendet die gehaltene Benutzen-Aktion, gibt in Survival eine Flasche
zurueck und kann trotz Giftschaden fertig werden, ohne den Schaden zu heilen.
Wasser loescht nur den Feuerstatus; fremde Effekte bleiben erhalten.

Lagerfeuer verwenden echte Brennstoffzeiten und vorhandene Garrezepte, heizen
einen darueberstehenden Kessel und verwenden begrenzte vorhandene Feuer-/Rauch-/
Licht-/Knisterpfade. Wasser, exponierter Regen, sichere Aufstellung, Flammen-
kontakt und Sichtkontakt fuer die Waerme werden geprueft. Unbekannte Nachbar-
Sections pausieren die Simulation, ohne gespeicherten Brennstoff zu verlieren.
Ein kleiner Waerme-Hook fuer F32 ist vorhanden, kein vorgetaeuschtes fertiges
Temperatursystem. Kessel, Loeffel, Flaschen und Feuer verwenden gemeinsame
Meshes/Materialien; der Feuerunterbau bekommt nur bei einem Kessel darueber
passende bodengestuetzte Steinpfeiler mit denselben Picking-/Kollisionsboxen.

Der regionale RG10-Wrapper speichert Brauarbeit, Wasser, Brennzustand und
Tunnelmodus; historische RGA9-Daten bleiben lesbar und unveraendert definiert.
Bestehende Block-IDs und Generatorversion bleiben erhalten. Client und Server
muessen gemeinsam Protokoll 91 verwenden. Beide Stations-UIs und Too Many
Items zeigen reale Slots, Zutaten und individuelle Effekte. Bedienung steht in
[Blocks README](assets/terra/blocks/README.md#alchemy-and-campfires).

Abschlusspruefung auf dem finalen Quellstand: 3422 Workspace-Tests bestanden,
0 fehlgeschlagen, 8 explizit ignoriert. Format, Workspace-Check, strenges Clippy
mit allen Targets und Workspace-Build bestehen. Der Headless-Serverstart
besteht mit 27 Sections. Neue Regressionen pruefen unter anderem beide Tunnel,
Filter/Rueckstau, Legacy-Speicherstaende, echte Brau-/Gartransaktionen, Wasser-
und Flaschenrueckgabe, Schaden waehrend des Trinkens, Feuer-/Ladegrenzen sowie
identische Unterbauformen in Rendering, Picking und Physik.

Der native Kurztest auf der RX 6950 XT/Vulkan besteht in 140.9 Sekunden mit
sieben nichtleeren Aufnahmen: gespeicherter Aufbau, echter UI-Wassertransfer,
Mischtrank, Too-Many-Items-Rezept, Anzuenden, Braten und Loeschen. Modelle,
Rezeptauswahl und Brennstoffanzeige wurden visuell kontrolliert. Keine neuen
10-/15-/30-Minuten-Laeufe und keine FPS-Zusagen. Die lokale Audioausgabe meldet
allerdings ALSA-POLLERR; der finale Test erfasst 12870543 wiederholte Rodio-
Backendfehler, im Testlog auf drei Beispiele plus Anzahl begrenzt. Keine
Produktionsunterdrueckung, keine behauptete Hoerabnahme oder Behebung des
Audiogeraets: A02 bleibt offen. Screenshots, native Logdatei und alle Cargo-
Prueflogs stehen im
[Phase-164-Ergebnisordner](tools/benchmarks/results/alchemy-campfire-2026-10-30/).

Phase 165 ergaenzt sechs negative Braurezepte: Gift, Verlangsamung, Frieren,
Blutung, Verderbnis und Feuer. Bestehende Trank-IDs 1-6 bleiben unveraendert;
7-12 verwenden dieselben Rezept-, Effekt-, Trink- und Too-Many-Items-Pfade.
Gift wird aus Pilz und Kohlestaub gebraut (Stufe 1, 20 Sekunden). Zutaten,
Wasser, wiederverwendbarer Loeffel und Glasrueckgabe bleiben autoritativ.

Ein Lagerfeuer passt jetzt direkt in das Kesselgestell derselben logischen
Zelle. Beide Survival-Platzierungsreihenfolgen erhalten Brennstoff und Inhalt;
Kessel und Feuer haben getrennte, umschaltbare Inventare. Ein veralteter
UI-Befehl kann dadurch nicht versehentlich das andere Inventar bearbeiten.
Die echte Platzierungstaste hat beim passenden gehaltenen Gegenstueck
Montage-Prioritaet; ein bereits montierter Aufbau oeffnet weiterhin sein
Inventar. Tests pruefen beide Reihenfolgen mit Rechtsklick und umgebundener
Taste sowie normale Stationsnutzung mit Nahrung in der Hand.
Rendering, Picking, Waermekontakt und Kollision verwenden denselben kompakten
Aufbau; Abbau gibt beide Bloecke und beide Inhalte genau einmal zurueck.
Alte gestapelte Aufbauten bleiben unveraendert und koennen neu aufgebaut
werden. RG11 speichert den Zusatzaufbau samt eigenem Inventar und liest
weiterhin RG10/RGA9. Client und Server verwenden gemeinsam Protokoll 92.

Erhitzte Fluessigkeit besitzt bewegte Oberflaeche, sechs Blasenkuppen und
Dampf, auch ohne laufendes Rezept. Kalte Fluessigkeit bleibt zeitlich stabil.
Die vorhandenen begrenzten FX-Batches werden weiterverwendet, ohne Entities
oder Materialien pro Blase. Beide Tunnel-UIs zeigen ein sichtbares Label
"Random" am gespeicherten Zufallsschalter.

Ein belegter vermeidbarer Renderpfad ist entfernt: Inventarvorschaubilder
wurden vorher fuer den gesamten Katalog schon beim Weltbeitritt erzeugt.
Nun werden nur tatsaechlich benoetigte Hotbar-, Inventar-, Rezept-, Kategorie-
und Ziel-Icons angefordert. Veraltete wartende Jobs werden entfernt, laufende
Readbacks korrekt beendet und RGB-Vorschaubilder bleiben begrenzt. Bestehende
Bildgueltigkeitspruefung und Wiederholungen bleiben erhalten. Vollkatalog-
Galerietests koennen TERRA_PREVIEW_WARMUP=1 explizit verwenden; normale Spiele,
Alchemy- und Benchmark-Laeufe bleiben bedarfsgesteuert.

Erste Messreihe nur zur bedarfsgesteuerten Icon-Erzeugung: kurze native
Vorher-/Nachher-Flugtests auf RX 6950 XT/Vulkan, 3840 x 2160,
Sichtweite 20, hohe Qualitaet, echte TiltUI und jeweils 48 Sekunden Messfenster:

| Seed / Wetter | Stand | AVG FPS | Min FPS | p99 ms | RAM MiB |
| --- | --- | ---: | ---: | ---: | ---: |
| 86420 / Klar | Vorher | 88.47 | 15.76 | 18.16 | 2408.4 |
| 86420 / Klar | Nachher | 87.29 | 40.48 | 17.94 | 2323.7 |
| 99173 / Regen | Vorher | 82.55 | 40.96 | 18.86 | 2196.6 |
| 99173 / Regen | Nachher | 83.80 | 40.88 | 18.82 | 2052.9 |

Nachher benoetigt jeder gemessene Frame nur einen Core3d-Ansichtsdurchlauf;
vorher kamen 2141 beziehungsweise 2294 zusaetzliche Icon-Ansichtsdurchlaeufe
hinzu. Die Nachher-Fenster enthalten keine Frames unter 32 FPS, garantieren
aber keine allgemeine FPS-Untergrenze. AVG ist praktisch unveraendert; ein
grosser allgemeiner FPS-Gewinn oder eine vollstaendige Behebung aller Drops
ist nicht nachgewiesen. Der schlechteste Klarwetter-Vorher-Frame beinhaltet
49.52 ms Fenster-/Swapchain-Vorbereitung bei unfokussiertem Fenster. Das ist
eine gemessene Korrelation, keine bewiesene Treiber- oder Betriebssystemursache.
Bei diesem Zwischenstand benoetigte PostUpdate im Mittel rund 5.3 ms pro Frame.
Keine Qualitaetsreduktion, kein FPS-Limit und keine neuen Langzeittests.

Die anschliessende begrenzte GDB-Stichprobe (16 Stops, eigener Clientprozess)
trifft zweimal den Neuaufbau von QueryState/Archetypen in den eigenen
Tooltip-Funktionen fit_contents und position. Diese liefen auch bei
geschlossenem Inventar pro Frame; zusammen mit fit_identifier wurden sieben
Abfragen immer neu aufgebaut. Ein weltlokaler TooltipQueries-Cache verwendet
jetzt persistente QueryStates und aktualisiert weiterhin neue Archetypen.
Die Regression prueft wiederholte Frames, spaeter erzeugte Tooltips, Window,
Pointer und Texte, Mausbewegung, Identifier-Kuerzung und Owner-Despawn bei
genau einer Cacheinitialisierung. Keine Layout-/Hover-/Portalregeln oder
TiltUI-/Bevy-Quellen geaendert. Die Stichprobe zeigt weitere TiltUI-/Render-
Pfade, liefert aber keine exklusiven Systemzeiten. Deren kuenstliche Stop-
Pausen verursachen auch Audio-Unterbrechungen; die Debugger-Framewerte sind
ausdruecklich keine FPS-Benchmarkwerte und nicht Bestandteil der Tabelle.

Zweite Messreihe mit frischen Vorher-Laeufen fuer den Tooltip-Cache, ebenfalls
48 Sekunden pro Lauf, 4K, Sichtweite 20 und unveraenderte Grafikqualitaet:

| Seed / Wetter | Tooltip-Cache | AVG FPS | Min FPS | p99 ms | PostUpdate Mittel ms |
| --- | --- | ---: | ---: | ---: | ---: |
| 86420 / Klar | Vorher | 86.33 | 36.93 | 18.81 | 5.37 |
| 86420 / Klar | Nachher | 90.06 | 37.49 | 17.28 | 4.99 |
| 99173 / Regen | Vorher | 81.66 | 34.83 | 18.27 | 5.29 |
| 99173 / Regen | Nachher | 86.82 | 44.66 | 17.47 | 4.73 |

Damit steigt AVG in diesen Fenstern um rund 4.3 beziehungsweise 6.3 Prozent;
PostUpdate benoetigt rund 7.1 beziehungsweise 10.5 Prozent weniger Wandzeit.
Alle vier Fenster enthalten keine Frames unter 32 FPS. Das sind begrenzte
Messungen, keine garantierte Mindest-FPS oder vollstaendige Behebung aller
Streaming-/Treiber-/UI-Spitzen. Asynchrone Upload-/LOD-Zeitpunkte und damit
einzelne sichtbare Batchzahlen variieren; die Rohlogs enthalten Szene und
effektive Konfiguration. CSV trennt die Icon- und Tooltip-Messreihen.

Der native Alchemy-Kurztest auf dem finalen Build besteht mit 13 nichtleeren Aufnahmen in 35.3
Sekunden: kompakter gespeicherter Aufbau, echter Wassertransfer, Brodeln,
Mischtrank, Giftrezept, beide Feuerinventare, Braten, Loeschen und beide
Random-Labels. Aufbau, brodelnde Fluessigkeit, Giftrezept und Random-Label
wurden an den erzeugten Bildern visuell kontrolliert. Keine Shader- oder
Rodio-Backendfehler in diesem Lauf; eine menschliche Hoerabnahme bleibt A02.
Das Dropdown enthaelt alle zwoelf Optionen und besitzt einen Scrollpfad;
Scrollen und Anklicken des siebten Eintrags wurde nativ nicht separat geprueft.
Alle 3453 Workspace-Tests bestehen, 0 Fehler, 8 explizit ignoriert. Format,
Workspace-Check mit allen Targets, strenges Clippy mit allen Targets und
Workspace-Build bestehen. Der Headless-Serverstart besteht mit 27 Sections.
Die neuen Tests decken unter anderem beide Montage-Reihenfolgen, getrennte
Inventaransichten, echte Zutaten-/Brennstofftransaktionen, Pause/Reload,
RG11/Legacy-Speicherstaende, begrenzte Icon-Anforderungen und gueltige
Blasen-/Kollisionsgeometrie ab. Beim Nachlegen prueft die Regression den
gesamten verbleibenden Brennstoff, nicht nur den korrekt wieder steigenden
aktuellen Brenntimer.
Rohlogs und Screenshots stehen im
[Phase-165-Ergebnisordner](tools/benchmarks/results/alchemy-compact-frame-tails-2026-10-31/).

## Phase 166: Start-Icons Und Wassertechnik

UI- und Inventar-Icons werden beim normalen grafischen Start einmalig vor
Weltbeitritt beziehungsweise Serverstart vorbereitet. Der endliche Katalog
enthaelt auch die bekannten Holz-, Bett- und Cape-Varianten. Die fertigen
Bild-Handles bleiben erhalten; frei gemischte RGB-Varianten verwenden weiter
den begrenzten Bedarfscache. TiltUI-Symbole aller drei Laufzeitgroessen,
statische UI-Bilder und Schriften gehoeren zum Start-Gate. Fehlende Assets
werden als Fehler angezeigt statt eine endlose Ladephase zu erzeugen.

F27/F28 sind umgesetzt: Holz-, Stein- und Eisenrohre, Ventil, Wasserhahn,
kinetische Pumpe und modulare Becken mit angehaengten Block-IDs 163..169.
Rohre verbinden sich entlang ihrer tatsaechlichen Nachbarn; vollstaendige
rechteckige Becken verlieren ihre inneren Waende und halten einen gemeinsamen
Pegel. Unterstuetzt werden Grundflaechen von 1x1 bis 5x5, auch 1x2, und bis
zehn Bloecke Hoehe. Die Darstellung orientiert sich an der gelieferten
Vorlage; eine pixelgenaue 1:1-Uebereinstimmung wird nicht behauptet.

Die Wassertechnik verwendet bestehende FluidCells, Kinetik und den vorhandenen
Terrain-Fluessigkeitsscheduler. Quellenaufnahme, endlicher Speicher, Gefaesse
und echter Ausfluss am Hahn sind gekoppelt. Geschlossene Ventile, fehlender
Antrieb und ungeladene Verbindungen verhindern ungueltige Versorgung.
Der untere Zulauf nutzt die freie Kapazitaet des gesamten Beckens; ein
strikt positiver Druckgradient verhindert den Ruecklauf in seine eigene
Quellkammer im selben Update. Mengenbilanz, voller Speicher, unvollstaendige
Formen sowie Unload/Reload sind durch Regressionen abgesichert.

Flasche, Eimer und Giessekanne fuellen sich sichtbar beim Halten der
Platzieren-/Benutzen-Aktion an einer sichtbaren echten Quelle oder einem
versorgten Hahn. Abbruch, geaenderte Slots und Inventare sowie verdeckte Ziele
werden geprueft. Der Server uebertraegt erst nach der Haltezeit echte Einheiten.
Die Giessekanne kippt, zeigt einen Wasserstrahl und bewaessert bepflanzte Beete.
Ihre bestehende Reserve bleibt sitzungslokal, nicht je einzelner Kanne gespeichert;
eine neue Feuchtigkeits-Persistenz fuer unbepflanzten Boden wurde nicht eingefuehrt.

Ein nativ gefundener Revisionskonflikt ist behoben: Gefaessaktionen an Hahn und
Becken verwenden die stabile Blockrevision statt der bei laufendem Wasser
fortwaehrend wechselnden Wasser-Meshrevision. Natuerliche Wasserzellen behalten
ihren Fluessigkeits-Revisionsschutz. Unveraenderte Inventare und Drops werden
ausserdem nicht mehr wegen jeder privaten Wassertechnik-Aenderung erneut
versendet. RG12 speichert Leitungsinhalte und Beckenformen, ohne die RG11-Huelle
zu aendern; alte Regionen bleiben lesbar. Netzwerkprotokoll ist 93.

`TODO.md` enthaelt nur offene Aufgaben. Die bisherigen Berichte stehen im
oben verlinkten Altstand; erledigte F27/F28 und die erledigte Vorlade-Anforderung
wurden nach der Pruefung entfernt. Neu sind F44..F52 fuer Inventarsortierung,
Create-artige Rezeptansichten, Rezept-UI, experimentelle Chemie, Rucksaecke,
KI, /heal, /give mit Farbe und eine F3-artige Debugansicht. O07 und externe
Abnahmen bleiben offen.

Final erfolgreich: `cargo fmt --all --check`, `cargo check --workspace -j1`,
`RUST_TEST_THREADS=4 cargo test --workspace -j1`,
`cargo clippy --workspace --all-targets -j1 -- -D warnings`,
`cargo build --workspace -j1`. Insgesamt 3511 bestandene Tests, null Fehler,
acht explizit ignorierte Spezialtests. Der gezielte Hydrauliklauf besteht
mit allen 14 Welt-Tests, einschliesslich der drei neuen Becken-Regressionen.

Native Kurztests auf RX 6950 XT / RADV / Vulkan: neun geprueft nichtleere
Wassertechnik-Aufnahmen in 19.51 Sekunden mit echten Rechtsklick-Aktionen,
Ventilsperre und auf 100 Prozent bewaessertem Beet. Wasserstand von Flasche,
Eimer und Becken sowie der Giesstrahl wurden visuell kontrolliert.
Der getrennte Starttest bestaetigt vor jeder Welt-/Sessionerzeugung 454 fertige
Inventar-Icons und 1611 UI-Symbole; alle 1624 UI-Vorbereitungspositionen sind
erledigt und keine Vorschaujobs uebrig. Die finale Hauptmenue-Aufnahme enthaelt
keinen veralteten Ladehinweis. Beide grafischen Laeufe enthalten keine
Warnungen, Panics, Shader- oder Rodio-Backendfehler. Der separate Headless-Start
besteht mit 45 Sections. Keine neuen FPS-Vorher-/Nachher-Messungen und keine
10-/15-/30-Minuten-Laeufe; daraus wird kein allgemeiner FPS-Gewinn abgeleitet.

[Phase-166-Nachweise Und Reproduktion](tools/benchmarks/results/water-tech-startup-2026-10-31/README.md).

## Phase 167: Naturwasser-Ansaugung, Runde Rohre Und Direkte Gefaessfuellung

Laufende Pumpen ziehen Wasser ueber einen zusammenhaengenden Zulauf aus Meer,
See und Fluss auch dann an, wenn die Pumpe oberhalb des natuerlichen Pegels
steht. Die gerichtete Suche bleibt im vorhandenen Komponentenbudget; Druck
entsteht erst nach dem Pumpendurchtritt. Ventile, Drehrichtung, fehlender
Antrieb, entnommene Quellen und ungeladene Verbindungen bleiben wirksame
Sperren. Endliche Becken behalten ihre Mengenbilanz.

Rohrenden und Verzweigungen verwenden runde Hohlprofile statt des bisherigen
Wuerfelkerns. Ein einzelner Anschluss behaelt ein gegenueberliegendes offenes
Ende; Boegen und Mehrfachverteiler besitzen passende Auswahl-/Kollisionsformen.
Die vorhandenen Material-/Flanschslots werden weiterverwendet; ein auf 64
Varianten begrenzter Cache je Meshing-Auftrag vermeidet wiederholten Profilbau.

Externes Wasser kann den freien Raum um Rohre, Ventile, Haehne und Pumpen
belegen. Es bleibt vom gespeicherten Leitungsinhalt getrennt. Die Wasserflaeche
bleibt bei Konstruktionen in oder direkt ueber dem Wasser erhalten; natuerliche
Bruchpegel werden ohne Achtel-Aufrundung gezeichnet. Ganzzahlige Oberflaechen
an positiven und negativen Y-Section-Grenzen haben einen eindeutigen Besitzer,
auch wenn der darunterliegende Chunk noch nicht geladen ist. Beim Reload
beruecksichtigt die Erstpublikation Teilbloecke im Wasser-Halo.

Eimer und Giessekanne senden an einer sichtbaren echten Quelle bereits beim
ersten Rechtsklick ihre Fuellaktion. Gehaltene Eingabe wiederholt sie nicht;
Sichtlinie, Rechte, Reichweite, Slot und Inventarrevision prueft weiterhin der
Server. Flaschen und endliche Hahnversorgung behalten die sichtbare Halteaktion.

Die BR-52-Bildreferenz liegt dauerhaft unter docs/references/br52-train-system.png.
TODO F53 beschreibt das noch offene fahrbare Zugsystem einschliesslich Gleisen;
F54 listet den modularen Dampflok-Baukasten, Tender, Gestaenge und Dekoration.

Geprueft: `cargo fmt --all --check`,
`cargo test --workspace --lib --bins -j1` (3515 bestanden, 0 Fehler,
8 bestehende Tests ignoriert) und `cargo build --workspace -j1`.
Die neuen Regressionen pruefen Naturansaugung, Sperren, endliche Mengen,
alle 57 Rohr-Anschlussformen, Wasserflaechen bei 7.3/7.5/8/16/-16,
positive/negative Section-Grenzen, ungeladene untere Nachbarn und echten
Rechtsklick ohne lokale Inventarmutation.
Der native Hydraulik-Kurztest bestand in 16.78 Sekunden mit neun Aufnahmen:
gespeichertes Becken, Flaschen-/Eimer-/Kannenfuellung, Ventilsperre und echte
Bewasserung. Runde Rohrkoerper und Bogen sowie der gefuellte Eimer sind in der
[Nahaufnahme](tools/benchmarks/results/water-pipes-intake-2026-11-01/hydraulics-native/bucket-filled.png)
sichtbar. Das native Log enthaelt keine Warnung, keinen Fehler und keine Panic.
Naturansaugung und die Oberflaechen-Randfaelle sind durch die automatisierten
Regressionen belegt. Logs und Aufnahmen liegen im
[Ergebnisordner](tools/benchmarks/results/water-pipes-intake-2026-11-01/).

## Phase 168: Wasserinteraktionen, Becken, Hahn-Stufen Und Start-Ladescreen

Der lokale Wasser-Meshzaehler war kein zwischen Server und Replica vergleichbarer
Transaktionszaehler. Gefaessaktionen verwenden jetzt die replizierte Blockrevision;
Quelle, Menge, Sichtlinie, Reichweite, Rechte und Slot bleiben serverseitig geprueft.
Prepared Naturwasser-Spalten gehoeren zur gemeinsamen Quellen-Zielerfassung.
Ein eigener Mining-Picker durchquert Quellbloecke; Client und Server verwenden
identische Regeln, waehrend Rohrkoerper, Props und Wasserpflanzen Ziele bleiben.
Der native Hydrauliktest fuellt jetzt auch an echten Quellen per kurzem Rechtsklick
und baut Boden unter Wasser ab; die vorherige Grafikfixture pruefte nur den Hahn.

Uferkontaktpunkte bleiben verankert, offene Wasserflaechen behalten ihre Wellen.
Auch aktuelle platzierte Bloecke im gepaddeten Feld zaehlen als Ufer. Bei ganzzahligen
Pegeln prueft die Naturflaeche den tatsaechlichen Wasserraum unter der geometrischen
Ebene, damit keine zweite transparente Flaeche auf einem Gras-/Sandblock liegt.
Ein schmaler Naturwasser-Uebergang um Teilkoerper verbindet exakte Rohrflaechen mit
geneigten Flusspegeln; volle explizite Quellen an 16/-16 behalten denselben oberen
Flaechenbesitzer wie das Naturwasser. Positive/negative XZ-Naehte werden geprueft.

Das Beckenmodell verwendet disjunkte Schalenzellen mit genau einer Flaeche je
Grenze. Waende, Ecken, Rand und Metallbaender besitzen keine koplanaren Ueberlagerungen;
Nachbarmodule unterdruecken ihre gemeinsamen Endflaechen. Vorhandene stilisierte
Holz-, Stein- und Schmiedeeisentexturen ersetzen die frueheren Maschinenbindungen.

Haehne besitzen zu/1/2/3, angezeigt im HUD: LMB schliesst eine Stufe, RMB oeffnet eine
Stufe; Shift+LMB baut ab. Gehaltene Gefaesse behalten ihre Fuellaktion. Legacy-Facings
bleiben lesbar; Stufen liegen im vorhandenen facing. Der gerichtete Interaktionsbefehl
benoetigt gemeinsam Protokoll 94. Stufen entnehmen 1/2/4 echte Einheiten pro Tick,
begrenzt durch Vorrat und freien Speicher. Ein klarer Fallweg bis 32 geladene Zellen
kann direkt in ein separates Becken liefern, ohne zugleich Terrainwasser zu erzeugen.
Der sichtbare Strahl endet an Beckenwasser, Naturwasser oder Hindernis und zeigt
stufenabhaengige Dicke/Geschwindigkeit. Offene Becken sammeln bei vollem Regen eine
Einheit je 30 Sekunden und offenem Quadratmeter; gespeicherte Daecher sperren auch
ungeladen. Teilfortschritt bleibt begrenzt und laufzeitlokal, der Wasserinhalt wird gespeichert.

Vor dem Hauptmenue erscheint ein eigener Ladescreen mit assetarmem Hintergrund,
tatsaechlichem Fortschritt und Fehleranzeige. Menuebuttons materialisieren erst nach
dem Gate. Das bestehende endliche Modell-/Textur-/Itemicon-/UI-/Font-Gate bleibt
erhalten; UI-Audioclips und der Status der vorhandenen Recordingbank sind einbezogen.
Es gibt weder einen zweiten Audio-Decode noch eine neue Assetregistry.

Geprueft: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -j1 -- -D warnings`,
`cargo build --workspace -j1` und
`RUST_TEST_THREADS=4 cargo test --workspace --lib --bins --no-fail-fast -j1`
(3537 bestanden, 0 Fehler, 8 bestehende Tests ignoriert).
Der native Hydrauliktest bestand in 20 Sekunden mit zwoelf Aufnahmen; er prueft
neben Hahn, Becken und Ventil auch einzelne echte Rechtsklicks mit Eimer und Kanne
an Wasserquellen sowie den anschliessenden Bodenabbau unter Wasser.
Langsamer Regeneintrag und Dachabschirmung sind durch Simulationstests geprueft.
Der native Starttest zeigt zuerst den
[Ladescreen](tools/benchmarks/results/water-controls-startup-2026-11-01/startup/startup-icons.png)
und danach das Hauptmenue: 454 Katalogeintraege und 1627 UI-/Audio-Assets sind bereit.
Beide nativen Logs enthalten keine Warnung, keinen Fehler und keine Panic.
Pruefprotokolle und Aufnahmen liegen im
[Ergebnisordner](tools/benchmarks/results/water-controls-startup-2026-11-01/).

## Phase 169: Einheitliches Beckenwasser Und Rohrgespeiste Bandbefuellung

Die separate, hellblaue StandardMaterial-Flaeche des Beckens entfaellt. Der
Beckenrenderer nutzt denselben WaterAssets-Handle, Shader, Biomtint, Tiefen- und
Wellenvertrag wie Naturwasser. Die gemeinsame endliche Pegelberechnung versorgt
auch Schwimmen, Atempruefung, Wasserbewegung und Unterwasseroptik. Bestaetigte
Beckenaggregate werden in den vorhandenen Replica-Zustand uebernommen, ohne
Transaktionsrevisionen zu erhoehen oder Terrain-FluidCells zu erzeugen.
Unveraenderte Flaechen behalten ihre Meshes. Alle vier Holzarten werden ueber
die bestehende Item-/Objekt-Holzidentitaet verarbeitet; Block169 ist im TMI
standardmaessig Spruce, die drei weiteren Varianten bleiben explizit.

Der Ladescreen enthaelt keinen Quit-Button mehr. `Laden` steht ueber dem Balken,
der echte Zaehler in dessen Mitte und der aktuelle Assetpfad mittig darunter;
lange Pfade werden Unicode-sicher in der Mitte gekuerzt. Das vorhandene
Preload-Gate bleibt fuer die Freigabe des Hauptmenues verantwortlich.
Pumpen nutzen den vorhandenen mechanischen RPM-Mix; versorgte fliessende Haehne
einen gemeinsamen kontinuierlichen Wasserloop. Entfernungsgrenzen, Stereo,
Hindernisse, Kategorien, Pause und Mute begrenzen die Wiedergabe. Der native
Hahntest prueft auch tatsaechlich dekodiertes Audio-RMS.

Hold Block170 und Fluid Infuser171 sind neue angehaengte Material-IDs mit
Workbench-Rezepten, Technic-Eintraegen, echten Kollisionshuellen und begrenzten
glTF-Modellen nach der
[Vorlage](docs/references/filling-machine-system.png). Der Hold liegt auf der
vorhandenen Bandhoehe; der Infuser sitzt eine Zelle darueber, mit seitlichen
und oberen Rohrports sowie freier Duese nach unten. Er ist ein passiver
Netzverbraucher ohne eigenen Kinetikbedarf und ohne Terrainwasserausgabe.

Der Server haelt fuellbare Drops mittig an und entnimmt echte endliche Mengen.
Volle oder ungeeignete Items passieren direkt. Trockene Versorgung, Rueckstau
und Entladen behalten die Gegenstaende; Flaschenstapel werden einzeln geteilt.
Vier Foerderrichtungen und die Luecke zur ausgehenden Bandrolle verwenden die
vorhandene Drop-Kollisions- und Besitzpruefung. Entfernen/Bewegen/Entladen setzt
den begrenzten Fuellfortschritt zurueck. Es entsteht kein zweites Iteminventar.

Giesskannen speichern Wasser jetzt in ihrer einzelnen Itemidentitaet. Refill,
Giessen, Stapelgrenzen, Weitertransport, Aufheben, UI und Speichern beachten den
tatsaechlichen Inhalt. Vorschauen teilen weiterhin das endliche Werkzeugicon;
sichtbare Drops aktualisieren nach bestaetigter Befuellung auch bei gleicher ID
ihr Modell. Alte Stapel leerer Kannen bleiben bis 128 gueltig. Handbefuellung
trennt eine Kanne atomar ab und verteilt den Rest verlustfrei; bei fehlendem
Inventarplatz wird vor dem Verbrauch abgebrochen. Die Bandstation bearbeitet
gestapelte Kannen einzeln und beendet auch mehrteilige Fuellvorgaenge vor der
Freigabe. Das Rohrsystem unterstuetzt weiterhin Wasser; der explizite
Fluessigkeits-Konvertierungsvertrag verhindert erfundene Fremdfluessigkeiten.
Die neue Drahtidentitaet benoetigt gemeinsam Client-/Server-Protokoll95.

Geprueft: `cargo fmt --all --check`, `git diff --check`,
`cargo clippy --workspace --all-targets -j1 -- -D warnings`,
`cargo build --workspace -j1` und
`RUST_TEST_THREADS=4 cargo test --workspace --lib --bins --no-fail-fast -j1`:
3561 bestanden, 0 Fehler, 8 bestehende Tests ignoriert.
Die sechs neuen Infuser-Transportfaelle pruefen echte Ein-/Ausgangsuebergaben,
alle vier Richtungen, Mengenbilanz, Flaschen-/Kannenstapel, trockene Versorgung,
Rueckstau, Aufheben und Entladen. Wiederholte Blockaktualisierung und Section-
Reload behalten die Richtung der neuen Maschinen. Kontaktgeraeusche decken
weiterhin jeden Materialtyp vollstaendig ab.

Der native Hydrauliktest bestand in 23.42 Sekunden mit 16 Aufnahmen. Er prueft
echte bestaetigte Schwimm-/Unterwasserwerte im endlichen Becken, dekodiertes
Hahn-Audio, Quellenfuellung, Unterwasserabbau sowie dieselben Drop-IDs vor und
nach der passiven Befuellung von Flasche, Eimer und Kanne. Aufnahmen zeigen den
[Fuellvorgang](tools/benchmarks/results/basins-filling-audio-2026-11-01/hydraulics-verified/infuser-filling.png)
und die
[gefuellten Gefaesse](tools/benchmarks/results/basins-filling-audio-2026-11-01/hydraulics-verified/infuser-filled.png).
Bandbewegung und genaue Mengenbilanz sind zusaetzlich durch die Transporttests
belegt; die native Szene bestaetigt ausdruecklich den Betrieb ohne Infuser-Antrieb.

Der abschliessende native Starttest bestand mit 459 Katalogicons und 1627
UI-/Audio-Eintraegen (2086 gesamt), vor Welt-/Serverstart. Er prueft auch die
tatsaechliche Textbreite und horizontale Zentrierung unter dem Balken. Der
[Ladescreen](tools/benchmarks/results/basins-filling-audio-2026-11-01/startup-verified/startup-icons.png)
zeigt eine reale Quelldatei; das Hauptmenue erscheint erst nach Freigabe.
Beide abschliessenden nativen Logs enthalten keine Warnung, keinen Fehler und
keine Panic. Protokolle und Aufnahmen liegen im
[Ergebnisordner](tools/benchmarks/results/basins-filling-audio-2026-11-01/).

## Phase 171: Ansaugung, Banduebergabe Und Startuebergang

Das Ansaugen prueft den verbundenen Wasserkoerper mit einer begrenzten Suche.
Ab 27 nassen Zellen oder an einer offenen ungeladenen Wassergrenze bleibt der
Aussenwasservorrat unveraendert. Nur nachweislich isolierte Koerper unterhalb
27 Zellen werden endlich entnommen. Dabei verlieren alle Quellen des kleinen
Koerpers gemeinsam ihre Nachfuellmarkierung; die Entnahme verteilt sich auf
die oberste nasse Schicht. Becken- und Rohrspeicher bleiben endlich. Physische
Rohranschluesse bestimmen den Ansaugmund. Bestaetigte, tatsaechlich uebertragene
Einheiten liefern die kurze Blasenaktivitaet; ein ruhendes oder gesperrtes
System erzeugt keine dauernde Ansauganimation.

Die rechteckige Vertiefung um eingetauchte Rohre entstand auch durch seitliche
Abschwaechung der externen Wasserzelle nach dem Ersetzen einer Quelle. Verbundenes
Naturwasser behaelt um Rohrkoerper jetzt seinen Umgebungspegel. Der Meshauftrag
bewahrt passende gebrochene Naturpegel trotz expliziter endlicher Zellmetadaten;
geplante trockene oder niedriger gefuellte Zellen werden nicht optisch aufgefuellt.
Neue Regressionen beschreiben echte Quellenplatzierung, vertikale Rohre,
Fluessigkeitsschritte und anschliessendes Meshing bei 7.3/8/16/-16 sowie die
Grenze 26/27, Verbindungen ueber Sections, unbekannte Grenzen und endliche Tanks.

Auch gesetzte Quellen oberhalb des prozeduralen Wasserpegels erhalten den
hoechsten tatsaechlich verbundenen Quellpegel. Eine spaeter gepruefte volle
Quelle gewinnt dabei gegen eine niedrigere natuerliche Nachbarspalte. Die
autoritative Scheduler-Regression behaelt 49 Quellen mit 392 Einheiten und
das endliche Umgebungswasser des eingetauchten Rohrs ueber 32 Schritte hinweg.
Die physische Wassersaeule vereinigt beruehrende freie Teilraeume am gleichen
X/Z, bevor sie Oberflaeche und Boden bestimmt. Echte Feststoffluecken bleiben
getrennt; der Uebergang in Nachbarzellen waehlt den jeweils verbundenen Raum.
Vier Rohrseiten teilen damit denselben Pegel, waehrend eine feste Welle weiter
zwei getrennte Wasserbereiche bildet. Mesh und Wassershader bleiben gemeinsam.

Der Hold Block nimmt ein einzelnes physisches Item auf. Durchgehende Bandauflage,
angehobene Sperren und die vorhandene Kollisionspruefung steuern Einlass, Halt,
Befuellung und Freigabe. Passende benachbarte Baender bestimmen Achse und
Laufrichtung. Ungeeignete und volle Items passieren, Rueckstau behaelt das
aufgenommene Item; es gibt keinen Teleport zum Ausgangsband und kein zweites
Iteminventar. Der Fluid Infuser sitzt direkt darueber und bezieht sein Wasser
ueber Rohre ohne eigenen Kinetikantrieb. Holz-, Eisen- und Messingmodelle folgen
der Referenz mit ausgearbeiteter Plattform, Staendern, Flansch und Duese.

Der Asset-Ladescreen verwendet keinen Zierrand. Nach erfolgreichem Preload blendet
er in 650 ms zum Hauptmenue ueber, mit gesperrter Menueeingabe waehrend des
Uebergangs. Die bestaetigten Stations- und Ansaugaktivitaeten benoetigen gemeinsam
Client-/Server-Protokoll 96.

Die abschliessende Suite
`RUST_TEST_THREADS=4 cargo test --workspace --lib --bins --no-fail-fast -j1`
bestand mit 3588 Tests, 0 Fehlern und 8 bestehenden ignorierten Tests.
Darunter liegen 965 bestandene Welt-Tests, zwoelf Infuser-Transportfaelle und
die echten autoritativen Wasser-Scheduler- sowie physische Saeulenregressionen.
`cargo build --workspace --locked -j1`,
`cargo build -p terra-client --locked -j1`,
`cargo clippy --workspace --all-targets -j1 -- -D warnings`,
`cargo fmt --all --check` und `git diff --check` sind ebenfalls bestanden.

Der abschliessende native Hydrauliklauf bestand in 43.29 Sekunden mit 18
Pruefaufnahmen: echte Handkurbel, beide angetriebenen Bandseiten, ein physisches
Item auf der Station, wartende Gefaesse, sichtbare angehobene und abgesenkte
Sperren sowie durchlaufender unveraenderter Stein. Flasche, Eimer und Kanne
erreichen gefuellt das Ausgangsband. Der getrennte 50-Zellen-Pool behaelt seine
49 Rohquellen mit 392 Einheiten und das endliche Umgebungswasser am Rohr vor,
waehrend und nach dem Pumpen. Vier freie Rohrseiten behalten Pegel 19; der
Empfaenger erhaelt Wasser, bestaetigte Ansaugung erzeugt mindestens vier reale
Blasen, die nach dem Loslassen verschwinden. Quellenklicks, Unterwasserabbau,
endliches Beckenschwimmen und dekodiertes Hahn-Audio bleiben geprueft.

Der native Starttest bestaetigt 2086 vorgeladene Eintraege vor Welt-/Serverstart,
den rahmenlosen Ladebereich und tatsaechliche Zwischen-Deckkraft von Ladeansicht
und Menue. Aufnahmen zeigen die
[Fuellstation](tools/benchmarks/results/water-station-polish-2026-11-01/hydraulics/infuser-filling.png),
die [Ansaugung](tools/benchmarks/results/water-station-polish-2026-11-01/hydraulics/pump-natural-intake-bubbles.png)
und den [Startuebergang](tools/benchmarks/results/water-station-polish-2026-11-01/startup/startup-transition.png).
Beide abschliessenden nativen Logs enthalten keine Warnung, keinen Fehler und
keine Panic. Logs und Pruefaufnahmen liegen im
[Ergebnisordner](tools/benchmarks/results/water-station-polish-2026-11-01/).


## Phase 172: Dichter Banddurchlauf, Infuser Abstand Und Ruhende Quellen

Volle und ungeeignete Items verwenden am Hold Block jetzt denselben engen
Durchlauf wie auf einem Foerderband. Sie reservieren keinen Einzelplatz und
koennen die Plattform gemeinsam passieren. Erst ein fuellbares Gefaess fordert
den Fuellplatz an: vorherige Items raeumen das Deck, danach heben die Sperren,
das Gefaess wird angehalten und mit echtem Netzinhalt befuellt. Ungeeignete
Nachfolger duerfen nach der Freigabe wieder dicht anschliessen.

Die Ausgangsband-Items werden vor den folgenden Hold-Items mit den vorhandenen
Kollisionsschritten bewegt. Markierte IDs verhindern eine zweite Bewegung im
selben Tick. Dadurch bleibt die volle Restbewegung ueber dem Ausgangsroller
bestehen; der normale Bandcursor und die allgemeine Bandphysik bleiben erhalten.
Die Durchlauftests vergleichen die tatsaechlichen Eintrittsabstaende und pruefen
zusaetzlich Mehrfachbelegung, volle Inhalte und Fuell-Exklusivitaet.

Die Anordnung lautet Hold Block auf Y, freier Luftblock auf Y+1 und Fluid Infuser
auf Y+2. Platzierung, Zielhilfe, Server, HUD und Fuellstrahl verwenden dieselben
Hoehenhelfer. Ein belegter Zwischenraum sperrt die Befuellung ohne Wasserverbrauch.
Die alte direkte Anordnung auf Y+1 fuellt nicht mehr. Der Infuser belegt nun eine
volle Zelle einschliesslich Duese und Rohrarmaturen. Holzgehaeuse, abgeschraegte
Kanten, Eisenrahmen, Messingbaender und erhoehte Nieten folgen der neuen
[Sechsseitenvorlage](docs/references/fluid-infuser-block.png). Das Modell bleibt
mit 2984 Vertices und vier verwendeten Materialien unter dem 3000er-Budget.
Die feste seitliche Kupplung der Vorlage bleibt sichtbar; zusaetzliche
Rohrfittings erscheinen nur an tatsaechlich verbundenen Seiten.

Die flache transparente Ripple-Zweitflaeche an der Ansaugung ist entfernt.
Sie lag nur vier Millimeter ueber dem Simulationspegel, waehrend der eigentliche
Wassershader Wellen bewegt; Ring, Wasser und Flansch konnten sich dadurch
ueberschneiden. Die vier runden Blasen bleiben erhalten. Ihre gesamten Koerper
halten Abstand zum Boden, zur Oberflaeche und zum echten Flanschradius. Eine
Regression prueft den realen Effekt an positiven und negativen Koordinaten
ueber 96 Animationszeitpunkte, einschliesslich des Fehlens flacher Sheet-Dreiecke.

F29 bildet in einem gestuetzten, geschlossenen 2x2-Becken aus zwei vollen
Diagonalquellen vier Quellen. Einzelne oder benachbarte Quellen vermehren ihre
Quellmarkierungen nicht. Ruhende flache Becken anderer Grundrisse werden bis
64 Innenzellen ebenfalls konservativ nivelliert. Ohne Quellen bleibt ihre
Gesamtmenge erhalten; alte Stroemung wird null und stabile Becken aendern ihre
Fluid-/Wasser-Mesh-Epochen nicht weiter. Das gemeinsame Pruefbudget betraegt
256 Zellen je Schritt. Unvollstaendige kleine Nachweise warten; nachweislich
offene, ungestuetzte oder zu grosse Bereiche bleiben im normalen Flussablauf.
Auch eine zuvor gesehene ungeladene Kante darf den Nachweis von mehr als 64
bereits geladenen Innenzellen nicht ausser Kraft setzen.

Eine kleine Pumpenentnahme entfernt weiterhin alle Quellflags des Gewaessers
atomar. Danach nivelliert F29 nur noch die verbleibende endliche Menge und
erfindet keine neuen Quellen. Boden-/Seitenflaechen, offene Raender, fallendes
Wasser, Chunkgrenzen, Epochoverlauf, Budgetende und echte Region-Speicherung
werden durch Regressionen abgedeckt. Ein geschlossener Endkanal oder Bodenpool
behaelt nun sein endliches Restwasser nach dem Schliessen des Zulaufs; freie
Wasserfallspalten und grosse offene Restwasserflaechen gehen weiterhin zurueck.
Die vorhandenen FluidCell- und Regionformate bleiben erhalten. Clients und Server
verwenden gemeinsam Protokoll 97 fuer den geaenderten Stationsaufbau.

Die abschliessende Gesamtsuite
`RUST_TEST_THREADS=4 cargo test --workspace --lib --bins --no-fail-fast -j1`
bestand mit 3609 Tests, 0 Fehlern und acht bestehenden ignorierten Tests.
Darunter liegen 981 Welt-Tests mit allen 16 Infuser-Transportfaellen, die echten
Region-Save-/Reload-Faelle und die Modell-, Anschluss- und Ansauggeometrietests.
`cargo build --workspace --locked -j1`,
`cargo build -p terra-client --locked -j1`,
`cargo clippy --workspace --all-targets -j1 -- -D warnings`,
`cargo fmt --all --check` und `git diff --check` sind ebenfalls bestanden.
Auch der separate Client-Build linkt mit den normalen Projektoptionen erfolgreich.

Der native Hydrauliklauf am abschliessenden Stand bestand in 42.62 Sekunden
mit 18 Pruefaufnahmen. Neun Items mit anfaenglich 0.26 Block Abstand bleiben
neun einzelne Items. Sechs volle oder ungeeignete Items fahren unveraendert
weiter; die Pruefung erfasst 43 Bewegungsproben und gleichzeitige Belegung des
Hold-Decks. Flasche, Eimer und Kanne werden exklusiv befuellt und erreichen das
Ausgangsband. Die Durchlaufpruefung misst Stillstand nur in bestaetigter
Drop-Simulationszeit; ausbleibende Snapshots und echte Eingabepausen zaehlen
nicht als Bewegungsausfall. Zwei Regressionen pruefen diese Uhr und eine
weiterhin erkannte echte Sperre nach Wiederaufnahme.

Der getrennte F29-Pool bildet vier Quellen mit zusammen 32 Einheiten und behaelt
ruhende Fluid-/Wasser-Mesh-Epochen. Der grosse Ansaugpool behaelt seine 49
Rohquellen und den endlichen wassergefuellten Raum um das Rohr. Die Pumpe
uebertraegt tatsaechlich Wasser, erzeugt Blasen beim Ansaugen und beendet diese
beim Loslassen. Quellenklicks, Unterwasserabbau, Beckenschwimmen und Hahn-Audio
bleiben im selben nativen Lauf geprueft. Das Log enthaelt keine Warnung, keinen
Fehler und keine Panic. Aufnahmen zeigen die
[Fuellstation](tools/benchmarks/results/dense-infuser-sources-2026-11-01/hydraulics/infuser-filling.png)
und die [Ansaugstelle](tools/benchmarks/results/dense-infuser-sources-2026-11-01/hydraulics/pump-natural-intake-bubbles.png).
Logs und Pruefaufnahmen liegen im
[Ergebnisordner](tools/benchmarks/results/dense-infuser-sources-2026-11-01/).


## Phase 173: Filterfreie Normale Tunnel, Rundes Rohrwasser Und Rezeptbuch

Normale Tunnel/Funnel akzeptieren keine Ghostfilter mehr. Container-Snapshots
trennen den Random-Schalter eines normalen Tunnels von der Filterkonfiguration;
UI, Serveraktionen und Transport verwenden dieselbe Variantenregel. Alte private
Filter normaler Ports werden beim Region-Laden verworfen, waehrend echte
gefilterte Varianten ihre Auswahl behalten. Normale Tunnel zeigen keinen
Filtermodus und keinen Ghostslot, behalten aber Random-/Ordered-Extraktion.

Ein eigener autoritativer Platzierungsray ueberspringt Quellwasser auch dann,
wenn der Ursprung bereits in einer Wasserzelle liegt. Vorschau, normale
Blockplatzierung, Orientierungen, Logistik, Baender und Schnee verwenden ihn.
Quellenfuellung und Unterwasserabbau behalten ihre jeweils eigenen Rays. Die
neuen Client-/Server-Regressionen pruefen insbesondere das tatsaechliche
Bodenziel und die Ablehnung einer Platzierung in der Augenzelle.

Glasrohr (172) ergaenzt den bestehenden Material-/Rohrvertrag ohne alte IDs zu
aendern. Vier Rohre kosten drei gerahmte Glasbloecke und einen Eisenbarren.
Das transparente Modell hat 2904 Vertices und drei benutzte Materialien;
Flansche bleiben opak. Gespeicherter Leitungsinhalt folgt dem runden Sweep
innerhalb der Bore, einschliesslich Boegen und Verzweigungen. Bei bestaetigtem
Transit zeigen dezente Innenpulse auch dann reale Bewegung, wenn der Transfer
innerhalb desselben Ticks weiterfliesst und keine gespeicherte Menge zurueckbleibt.
Ruhende leere Rohre erzeugen keine Fluessigkeitsgeometrie; gespeicherte Mengen
und reine Transitanzeige haben getrennte Pruefzaehler. Unvollstaendige Netze
loeschen veraltete Flow-/Intake-Aktivitaet ohne Mengen- oder Quellveraenderung. TMI, Werkzeuge, Ausrichtung, Glas-Audio und
Regiondaten verwenden die bestehende Registrierung. Verbundene Rohrmuender zeichnen keine
doppelten koplanaren Stirnringe; der Innenradius ihrer Flansche passt jetzt
lueckenlos zum Rohrkoerper, bei unveraenderten Aussenmassen.

Das Einsetzen eines Rohrs entquellt eine vorhandene Wasserquelle nicht mehr.
Rohrwechsel und Abbau zu Luft erhalten die aeussere FluidCell. Andere Teilbloecke
bekommen dadurch keine unendlichen Quellen, und endlicher Durchfluss bleibt
endlich. Echte kleine Pumpentnahme entfernt weiterhin alle Quellflags des
betroffenen Gewaessers und kann es vollstaendig leeren. Die bisherige Aufnahme
an jedem Rohrmund konnte bereits beim blossen Setzen acht Einheiten in den
Rohrspeicher ziehen und einen kleinen Pool entquellen. Ein Rueckwaertsnachweis
prueft jetzt vor dieser Entnahme eine gerichtete Saugroute zur laufenden Pumpe
oder einen offenen erreichbaren Verbraucher mit ausreichendem Wasserkopf.
Der geordnete Min-Head-Nachweis bearbeitet hoechstens 512 Knoten mit je sechs
Kanten. Geschlossene Ventile, stehende Pumpen und zu hohe Abnehmer sind kein
Bedarf. Gespeicherte endliche Beckenversorgung bleibt erhalten. Vier kombinierte
Fluid-/Rohr-Scheduler-Regressionen sichern besonders den vollstaendigen
12-Zellen-/96-Einheiten-Pool nach reiner Rohrplatzierung ab. Weitere Regressionen
umfassen alle vier Rohrarten und sechs Richtungen, isolierte/komplett verrohrte
Quellen, endlichen Durchfluss, Materialwechsel und Delta-/Region-Reload.

Wassermesh-Auftraege behalten neben den Kollisionsboxen eine kleine Zuordnung
der tatsaechlichen Rohranschluesse. Sichtbare Wasserflaechen werden mit dem
16eckigen Rohr-Sweep, den Flanschen und den echten Patchdreiecken des Hubs
beschnitten. Konkave Hubfacetten werden in Tetraeder zerlegt; weder eine ideale
Kugel noch eine globale konvexe Huelle entfernt zusaetzliches Wasser neben dem
Metall. An der offenen Muendung eines trockenen Rohrs wird nur der Ring
abgezogen, sodass Wasser im sichtbaren Bore erhalten bleibt. Die Schnitte
uebernehmen die bestehende Wasserflaeche und erzeugen keine versetzte Kopie.
Konvexe Clipfragmente werden nach f32-Quantisierung normalisiert; leere oder
umgekehrte Faecher an negativen Chunkgrenzen entstehen dadurch nicht.

F46 baut die allgemeine Rezeptansicht zu einem JEI-aehnlichen Rezeptbuch im
bestehenden Teramic-Stil um. Stations-Tabs, Seiten-/Alternativenwahl, zentrale
Zutaten-/Ergebnisslots, Prozesspfeile, Mengen und Materialbedarf lesen die echten
Rezeptkataloge. Die Ansicht reagiert auf kleine Fenster mit kleineren Slots und
einem getrennt scrollbaren Inhalt. R navigiert auch in Rezeptzutaten und
Ergebnisse, respektiert aber Suchfokus. Dynamische RGB-Rezepte bleiben exakt
gefaerbt. Bestehende Stationsaktionen und Plus-Transfers bleiben autoritativ;
reines Blaettern veraendert weder Inventar noch reservierte Crafting-Slots.
Seitenzaehler und Alternativen gelten pro Station; Ofen und Lagerfeuer fuer
dasselbe Ergebnis erscheinen als getrennte Tabs.
Die nativen Rezeptpruefungen verwenden die materialisierten Tilt-Buttons und
deren echte onclick-Handler. Stationssymbole stellen eine eigene begrenzte
Vorschauanfrage; sie bleiben dadurch auch ohne vollstaendiges Icon-Warmup
sichtbar. Kategorie-/Alternativenwechsel bleibt von reservierten Zutaten getrennt. Die Varianten-/Materialregeln verwenden gemeinsam
Client-/Server-Protokoll 98; Speicherformate bleiben erhalten.

Native Pruefung: Hydraulik bestand in 45,77 s mit 19 Pflichtaufnahmen,
echtem Glasrohrspeicher und Durchfluss, Unterwasserplatzierung und erhaltener
aeusserer Quelle. Alchemie/Logistik bestand in 36,88 s mit 13 Aufnahmen;
normale Tunnel blieben filterfrei und gefilterte Tunnel behielten ihren Filter.
Das gesamte Crafting-Szenario bestand mit echten OS-Mausbewegungen, Tooltips,
Stations-/Alternativenklicks, reservierten Zutaten und binaerem Save/Reload.
Ein zusaetzlicher Durchgang mit 960 x 640 logischen Pixeln bei 1,5-facher
Monitor-Skalierung pruefte die kleine, scrollbare Rezeptansicht im Fenster.
Hydraulik und Alchemie liefen ohne Warnungen oder Fehler; Crafting hatte keine
Fehler, lediglich Monitor-/doppelte Screenshot-Anfragen als Warnungen.

Die Crafting-Pruefung startet mit `TERRA_CRAFTING_SMOKE=1`,
`TERRA_INVENTORY_MODE=survival`, `TERRA_PREVIEW_WARMUP=1` und
`TERRA_NATIVE_OS_POINTER=1` sowie `--smoke-test-inventory --gender=f`.
Fuer die kleine Ansicht sind Client- und Menue-Konfiguration beide auf das
separate Fensterprofil gesetzt. Die Ergebnisse und Aufnahmen liegen unter
`tools/benchmarks/results/recipes-glass-waterlogging-2026-11-01/`.

Die nach dem IDE-Absturz vollstaendig wiederholten Workspace-Tests bestanden:
3647 erfolgreich, keine fehlgeschlagen, acht ignoriert (zwoelf Testprogramme).
Darunter sind 997 erfolgreiche Welt-Tests und alle 34 Rohrsystem-Regressionen.
`cargo clippy --workspace --all-targets -j1 -- -D warnings`,
`cargo fmt --all --check` und `git diff --check` bestanden ebenfalls.
F46 ist nach diesen Pruefungen und den nativen Durchgaengen abgeschlossen.
Auch `cargo build --workspace --locked -j1` bestand mit dem endgueltigen Stand
fuer Client und Server. Beide verwenden Protokoll 98 und muessen gemeinsam
neu gestartet werden. Die vollstaendigen Prueflogs und Quellhashes sind in
`verification.json` und `logs/` neben den Aufnahmen hinterlegt.

## Phase 174: Absperrhahn-Achsen, Endlicher Auslauf Und Mildere Sonne

Die normale serverautoritative Hydraulikplatzierung verwendet den vollstaendigen
Ausrichtungsplan. Wand-, Boden- und Deckenplatzierung liefern damit dieselben
Rohr-/Ventilachsen wie die Vorschau. Die Ventilrotation folgt explizit derselben
Basis wie die Kollisionsform, einschliesslich umgekehrter horizontaler Richtung.
Die bestehende binaere Ventilbedienung bleibt erhalten.

Bereits gespeicherter passiver Leitungsinhalt ist eine endliche Druckquelle,
wenn ein offener Abnehmer mit ausreichender Hoehe erreichbar ist. Nach dem
Schliessen der Zufuhr laeuft die Abnehmerseite weiter aus. Mengen werden durch
echte Transfers bewegt; gesperrte Leitungen und Speicher ohne erreichbaren
Verbraucher werden nicht geloescht.
Hahn und Infuser sind dabei echte Terminals: Ihr Verbraucherpuffer foerdert
nicht rueckwaerts durch den Einlass. Passive gespeicherte Mengen folgen einem
streng abnehmenden Paar aus erforderlichem Wasserkopf und Entfernung zum
Abnehmer. So verhindern auch leere Nebenarme einen geordneten Auslauf nicht.
Der bestehende Bedarfsnachweis bleibt auf 512 Knoten begrenzt; gerichtete
Pumpensaugwege und aktive Druckquellen behalten ihren bisherigen Vertrag.

Die hydraulische Wassermenge wird im echten runden Profil horizontal beschnitten;
eine obere freie Wasserflaeche schliesst Teilfuellungen. Der Renderer verwendet
fuer Glasrohre, Pumpeninhalt und Wasserstrahlen denselben WaterAssets-Handle und
Biomfarbvertrag wie See und Becken. Rohrvolumina liefern ihre eigene optische
Dicke, bleiben geometrisch im Rohr und erzeugen keinen Uferschaum oder
Spielerwellen. Bestaetigte Stroemung wird im gemeinsamen Wassershader animiert;
aufbewahrte Menge und reiner Transit bleiben getrennt. Ansaugblaeschen behalten
ihren eigenen Effektbatch.

Die gemeinsame Sonnenkonstante fuer Setup und laufenden Tageszyklus betraegt
65000 statt 100000 Lux. Direkte Highlights sinken um 35 Prozent (~0,62 EV),
ohne Kamerabelichtung, ACES, Texturen, UI, Mond-, Ambient- oder Fackellicht
umzustellen. Gezielte Tageszyklusregressionen erhalten Nacht- und Hoehlenwerte.

Pruefung: Die Gesamtsuite bestand mit 3660 erfolgreichen Tests, keinem Fehler
und acht ignorierten Tests in zwoelf Programmen. Darunter sind 1003 Welt- und
980 Render-Tests sowie die serverautoritative Platzierung an allen sechs
Flachennormalen. Clippy mit `--workspace --all-targets -j1 -- -D warnings`,
Formatierung und Diffpruefung bestanden. Die Geometrie prueft alle runden
Anschlussmasken mit acht Mengen, die vollstaendige obere Flaechenbilanz und
sichtbare Anschlusskanten bei unterschiedlichen Nachbarpegeln.

Der native Tageslichtvergleich bestand vorher und nachher mit jeweils 31
Ansichten bei identischer Konfiguration, einschliesslich Nacht, Unterwasser,
Schattenuebergaengen und TAA/MSAA. Die gemeinsame gruene Pixelmaske in der
unteren Bildhaelfte zeigt in der Mittagsaufnahme rund 20 Prozent weniger
Bildhelligkeit im oberen Dezil; sie umfasst Gras und Laub und ist keine
physikalische Luxmessung. Die mittlere naechtliche Bildhelligkeit bleibt
praktisch gleich. Aufnahmen, Konfiguration, Quellhashes, Vergleichsskript und
vollstaendige Logs liegen unter
`tools/benchmarks/results/valve-daylight-2026-11-02/`.

Der abschliessende Workspace-Build fuer Client und Server bestand ebenfalls.
Die erweiterte native Hydraulikpruefung bestand in 62,98 s mit 24
Pflichtaufnahmen: beide Ventile wurden durch regulaere Bauaktionen gesetzt,
Teilfuellungen sichtbar und alle 32 gespeicherten Einheiten nach Schliessen
in den Receiver uebertragen; abgesperrter Quellvorrat und Gesamtmenge blieben
erhalten. Die bisherige Pruefung nach dem Schliessen bilanziert nun den
bereits vorhandenen endlichen Puffer einschliesslich exakt befuellter Gefaesse,
statt ein sofort leeres Rohr vorauszusetzen. Alle bisherigen Hydraulik-,
Unterwasserbau-, F29-, Pumpen- und Infuserpruefungen blieben erhalten.
Ein vorangehender Spieltest wurde waehrend paralleler Kompilierung durch einen
Input-ACK-Timeout unterbrochen. Der unveraenderte Wiederholungslauf ohne
Compilerlast bestand ohne Warnungen oder Fehler; Timeouts wurden nicht erhoeht.

## Phase 175: Offene Ventile Ohne Pegelpumpen Und Verschwindenden Transit

Der Fehler hatte zwei Ursachen. In einer angehobenen Saugleitung konnte ein
lokaler Stored-Druck mit groesserem Kopf die weiterhin erreichbare Wasserquelle
verdraengen. Der Druckbaum und die Transfers konnten gespeichertes Wasser dann
wieder in tiefere Ansaugrohre zurueckschieben. Reale Natural-/Basin-/Pump-Reichweite
hat jetzt Vorrang vor einem reinen Restwasserseed. Ein Stored-Seed innerhalb
aktiver Saugwege folgt in Druckausbreitung und Transfer ausschliesslich dem
bereits bestaetigten Nachfolger zur laufenden Pumpe. Die passive gerichtete
Restwasserentleerung bleibt erhalten.

Die bisherige Flussanzeige leitete ihre Aktivitaet aus dem horizontalen
FluidCell-Flussvektor ab. Senkrechter Transport hatte deshalb den Wert null;
ein im selben Tick leer konsumierter Puffer verlor ebenfalls seine Anzeige.
Ein eigenes fluechtiges Transferflag bestaetigt nun echte Aufnahme, Bewegung
und Entnahme unabhaengig von der verbleibenden Menge. Ein Gefaess-/Terminaldraw
markiert nur die tatsaechliche Strecke vom entnommenen Puffer zum Verbraucher,
nicht unbeteiligte weiter oben liegende Speicher. Der naechste Hydraulikschritt
setzt die Aktivitaet zurueck; unvollstaendige Netze loeschen sie ebenfalls.
Der bestehende zweidimensionale Richtungsvektor bleibt unverfaelscht.

Das Flag wird nicht gespeichert und veraendert das bestehende binaere
Wasserformat nicht. Client-Snapshots verwenden weiterhin das vorhandene
flowing-Feld. Leere Transitpuffer zeichnen einen schmalen Strom durch den
vollstaendigen runden Verlauf statt eines erfundenen unteren Achtel-Pegels.
Gespeicherter Fuellstand, gemessene Menge und reine Transitgeometrie bleiben
getrennt. Der begrenzte Renderstatus weist pro Rohr die verarbeiteten
Snapshotfolgen und realen Wasservertices aus.

Gezielte Vergleiche setzen gleichzeitig horizontale und vertikale offene
Ventile anstelle gerader Rohre ein. Mengen, Quellbedarf, Pumpenguthaben,
Transferaktivitaet und Arbeitszaehler muessen pro Tick identisch sein. Hinzu
kommen das angehobene Ansaug-Repro, unveraenderte Mengenbilanz, ein leer
konsumierter realer Puffer und das sofortige Ende veralteter Transitaktivitaet.
Die native Pumpenpruefung beobachtet alle drei Glasrohre ueber mehrere Sekunden
und unterschiedliche tatsaechlich gerenderte Snapshots; stehendes Wasser bei
Rueckstau ist weiterhin sichtbar, jedes Rohr muss auch eigenen echten
Transport zeigen. Die bisherigen Druck-, Stop- und endlichen Auslaufpruefungen
bleiben erhalten.

Pruefung: `cargo check --workspace --all-targets -j1`, der gemeinsame
Client-/Server-Build, Clippy mit `--workspace --all-targets -j1 -- -D warnings`,
Formatierung und Diffpruefung bestanden. Die Gesamtsuite bestand mit 3667
erfolgreichen Tests, keinem Fehler und acht ignorierten Tests in zwoelf
Programmen, darunter 1006 Welt-Tests. Der Postcard-Vergleich bestaetigt, dass
das fluechtige Transferflag keine gespeicherten Wasserbytes hinzufuegt.

Die native Hydraulikpruefung bestand in 66,21 s mit 24 Pflichtaufnahmen ohne
Warnungen oder Fehler. Alle drei Glasrohre behielten ueber 3,64 s und zehn
unterschiedliche, jeweils bereits gerenderte Snapshots eigene sichtbare
Wassergeometrie; alle drei meldeten in allen zehn Samples echte Transfers.
Receiverzuwachs, Pumpe, Druckaufbau, korrektes Stoppen und der vollstaendige
endliche 32-Einheiten-Auslauf wurden ebenfalls bestaetigt. Die strengere
Zeitpruefung entdeckte eine alte unbeabsichtigte Verbindung des Receivers zu
zwei Schau-Rohren; diese Testmodelle stehen jetzt getrennt. Der Receiver wird
vor dem Test auf unerwuenschte Rohrnachbarn geprueft. Die Mengenassertion blieb
streng. Das endliche A/B-Quellbecken liegt vollstaendig auf seiner tatsaechlich
angeschlossenen Entnahmeebene und muss alle 40 Anfangseinheiten einschliesslich
Leitungspuffer in den Empfaenger liefern.

Referenzbild, Aufnahmen, Quellhashes und vollstaendige Logs liegen unter
`tools/benchmarks/results/valve-flow-2026-11-02/`.

## Phase 176: Wandfackeln, Gebundene Rohrtexturen Und Nahtlose Wasserwellen

Der Wandfackelfuss ragte wegen eines gerundeten Montageversatzes geringfuegig
in seine Stuetzwand. Die echte Clearance-Pruefung wies dadurch insbesondere
Welten mit erweiterten Kinetikformen ab. Der gemeinsame Core-Transform setzt
den Fuss jetzt vor die Wandflaeche; es gibt keine pauschale Kollisionsausnahme.
world::torches vereinheitlicht Normale, Facing und Support fuer Vorschau,
Platzierungsplan und Server. Vier Wandseiten und Boden bleiben gueltig;
Decken-/Teilblockmontage wird nach den bestehenden Regeln abgewiesen. Ein
ungeeigneter Supportersatz loest die Fackel genau einmal samt Drop.

Das pro Quad nach dominanter Normale gewaehlte UV-Achsenpaar sprang in Boegen.
Die neuen Zylinderkoordinaten folgen dem Sweep, einschliesslich passender
Tangenten und Handedness der Innenflaechen. Hub und Stirnringe verwenden feste
Charts. Kleine Messingringe lagen zuvor innerhalb der eigentlichen Bogenregion;
sie sitzen nun axial bei .20..25 beziehungsweise .75..80 auf den geraden Armen.
Die vier Rohrassets erhalten dazu Zylinder-UVs und Tangenten. Der eigentliche
Rohrkoerper und sein Rund-/Bogenprofil bleiben geometrisch unveraendert.

Ventile und Pumpen hatten bisher keinen sichtbaren Wasserclip-Vertrag und
wurden ueber ihre groben Kollisionsboxen maskiert. Separate sichtbare Profile
bilden jetzt Rohr-/Flanschachtecke, Ventilkoerper und Handradbohrung sowie das
Pumpengehaeuse ab. Physik- und Entnahmevolumen bleiben getrennt erhalten.
Die kleinen Rundrohr-Stuetzringclips stimmen mit den regenerierten Assets ueberein.

Fuer das Flackern wurden zwei voneinander unabhaengige bewegte Randspalte
nachgewiesen. Ein Rohr eine Zelle hinter einer X/Z-Chunkgrenze erzeugte nur im
einen Worker virtuelle Natur-Uebergangszellen. Diese zaehlten faelschlich als
Fluidveraenderung und pinnten dieselbe gemeinsame Ecke einmal auf null und
beim Nachbarn auf eins. Das Pinning unterscheidet jetzt diese exakten
Natur-Fringe-Zellen von echten Fluidveraenderungen. Zweitens erhielt dieselbe
bewegte Ecke zuvor verschiedene Stroemungsdaten aus den angrenzenden Zellen;
der Shader verwendete deshalb unterschiedliche Wellenamplituden. Bewegte
natuerliche Gitterecken verwenden nun den aus denselben vier SurfaceColumns
ermittelten gemeinsamen Flow. Der Shadervertrag bleibt erhalten, echte
Rohrkontakte bleiben gepinnt und Fernwellen werden nicht abgeschaltet.

Regressionen decken echte Survival-Fackelrequests und Itemverbrauch, negative
Sectiongrenzen, Montagekollision, Supportersatz/Drop, Sweep-UVs und Tangenten,
Stuetzringlage, sichtbare Valve-/Pump-Profile sowie echte getrennte Worker an
X/Z-Grenzen plus und minus 16 ab. Der erweiterte native Lauf setzt vier
Wandfackeln ueber gewoehnliche Bauklicks und prueft alle Rohrmaterialien,
Glasinnenwaende aus zwei Richtungen und die Wasseroberflaeche aus zwei Ansichten.

Pruefung: Workspace-Build, Clippy fuer alle Targets mit `-D warnings`,
Formatierung und Diffpruefung bestanden. Die Gesamtsuite bestand mit 3686
erfolgreichen Tests, keinem Fehler und acht ignorierten Tests in zwoelf
Programmen, darunter 1016 Welt-Tests.

Der native Hydrauliklauf bestand in 74,95 s mit 35 Pflichtaufnahmen. Alle vier
Wandfackeln wurden mit normalen Rechtsklicks gesetzt und verbrauchten jeweils
genau ein echtes Item. Alle vier Rohrmaterialien, Glasinnenwaende und beide
Wasseransichten bestanden. Die vorhandenen Hydraulikpruefungen einschliesslich
kontinuierlichem Glasrohrtransport und endlichem Ventilauslauf bleiben erhalten.
Ein zusaetzlicher Naturwasserlauf bestand mit 33 Aufnahmen; sechs Teile stehen
im echten Oasengewaesser mit erhaltenen Quellen. Die CPU-Geometrie weist 928
bewegte und 72 gepinnte Kontaktvertices aus. Beide Nahansichten verwenden
fortschreitende Zeit und gegenueberliegende Kameras.

Der native Testaufbau prueft entfernte Galerierohre anhand tatsaechlich geladener
Positionen und nach dem Wasserupload die dauerhaften Mesh-Entities. Wasserproben
liegen nachweislich ausserhalb der legitimen Kollisionskoerper. Eine Bevy-Warnung
zu einem bereits entfernten RenderLayers-Empfaenger waehrend eines Szenenwechsels
ist im Hydrauliklog festgehalten; alle Assertions und Aufnahmen wurden beendet.
Referenz, Aufnahmen, Konfigurationen, Quellhashes und Logs liegen unter
`tools/benchmarks/results/torch-pipe-surfaces-2026-11-02/`.

## Phase 177: Runde Rohrdurchfuehrungen, Infuser-Strahl Und Teilblockwasser

Normale Rohre liessen bei Wand-/Dachdurchfuehrungen eine ganze Voxelzelle offen.
Der bestehende Ofenrohr-Capture-/Compose-Pfad ergaenzt jetzt eine passende
Umfassung auch fuer Holz, Stein, Eisen und Glas. Material und Textur kommen
von den tatsaechlichen vollen Nachbarflaechen. Runde 16-Eck-Portloecher,
Rohr-Sweep-Innenwaende und gemeinsame Flaechenkullung schliessen Boegen und
Abzweigungen an. Messingbaender behalten ihren eigenen Platz; freie Rohrnetze
erzeugen keine neue Terrainfuellung.

Der Infuser-Strahl bestand nur aus sechs getrennten kleinen Tropfen. Er ist
jetzt ein geschlossener runder Wasserkorper zwischen dem echten Auslass und
dem tatsaechlichen gehaltenen Einzelitem. Ein eigener Effektmarker im
gemeinsamen Wassermaterial verwendet reale Strahldicke, bewegte Normalen und
dezente Glanzlichter. Wasserfarbe und Umgebungsbeleuchtung bleiben gemeinsam;
es gibt keine leuchtende Ersatzfarbe. Die bisherige lokale Pufferpruefung
konnte trotz bestaetigtem Fuellvorgang Wasser aus vorgeschalteten Rohren
uebersehen. Sichtbarkeit folgt nun der autoritativen Filling-Phase samt
Supply-, Item- und Traegerpruefung. Ohne Versorgung und bei nicht fuellbaren
Items bleibt der Strahl aus.

Zaunformen fehlten im Partial-Vertrag und wurden als trockene volle Wuerfel
behandelt. Die vorhandenen Shape-Metadaten und der gemeinsame Collision-Visitor
beschreiben jetzt den freien Aussenraum; Zaunarme folgen ihren echten Nachbarn.
Eigene Basin-/Kesselreservoirs bleiben getrennt. Ein begrenzter, geometrisch
gepruefter Quellpfad gleicht den Pegel verbundener Konstruktionshohlraeume an,
ohne neue Sourceflags zu erfinden. Normale freie Ausbreitung behaelt ihre
Daempfung. Ein endliches niedriges Slabbecken wird gegen seinen tatsaechlichen
Pegel auf Dichtheit geprueft statt faelschlich gegen eine volle Wasserzelle.
Eine vom Eimer gesetzte volle Quelle oberhalb einer halbhohen Slabwand darf
weiterhin ueber deren Oberkante laufen.

Der Mesh-Sampler unterscheidet den logischen Wasserkopf von der geklemmten
Speicherhoehe gedeckter Taschen. Ein vollstaendig nasser geschlossener Pocket
senkt keinen benachbarten freien Spiegel mehr ab; ein hoeherer verbundener Kopf
wird nur uebernommen, wenn dadurch exakt kein zusaetzlicher Hohlraum nass wird.
Trockene obere Luecken bleiben trocken. Vollstaendig untergetauchte Teilmodelle
erhalten keine inneren vertikalen Voxelwasserwaende. Der Wasserhahn hat ebenfalls
ein sichtbares Rundprofil mit seinen tatsaechlichen Ringspalten.

Pruefung: Workspace-Build, Clippy fuer alle Targets mit `-D warnings`,
Formatierung und Diffpruefung bestanden. Die Gesamtsuite bestand mit 3712
erfolgreichen Tests, keinem Fehler und acht ignorierten Tests in zwoelf
Programmen; darunter 1031 Welt- und 994 Renderer-Tests.

Der native Hydrauliklauf bestand mit 35 Pflichtaufnahmen in 76,04 s. Der aktive
Infuser zeichnete einen bestaetigten durchgehenden Strahl mit 794 Vertices;
der geschlossene Infuser blieb ohne Strahl. Die vorhandenen Pumpen-, Eimer-,
Kannen-, Tor-, Quellen-, Wandfackel- und endlichen Auslaufpruefungen bestanden.
Die neue optionale Partial-Water-Szene bestand mit fuenf Aufnahmen in 11,66 s:
16 endliche Einheiten bleiben im niedrigen Slabbecken, verbundene Treppen-/
Zaunluecken erreichen den Quellpegel, Pfosten bleiben fest und sieben
untergetauchte Teilblockposen bewahren ihre Quellen. Alle vier Rohrmaterialien
schliessen Wand- und Dachdurchfuehrungen. Der bisherige Wasserfalllauf bestand
ebenfalls mit zwei Aufnahmen und messbarem Rueckgang nach Quellverschluss.

Die Gesamtsuite entdeckte einen unerwuenschten Sourcehead-Bypass ueber den
winzigen Deckspalt abgesenkter Terrainpfade. Diese behalten ihre normale
seitliche Daempfung; echtes fallendes Wasser darf den Freiraum weiterhin
fuellen. Ein alter Drain-Test war allseitig unterhalb einer Slabkante
eingeschlossen: Seine bisherige Trockenannahme widersprach dem nun korrekten
Restwasser. Er besitzt jetzt einen echten Abfluss; die strengen Trocken- und
Zeitpruefungen bleiben. Eine eigene Mengen-/Reloadregression sichert die
geschlossene Resttasche ab.

Eine bereits bekannte Bevy-Warnung zu einem entfernten RenderLayers-Empfaenger
trat beim Szenenwechsel im Hydrauliklauf auf; alle Assertions und Aufnahmen
bestanden. Referenzen, Aufnahmen, Quellhashes, Konfigurationen und Logs liegen
unter `tools/benchmarks/results/partial-water-pipe-surrounds-2026-11-02/`.

## Phase 178: Geschlossene Wasserraender Und Wassererhalt Beim Rohrsetzen

Die bisherige Ueberlaufannahme entsprach nicht dem gewuenschten Verhalten.
Treppen, Slabs und Zaeune dienen nun als Beckenrand: ihre freien Zwischenraeume
koennen Wasser aufnehmen, geben es aber seitlich nicht an den Aussenraum weiter.
Die Quellpfadpruefung verwendet dieselbe Flussrichtung. Ein vollstaendig aus
Konstruktionsbloecken bestehender geschlossener Rand begrenzt den Fuellpegel
auf seine niedrigste feste Oberkante; eine volle Eimerquelle legt dadurch keine
Wasserflaeche ueber einen halbhohen Slabring. Endliche geschlossene Becken
behalten ihre Mengenregeln.

Beim echten Block-Commit werden auch implizite Naturwasserzellen vor dem
Rohrsetzen aus der vorbereiteten Weltspalte gesichert und als Fluidzustand in
die Replica-Deltas aufgenommen. Wassergefuellete Rohre erhalten keine aus
Nachbarwaenden abgeleitete Terrainumfassung: diese konnte bisher den freien
Wasserraum wie einen ganzen Block verdecken. Trockene Wanddurchfuehrungen
behalten ihre runde Umfassung.

Die Spielpruefung verwendet jetzt einen frei stehenden Zaunring ohne zusaetzliche
Steinwand und kontrolliert explizit dessen trockene Aussenseiten. Neue
Regressionen pruefen mehrere Randmaterialien, alle vier normalen Rohrarten und
den Wassererhalt beim echten Commit samt Replica-Import.

Die native Pruefung fand beim Absenken mehrerer bereits voller Slabquellen
einen bisherigen Unsigned-Unterlauf in der Mengenbuchhaltung. Zugefuegte und
entfernte Mengen werden nun getrennt verbucht; eine eigene Regression prueft
jeden Schritt mit exakter Bilanz. Der abschliessende Workspace-Lauf besteht
3716 Tests ohne Fehler (8 ignoriert); Build, Clippy mit `-D warnings`, Format-
und Diffpruefung bestehen ebenfalls.

Beide nativen Laeufe bestehen: sechs Partial-Water-Aufnahmen (12,83 s) und
35 Hydraulik-Pflichtaufnahmen (74,75 s), einschliesslich echtem Unterwasserbau,
Quellerhalt beim Rohrsetzen, kontinuierlichem Glasrohrfluss und Infuserbefuellung.
Beim Hydraulik-Szenenwechsel erschien einmal die bereits bekannte Bevy-Warnung
zum RenderLayers-Eintrag an einer entfernten Entity; keine Assertion schlug fehl.
Referenzen, Bilder, Logs und Quellhashes liegen unter
`tools/benchmarks/results/closed-banks-waterlogged-pipes-2026-11-02/`.

## Phase 179: Nasse Zaunseite, Teilblock-Wassererhalt Und Ueberflutete Fackeln

Die Zaunreferenz verlangt eine Grenze innerhalb der Zaunzelle. Verbundene
Streben trennen nun vier horizontale Bereiche um den Pfosten. Eine begrenzte
Suche durch diese Bereiche bestimmt die vom vorhandenen Wasser erreichbare
Seite, auch an Ecken und ueber Section-Grenzen. Physik, Punktabfragen und
Wassermesh benutzen dieselben Bereiche. Die trockene Seite begrenzt Wasser,
verdeckt aber keine sichtbare Wasserflaeche in einer Strebenoeffnung. Bereits
untergetaucht gesetzte Zaeune behalten ihr vorhandenes Quellwasser rundum.

Alle wasserfuellbaren Teilbloecke behalten beim echten Commit ihr vorhandenes
Naturwasser und echte Quellflags; endliches Wasser wird dabei nicht zur Quelle.
Auch beim direkten Journal-Wechsel auf einen anderen Teilblock bleibt die
vorhandene Quelle erhalten; ein Vollblock entfernt sie.
Der bisherige Drain-Test sperrt seinen Zufluss deshalb mit einem echten
Vollblock. Ein Rohr genau auf einer ganzzahligen Wasseroberflaeche darf seine
trockene Zelle nicht mit Terrain auffuellen und so das Wasser darunter
verdecken: beruehrende Naturwasserzellen werden beim Commit mitgesichert und
die Terrainumfassung beruecksichtigt diesen Kontakt.

Die Punktabfrage behaelt bei einer erhaltenen Quelle unter einer horizontalen
Welle die Oberkante des tatsaechlich verbundenen freien Intervalls. Ohne
prozedurale Wassersaeule darf sie nicht auf die gesamte gespeicherte
Zellhoehe zurueckfallen. Der bestehende strenge Test fuer getrennte obere und
untere Wasserraeume bleibt unveraendert.

Fackeln werden im Wasser sowohl in der Platzierungspruefung als auch beim
Commit abgelehnt. Erreicht Wasser eine bereits gesetzte Fackel, baut die
Autoritaet sie ueber den normalen Editpfad ab und erzeugt genau einen Drop.
Wasser, Posebereinigung und Replica-Deltas bleiben dabei konsistent. Leere
Kontaktlisten loesen keine staendigen Journal-Aenderungen aus.

Die native Partial-Water-Szene prueft die innere Zaunseite einschliesslich
Ecken, wassergefuellte Teilmodelle ohne nachtraegliches Einsetzen ihres Wassers,
Rohre auf der Wasseroberflaeche und die Fackelregeln. Sie erhaelt eine
Zaun-Draufsicht und eine Ansicht wassergefuellter Wandmodelle.

Der abschliessende Workspace-Lauf besteht 3721 Tests ohne Fehler (8 ignoriert,
12 Testprogramme). Der unveraenderte Test fuer getrennte Wasserintervalle an
der horizontalen Welle besteht ebenfalls. Build, Clippy fuer alle Targets mit
`-D warnings`, Format- und Diffpruefung bestehen. Die beiden abschliessenden
nativen Laeufe bestehen mit acht Partial-Water-Aufnahmen (14,80 s) und
35 Hydraulik-Pflichtaufnahmen (74,87 s), ohne Warnungen oder Fehler im Log.
Die Bildkontrolle bestaetigt die trockene aeussere Zaunhaelfte und den freien
Wasserraum um Rohre an Beckenwaenden. Fackel-Wasserplatzierung wird abgelehnt;
die nachtraeglich ueberflutete Fackel erzeugt genau einen Drop bei erhaltenem
Wasser. Historische Vorschauaufnahmen sind getrennt markiert. Referenzen,
Aufnahmen, Logs, Konfigurationen und Quellhashes stehen unter
`tools/benchmarks/results/fence-inner-water-partial-blocks-torches-2026-11-02/`.


## 180. Kupferinventar, Crafting Recipes Und Wasser An Rohren

Player Inventory und Crafting Recipe folgen den beiden neuen Kupfer-/Braunvorlagen:
transparenter Nine-Slice-Rahmen, Titelplakette, goldene Auswahl, Avatar zwischen
zehn Ausruestungsplaetzen, 2x2-Crafting und Rezeptbuch. 36 Rucksack- und neun
Hotbarplaetze sind echte Inventarplaetze. Die Rezeptansicht zeigt Stationsreiter,
Zutatenraster, Ergebnis, Itembeschreibung und alternative Rezepte mit Miniaturen.
Der gemeinsame Itembrowser hat Suche, Filter und Seitenauswahl. Vorschau-Icons
werden auch fuer Alternativrezepte angefordert, der neue Rahmen vorab geladen.
Reines Nachschlagen erzeugt im Creative-Modus keine Items.

Die Ursache des noch sichtbaren Wasserkastens wurde in einer isolierten Kopie
der gespeicherten Testwelt reproduziert: Der Terrainrenderer rekonstruierte
Voxelkontaktflaechen von Fluid Water Block neben Rohrmodellen, zusaetzlich zum
korrekten Fluidmesh. Diese Rekonstruktion ueberspringt jetzt Wasser. Ein Test mit
allen vier echten Rohrmodellen deckt Innenzellen und alle sechs Sectiongrenzen ab.
Dekoratives Solid Water erhaelt beim Rohrsetzen seine Wassermenge; interne
Kontaktflaechen an vollstaendig nassen Modellen werden gezielt aktualisiert.
Vorher-/Nachher-Aufnahmen verwenden dieselbe Unterwasserkamera. Der originale
Spielstand wurde fuer diese Pruefung nicht geoeffnet oder veraendert.

Alte Inventardateien bleiben lesbar. Begrenzte Tuple-Serde erweitert Reservierung,
Drag und kombinierte Containeransichten auf 45 beziehungsweise 81 Plaetze.
Netzwerkprotokoll 99 setzt passende Client-/Serverversionen voraus.
Referenzen, echte native Aufnahmen und abschliessende Pruefprotokolle:
[Phase-180-Bericht](tools/benchmarks/results/copper-inventory-underwater-pipes-2026-11-02/README.md).

Abschluss: 3732 Tests bestanden, keine fehlgeschlagen, acht ignoriert;
Workspace-Build, Clippy mit `-D warnings`, Format und Diff-Pruefung erfolgreich.
57 native Fixture-Aufnahmen sowie der gespeicherte Vorher-/Nachher-Vergleich.
Inventar und Rezeptbuch sind in 1680 x 936 und 1025 x 641 Pixeln mit echten
Betriebssystem-Klicks geprueft. Der Blur auf dem Layoutroot wurde entfernt, weil
er beim Verkleinern seine alte Breite festhielt und den Browser beschnitt.

## 181. Kompaktes Inventar, Modales Rezeptbuch Und Ofenreferenz

Inventar- und Rezeptansichten haben keine X-Schaltflaechen mehr. E und ESC
schliessen zuerst das Rezept-Popup, dann das erhaltene Inventar. Das Buch am
Craftingbereich klappt ausschliesslich den Itembrowser ein und aus. Kompaktere,
zentrierte Kupferrahmen, vertikale Kategorie-Tabs links am Itembrowser und ein
Filter-Reset; die Seitenzahl steht mittig zwischen den Rezeptpfeilen.

Rezeptansichten liegen als eigene Ebene ueber dem abgedunkelten Inventar.
Der Hintergrund bleibt erhalten und nimmt keine Zeigeraktionen an. Die dritte
Vorlage ist als eigene Ofenansicht umgesetzt: Input, ausgewaehlter Brennstoff,
vorhandenes Ofenmodell, Ergebnis, Dauer und Schmelzalternativen. Alternative
Brennstoffe kommen aus der vorhandenen Registry; ihre Auswahl aendert nur die
Vorschau. Tickangaben folgen dem konfigurierten Servertakt. Holz-Wildcards tragen
nun die ausdrueckliche Angabe „Any wood planks“ mit allen vier erlaubten Arten;
insbesondere ist die Truhe nicht auf die gerade abgebildete Holzart beschraenkt.

[Referenzen, native Aufnahmen und Pruefung](tools/benchmarks/results/inventory-modal-furnace-2026-11-02/README.md).

Abschluss: 3735 Tests bestanden, keine fehlgeschlagen, acht ignoriert; letzter
vollstaendiger Lauf mit vier Testthreads. Workspace-Build, Clippy fuer alle
Targets mit `-D warnings`, Format und Diff-Pruefung erfolgreich. 13 native
Aufnahmen in 1680 x 936 und 1025 x 641 Pixeln mit realen Betriebssystem-Klicks
und Tastendruecken, geprueften Ofen-Inhaltsgrenzen und unveraendertem autoritativem
Inventar. Der erste Netzwerk-Timeout und die erfolgreichen Wiederholungen sind
im Bericht dokumentiert. R ueber dem Katalogitem oeffnet dessen Rezept-Popup.

## 182. Quadratische Slots, Pagination Und Weitere Referenz-UIs

Spielerinventar passt die Breite an die Hoehe an; Rucksack und Inventar-Hotbar
verwenden quadratische Zellen, auch bei eingeklapptem Itembrowser. Pagination
steht rechts und stoppt an den Enden. Explizite Grid-Tracks runden physische
Pixel stabil, Kapazitaetsaenderungen erhalten den Item-Offset und die Suche
normalisiert den internen Seitenindex. Rezept-Popups enthalten keinen Itembrowser.

Holz-Wildcards zeigen Oak, Spruce, Birch und Cherry gleichzeitig als waehlbare
Vorschauen. Vorschauwechsel aendern weder Reservierungen noch Inventarinhalt.
Kesselrezepte besitzen eine eigene Referenzansicht mit Zutaten/Wasser,
wiederverwendbarem Ruehrwerkzeug, Kesselmodell, Ergebnis und echten Alternativen.
Der Balken folgt ausschliesslich der autoritativen Kesselarbeit, inklusive
Fortschrittsaenderungen ohne neue Inventarrevision.

Hotbar mit Kupfer-/Blatt-/Kristallrahmen, neun echten Item-Slots, Mengen und
Nummernplaketten. Startmenue mit gemalter Landschaft, freigestelltem TERAMIC-Logo
und einzeln bedienbaren Buttons. Vier neue PNG-Assets per Builtin-Imagegen aus
der Bildreferenz erstellt und vor dem Menue vorgeladen; Pixelbilder behalten ihr
Original-Alpha. Transparente Randbereiche werden nur beim Rendern geclippt.

[Aufnahmen, Prompts und Pruefbericht](tools/benchmarks/results/interface-pagination-square-slots-2026-11-02/README.md).

Abschluss: 3740 Tests bestanden, keine fehlgeschlagen, acht ignoriert.
Workspace-Build, Clippy fuer alle Targets mit `-D warnings`, Format und
Diff-Pruefung erfolgreich. 24 native Aufnahmen in 1680 x 936 und 1025 x 641
Pixeln; echte Betriebssystem-Eingaben, quadratische Slots, rechtsbuendige
Seitennavigation, vier interaktive Holzvorschauen und unveraendertes Inventar.

## 183. Inventar Mit 27 Plaetzen Und Weitere Kupfer-Menues

Spieler besitzen 18 Rucksackplaetze und neun Hotbarplaetze. Character und
Crafting nutzen die frei gewordene Hoehe. Der Itembrowser verwendet dieselbe
quadratische Zellgroesse wie der Rucksack und leitet seine Seitenkapazitaet aus
passenden Zeilen und Spalten ab. Die Hotbar zeigt den ganzen 2172 Pixel breiten
Rahmen mit beiden Endornamenten und ohne zusaetzliche dunkle Hintergrundplatte.

Singleplayer besitzt einen eigenen Hoverzustand. Pause verwendet das Logo und
die Buttons des Hauptmenues. Settings und Singleplayer verwenden Kupferrahmen,
Titelplaketten und passende Bedienfelder nach den neuen Vorlagen. Settings hat
linke Kategorie-Tabs; die Weltliste zeigt echte Manifest-Versionen/Spielmodi.
Vorhandene Kategorien, Einstellungen und Weltaktionen bleiben erreichbar.

Protokoll 100 verwendet 27 Inventareintraege. Alte gespeicherte Inventare mit
bis zu 45 Eintraegen bleiben lesbar. Beim Beitritt werden ueberzaehlige Items
zuerst einsortiert und der Rest am Beitrittsort abgelegt; fehlende Drop-Kapazitaet
bricht die Uebernahme atomar ab. Speicher-, Erhaltungs- und Netzwerkregressionen
decken die neue Groesse und die Uebernahme ab.

[Vorlagen, Aufnahmen und Pruefung](tools/benchmarks/results/interface-menus-27-slots-2026-11-02/README.md).

Abschluss: 3743 Tests bestanden, keine fehlgeschlagen, acht ignoriert.
Workspace-Build, Clippy fuer alle Targets mit `-D warnings`, Format und
Diff-Pruefung erfolgreich. 34 native Aufnahmen in 1680 x 936 und 1025 x 641
Pixeln, echte Betriebssystem-Eingaben und unveraendertes Inventar. Pause ist
auf Zentrierung und Button-Grenzen geprueft; Screenshot-Aufnahmen erfolgen
vor dem jeweils naechsten direkten Ansichtswechsel.

## 184. Slotdetails, Holzalternativen Und Kesselreferenz

Crafting-Zellen und Inventarzellen haben dieselben Masse. JEI-Zellen sind
25 Prozent kleiner, Kategorien rund 20 Prozent groesser und die Tonne sitzt
unten in der Leiste links neben JEI. Suchtext besitzt linken und vertikalen
Innenabstand. Spieler behalten 18 Rucksack- und neun Hotbarplaetze.

Holzrezepte zeigen Oak, Spruce, Birch und Cherry als eigene Alternativkarten
mit Grid, Name und Menge. Karten und Seitenpfeile wechseln nur die Vorschau;
Registry, Reservierungen und Items bleiben unveraendert. Kesselrezepte zeigen
die neue freigestellte Referenzgrafik, groessere Zutaten und Ergebnisse sowie
echte Rezeptdaten und autoritativen Fortschritt. Das PNG wurde mit dem
eingebauten Imagegen erzeugt, mit Original-Alpha gespeichert und vorgeladen.

Start- und Welt-Ladeansichten sowie Create-/Delete-Popups verwenden den
Kupferstil. Der Startscreen bleibt rahmenlos und blendet ins Hauptmenue ueber.
Menueaktionen sitzen mit mehr Innenabstand oberhalb der Rahmenleisten.

Abschluss: 3744 Tests bestanden, keine fehlgeschlagen, acht ignoriert.
Workspace-Build, Clippy mit `-D warnings`, Format und Diff-Pruefung erfolgreich.
40 native Aufnahmen inklusive echtem Preload/Fade; Welt-Ladebilder verwenden
repraesentative View-Daten. Keine bestehenden Spielstaende veraendert.
[Vorlagen, Aufnahmen, Grafik-Prompt und Pruefung](tools/benchmarks/results/copper-ui-details-cauldron-2026-11-03/README.md).

## 185. Geraeteoberflaechen, Rezeptvorlagen Und F1

Ofen, Brecher, Mischer, Kessel, Lagerfeuer, Truhen, verbundene Werkbaenke
und Logistikfilter verwenden gemeinsame Kupferrahmen und quadratische Slots.
Schmale Fenster stapeln die Arbeitsbereiche; Listen bleiben scrollbar.
Auch Blockinformation, Chat, Karten-/Wegpunktdialoge, Minimap-Bedienelemente,
Multiplayer und verbleibende Einstellungen folgen den warmen Materialien.

F1 zeigt rahmenlose Textspalten mit echten Leistungs-, Streaming-, Positions-,
Welt-, Wetter-, Netzwerk- und Zieldaten. Fehlende optionale Messwerte stehen
als `--`; Minimap und Blockkarte ueberdecken die Diagnose nicht.

Crafting- und Ofenrezepte folgen den neuen Vorlagen mit grosser Prozessansicht,
Metadaten und echten Alternativen. Rezept-Popups bleiben ohne Itembrowser
und X-Schaltflaeche. Die neue Ofenillustration wurde mit dem eingebauten
Imagegen erzeugt, mit Original-Alpha gespeichert und ins Preload aufgenommen.

[Vorlagen, native Aufnahmen, Grafik-Prompt und Pruefung](tools/benchmarks/results/all-surfaces-recipes-debug-2026-11-03/README.md).

Pruefung: 3745 Workspace-Tests bestanden, keine fehlgeschlagen, acht ignoriert.
Nach der Korrektur bestaetigter Weltkoordinaten bestehen alle 448 UI-Tests.
Workspace-Build, Clippy aller Targets mit `-D warnings`, Format-, Diff- und
vier CSS-Parserpruefungen erfolgreich. 72 native Aufnahmen inklusive echter
E-/F1-Tasten und 768-Pixel-Rezeptdokumente. Geraete verwenden validierte
temporaere Client-Projektionen; vorhandene Spielstaende bleiben unberuehrt.

## 186. Controls, Textabstaende Und Hover

Verbleibende Dropdown-/Scrollbar- und Checkboxfarben vereinheitlicht.
Hotkey-Glyphen in HUD und Inventar mit Flexlayout zentriert. Gezieltes Padding
fuer Beschreibungen, Auswahlwerte und Kartenanzeigen; Weltkartenflaeche und
Bedienelemente liegen weiter innerhalb der Kupferleisten. Menue-Hover als
dezente Aufhellung ohne zusaetzliche gelbe Kontur.
[Pruefbericht](tools/benchmarks/results/controls-padding-hover-2026-11-03/README.md).

## 187. Einheitliche Inventare Und Wetter-Spaltenabfragen

Wetter-/Schnee-Edits lesen implizites Wasser ueber PreparedColumn::surface_at
statt ueber einen festen 18er-Stride. Regressionen decken beide Spaltenformate,
alle 256 Spalten sowie positive/negative Abschnittskoordinaten und die
Wasser-/Replica-Erhaltung ab. Die gemeldeten Array-Panics werden damit an
ihrer Ursache behoben.

Gemeinsame Slotgroesse fuer Spieler, Truhen, Werkbank, Geraete und Katalog.
Verbundene Truhen liegen im inneren Scrollbereich; Rahmen und Katalog bleiben
auch kompakt fest stehen. Rezeptlose Anfragen oeffnen keine leeren Popups.
Wegpunktfelder zeigen zwei Nachkommastellen mit Komma und erhalten unveraendert
gespeicherte Praezision. Passendere Settings-Icons, um 25 Pixel hoehere Welt-/
Serverkarten und transparente Rahmen-Hosts mit eingerueckten Innenflaechen.

3748 Tests bestanden, keine fehlgeschlagen, acht ignoriert. Build, Clippy,
Format, Diff und CSS-Parser erfolgreich. 25 Inventar-/Rezeptaufnahmen bestanden;
19 weitere Geraeteaufnahmen. Weitere Live-Bildpruefungen wurden durch
Fensterschliessen beziehungsweise OS-Mausinterferenz unterbrochen und werden
nicht als vollstaendig bestanden dargestellt.
[Pruefbericht und Aufnahmen](tools/benchmarks/results/inventory-frame-weather-2026-11-03/README.md).

## 188. Feste Slots Und Passende Inventar-Layouts

Alle Inventarzellen und der Itembrowser verwenden feste 28 x 28 logische
Pixel, Ergebnisfelder 32 x 32. Die Rahmen passen zum Inhalt; Blockinventare
zeigen oben den Arbeitsbereich und darunter das Spielerinventar ohne
Inhalts-Scrollbar. Angeschlossene Werkbanktruhen stehen links in einem eigenen
Rahmen. Die Tonne sitzt immer links unten am Itembrowser. HUD-Rahmen und
Hotbarslots sind aufeinander abgestimmt.

FPS 0 wird als Unlimited ohne Current-Eintrag angezeigt. Die Aufloesung bleibt
bei Borderless auswaehlbar; eine feste Auswahl schaltet den Entwurf auf
Windowed. Dropdowns verwenden einen Kupfer-Chevron.

3549 Library-Tests bestanden, keine fehlgeschlagen, sieben ignoriert. Build,
Clippy, Format, Diff und drei CSS-Parserpruefungen erfolgreich. 26 native
Layoutaufnahmen pruefen die exakten Zellengroessen und Positionen in zwei
Fenstergroessen ohne OS-Maus-/Tastatureingaben.
[Pruefbericht und Aufnahmen](tools/benchmarks/results/fixed-inventory-layout-2026-11-03/README.md).

## 189. Aufloesungsabhaengige Inventargroesse

Inventar und HUD-Hotbar wachsen mit der Aufloesung. Rahmen, Slots, Text und
Abstaende skalieren gemeinsam; die logischen Grundmasse bleiben einheitlich.
Bei 100 Prozent UI-Massstab sind normale Inventarslots in Full HD etwa
56 Pixel gross, bei 1440p etwa 74 Pixel. Die gesamte Kombination mit einer
angeschlossenen Truhe wird eingepasst; die Hotbar bleibt darunter sichtbar.
DPI und UI-Massstab werden nicht doppelt angewendet.

Tooltips beruecksichtigen die gezeichnete Groesse unter lokalem Zoom.
Native Pruefungen messen transformierte Grenzen, sichtbare Mindestgroessen,
passende Klickflaechen und den Abstand zur Hotbar.
3552 Library-Tests und 26 native Ansichten bestanden; Build, Clippy,
Format und Diff erfolgreich.
[Pruefbericht und Aufnahmen](tools/benchmarks/results/responsive-inventory-2026-11-03/README.md).

## 190. Kleinere Beschriftungen Und Bedienicons

Ueberschriften, Labels, Mengenangaben und Bedienicons im Inventar sind
kleiner, waehrend die aufloesungsabhaengigen Slot- und Rahmengroessen erhalten
bleiben. Die redundanten Zahlen 1 bis 9 sind aus den Inventarslots entfernt.
Die HUD-Hotbar behaelt ihre Tastenhinweise mit kleinerer Schrift.

CSS-Parser und Stylesheet-Regression, Build, Clippy aller Targets, Format und
Diff erfolgreich. 26 native Ansichten bestaetigen die passenden Layouts,
unveraenderte Slotgroessen und fehlende Hotkeylabels im Inventar.
[Pruefbericht und Aufnahmen](tools/benchmarks/results/inventory-typography-2026-11-03/README.md).

## 191. Inventaraufteilung Und 4K-Dichte

HUD-Hotbar 30 Prozent kleiner, Inventarzellen 20 Prozent groesser;
Player-/Crafting-Bereiche 20 Prozent niedriger. Angepasste Rahmenbreiten
und quadratische Ausruestungsfelder. Texte und Bedienicons wachsen bei
1440p/4K langsamer; Eingaben behalten Text, Cursor und Auswahl in einem
gemeinsamen Koordinatensystem und bleiben links verankert.

Unveraenderte volle Wuerfelkollisionen bleiben bei Nachbarinvalidierungen
nutzbar. Lokale Edits und angrenzende Modellformen erzwingen weiterhin
einen Neuaufbau. Regressionen pruefen Bereitschaft und Jobzahl sowie den
bestehenden UDP-Sprungschutz. Der konkrete gemeldete Teleport ist noch
nicht reproduziert; Blockart, Modus und Schritte sind angefragt.

[Pruefbericht und Aufnahmen](tools/benchmarks/results/inventory-density-collision-2026-11-04/README.md).

## 192. Reproduzierbare Framezeitmatrix Und Dichte Section-Imports

O07 umgesetzt: Endliche integrierte Vorher-/Nachher-Messungen fuer neue Chunks,
Flug, Doerfer, echten Regen, schnelle autoritative Platzierungen und das
offene Inventar bei gleicher Sichtweite und Grafikqualitaet. Der Runner
friert Konfigurationen ein, isoliert Welten/Profile und lehnt unvollstaendige
Szenarien ab. CPU-Wall-Zeiten, GPU-Passdiagnostik, Speicherwerte sowie
Queue-/Uploadzaehler sind getrennt dokumentiert.

Dichte Section-Deltas verwenden indizierte Validierung und bauen ihre
Objekt-/Hold-Indizes einmal nach Installation der finalen Metadaten auf.
Normale Edits, doppelte Blockeintraege und Fehlerteilzustaende behalten ihre
geprueften Regeln. Der CPU-Mikrobenchmark fuer 4096 Objektposen sinkt im
Mittel von 518,02 auf 9,35 ms. Native Kalenderuniforms bleiben kontinuierlich,
invalidieren aber keine unbeteiligten HTML-Bindings mehr: Das offene Inventar
steigt im gemessenen Paar von 78,62 auf 105,17 FPS, UI-Reaktivitaet sinkt von
4,28 auf 1,10 ms. Vorhandene Biomvorschaubilder sind im Preload registriert.

Flug-p99 sinkt von 18,00 auf 16,54 ms, Platzierungs-p99 von 15,37 auf
15,08 ms. Regen verbessert sich nicht (48,12 auf 46,66 FPS); RSS ist in
diesen kurzen Laeufen hoeher. Keine allgemeine GPU-, Speicher- oder
Mindest-FPS-Zusage. Der separat bestaetigte erste Inventar-Oeffnungsframe
von rund 101 ms bleibt als O08 offen; A05 behaelt weitere Hardware und
Langzeitlast. O07 ist aus der offenen Liste entfernt.

3804 Workspace-Library-/Bin-Tests bestanden (Client-Nachlauf eingeschlossen),
neun ignoriert; ein Exampletest und acht Python-Tests bestanden. Build,
Clippy aller Targets mit `-D warnings`, gezieltes Format und Diff erfolgreich.
[Messbericht, Reproduktion und Vergleichsdaten](tools/benchmarks/results/o07-frame-spikes-2026-11-05/README.md).

## 193. Vorgewaermte Inventarslots Und Autoritative Containerprojektion

Der erste Player-Oeffnungsframe aus O08 entstand durch 63 Slot-Style-Owner,
deren Klassen erst beim Oeffnen auf das sichtbare Layout wechselten. Slots
verwenden jetzt bereits beim versteckten Vorwaermen ihre spaeteren Klassen;
auch die vier Handwerksfelder und ihr Ergebnis werden vorbereitet.
Handwerksaktionen bleiben an ein wirklich offenes Inventar gebunden.
Der gemessene erste Player-Oeffnungsframe sinkt von 116,46 auf 25,19 ms;
eine strengere Pruefung zweier Oeffnungszyklen misst jeweils rund 27 ms.

Blockinventare bleiben waehrend ausstehender Antworten geparkt. Der Restore
wartet zusaetzlich, bis die getaktete Ansicht tatsaechlich zum autoritativen
Anker und zur logischen Containeransicht passt. Das verhindert den belegten
Zwischenaufbau eines Player-Inventars nach dem ACK. Die native Identitaet
wird nicht in HTML serialisiert. Wiederholtes Truhenoeffnen sinkt im
gemessenen Vergleich von 107,34 auf 39,96 ms bei erhaltenem Truhengrid.

Echte TiltUI-Regressionen pruefen Klassen-/Change-Tick-Stabilitaet,
Vorwaermen ohne Aktionen, ACK vor View-Tick, fehlgeschlagene Anfragen,
asynchrones Schliessen und Rueckkehr zum Player-Inventar. Zwei echte
Truhenzyklen pruefen Server, Snapshot, Slots, Eingabesperre und versteckte
Tooltips; 25 native Ansichten pruefen echte OS-Klicks, Fokus, Rezepte,
Ofen/Kessel, Pause und Einstellungen. Das Pointer-Pruefwerkzeug erhaelt
ein zur Entfernung passendes, begrenztes Bewegungsbudget fuer mehrere Monitore.

3813 Library-/Bin-Tests einschliesslich abschliessender UI-/Client-Nachlaeufe
bestanden, neun ignoriert. Acht Python-Tests, Build, Clippy aller Targets,
Format und Diff erfolgreich. Die temporaere externe TiltUI-Diagnosequelle
ist bytegenau wiederhergestellt.

O08 ist abgeschlossen. Der getrennt gemessene erstmalige Aufbau neuer
Blocklayouts bleibt mit etwa 93 bis 144 ms als O09 offen; keine allgemeine
Mindest-FPS-Zusage. [Messbericht und native Nachweise](tools/benchmarks/results/o08-inventory-opening-2026-11-05/README.md).

## 194. Bergfried-Kingdoms, Burgfamilie Und CSS-Regelindex

F43 umgesetzt: Ein platzierter Bergfried oeffnet einen Dialog fuer Gruendung,
Umbenennung, Mitglieder und bestaetigtes Aufloesen. Der Server prueft UUID,
Baurecht fuer Gruendung, Mitgliedschaft, Gruenderrechte, Reichweite und freien
Blick. Beitreten und Austreten sind moeglich; der Gruender muss aufloesen.
ID, Name, Gruender-/Mitgliederidentitaeten samt Anzeigenamen, Standort und Zustand
werden gespeichert und an alle Clients einschliesslich spaeterer Beitritte
uebertragen. IDX8 bewahrt die bisherigen Indexmigrationen; Protokoll 102.
Aktive Bergfriede sind gegen Abbau geschuetzt. Aufloesen behaelt Historie und
gibt Namen, Mitgliedschaften und Bergfried frei. Eigene RPC-Nonces verhindern
Einfluss auf Blockedits; exakte Antwort-IDs verhindern veraltete Dialogantworten.

Zwanzig Burgbloecke mit IDs 173 bis 192 folgen der Bildvorlage: Bergfried,
Stein-/Zinnenmauern, Mauerkrone, Ecken/Saeulen, Bogenhaelften, Schiessscharte,
Fenster sowie rissige, bewachsene, moosige, verstaerkte und weitere Stile.
Eigene glTF-Modelle, gemeinsame physische Formen, Inventarvorschauen, passende
Oberflaechengeraeusche und 21 Rezepte. Vier Stein ergeben eine Steinmauer,
vier Steinmauern eine Mauerkrone. Der Bogen besteht aus zwei einzeln
platzierbaren 1 x 1 x 0,5-Teilen fuer zwei Bloecke Laenge; alle 24
Treppenorientierungen, Schraubenschluessel, Picking und Section-Replikation
erhalten die wirkliche Oeffnung.

O09 umgesetzt: Ein unveraenderlicher Index der Dokument-CSS-Regeln wird je
Stylesheet wiederverwendet. ID-/Klassen-/Tag-Kandidaten werden konservativ
vorgefiltert; der vollstaendige Servo-Abgleich und die Kaskade bleiben erhalten.
Der externe TiltUI-Stand ist angewendet und als nachvollziehbarer Patch
im Projekt gesichert. Keine reduzierte Grafikqualitaet oder geaenderte Slots.
Im kontrollierten Paar mit identischen finalen Spielquellen sinkt die kalte
Truhen-Oeffnungsspitze von 94,53 auf 54,84 ms, das behaltene Layout beim erneuten
Oeffnen von 22,45 auf 19,69 ms. `main.update` im jeweils kalten Spitzenframe
sinkt von 80,08 auf 40,72 ms. Reaktivitaet und PostUpdate bleiben etwa gleich.
Die verbleibenden 54,84 ms sind weiterhin ein messbarer kalter Frame; keine
Behauptung eines ruckelfreien ersten Bildes, einer GPU-Present-Latenz oder
allgemeiner FPS-/Speichergewinne.

3836 Workspace-Library-/Bin-Tests bestanden, neun ignoriert; 331 TiltUI-Tests
und die gesonderte reale CSS-Differentialfixture bestanden. 19 Python-Tests,
Clippy aller Targets mit `-D warnings`, Builds, Format und Diff erfolgreich.
Echte Mehrspieler-/Save-/Reload-Pruefungen einschliesslich eines 2.763.543 Byte
grossen maximalen Kingdom-Snapshots; 60 native Ansichten fuer Burgmodelle,
Kingdom-Lebenszyklus, Inventarbedienung und zwoelf Stationslayouts in zwei
Groessen. O09 und F43 aus der offenen Liste entfernt; Claims/Diplomatie bleiben
F55/F56/F57, weitere Hardware und Langzeitlast bleiben A05.

[Pruefbericht, Modelle und Aufnahmen](tools/benchmarks/results/o09-kingdom-castle-2026-11-06/README.md),
[O09-Messdaten und TiltUI-Patch](tools/benchmarks/results/o09-kingdom-castle-2026-11-06/O09.md).

## 195. Chunk-Claims, Diplomatie Und Burgdetails

F55/F56 fertiggestellt: autoritative Spieler-/Bot-Claims mit getrenntem
Kingdom-Besitz, verbundenen Gebieten, Reichweite, Kontingenten und sicherer
Freigabe. Geschuetztes Bauland und offenes Territorium; Persistenz ueber
IDX9 mit IDX8-Migration und Replikation ueber Protokoll 103.

Diplomatie in der grossen Karte zeigt farbige Claims, Grenzen, Legende,
Gruender und ausfuehrenden Akteur. Live-Aenderungen, Auswahl und bestehende
Kartenebenen bleiben erhalten. Seitenleistenklicks sind gegen Durchgreifen
auf die Karte abgeschirmt; kleine Fenster erhalten eine passende Werkzeugleiste.

Mauerkrone mit passendem Mauerwerk bis zu den Koepfen; feine Risse,
unregelmaessiger Bewuchs und stabile Dekorvarianten. Durchgehende
Bogenunterseiten mit passenden UVs/Tangenten und verdeckten inneren Naehten.
F55/F56 sind aus TODO entfernt, Rollen und weitere Beziehungen bleiben F57.

[Tests, native Ansichten und Referenzen](tools/benchmarks/results/claims-diplomacy-castle-2026-11-06/README.md).

## 196. Kingdom-UI, Flächen-Claims, Transparente Fenster Und Wappen-Capes

Kingdom-Menü mit passenden Text-/Buttongrößen, sauber abschließendem Rahmen,
gestaltetem Colorpicker-Popup und vollständiger durchsuchbarer Symbolauswahl.
Stabile Symbolkennungen und gemeinsame Banner-/Cape-Glyphen.

Diplomatie erhält gemischte rechteckige Build-/Territorium-Auswahlen mit Vorschau,
Apply und Cancel. Der Server validiert die gesamte Auswahl atomar; unveränderte
Claims behalten ihren Akteur. Höhenrichtige Relief-Auswahl und Loslass-Endpunkte;
Protokoll 104 für Client und Server.

Durchsichtige Mauerfenster mit dem Glas normaler Fenster; aufrechte Seitenmontage
und passende Ausrichtung beim Verlängern vorhandener Burgteile. Neue TMI-/Crafting-
Capes erhalten das eigene Wappen und seine Farbe als dauerhaftes Itemmerkmal.

Lutera/Betross neutral, Helteria freundlich, Petora feindlich. Stabile zufällige
Dorfzugehörigkeit ohne Retrogen; Eintrittsanzeige und Karteninformationen.
Die drei neuen Strukturvorlagen fehlen als Bilder und bleiben offen unter F72;
frei verwaltbare Beziehungen und Rollen bleiben F57.

[Prüfbericht und native Ansichten](tools/benchmarks/results/kingdom-polish-batchclaims-2026-11-06/README.md).

Abnahme 196: 3.674 Workspace-Library-Tests bestanden, acht ignoriert; Build,
Clippy aller Targets mit `-D warnings`, Format, Diff und 60 CSS-Stylesheets geprüft.
24 native Ansichten, darunter QHD-/4K-/1024x768-Menüs mit echten Serverantworten,
Colorpicker, Symbolsuche, gezogene Claim-Vorschau und transparente Mauerfenster.

## 197. Reichsgebiete, Dorfwappen Und Bewohnte Kingdom-Strukturen

Die Diplomatiekarte zeigt die Claims aller Spielerreiche und die Gebiete bereits
entdeckter NPC-Siedlungen in ihren Reichsfarben. Bekannte Siedlungsgebiete bleiben
neben den Kartendaten gespeichert, auch wenn ihre Chunks entladen werden. Direkter
Chunk-Besitz hat Vorrang; ansonsten übernimmt eine Siedlung den Besitzer ihres
Zentrums oder ihre ursprüngliche Bot-Zugehörigkeit. Karte, Eintrittsanzeige,
Fassadenbanner, Dachflaggen und Soldaten-Wappenröcke verwenden dieselbe Auflösung.
Änderungen an Farben, Wappen und Besitz werden sichtbar aktualisiert.

Rechtsklick entfernt Chunks aus der unbestätigten Auswahl, gehalten auch entlang
einer gezogenen Spur. Bestehende Claims werden dadurch nicht freigegeben.
Die hellen Einzelumrandungen entfallen; die Auswahl bleibt als Farbfläche sichtbar.

F72 umgesetzt: Die drei nun vorhandenen Bildreferenzen bilden zwei begehbare
Kingdom-Varianten und einen Außenposten. Runde Stadtmauer, innere Burg, vierstöckiger
Bergfried, Kirche, Wohnhäuser, Kasernen, Markt, Schmiede, Felder und Windmühle;
die Wintervariante besitzt gestaffelte Höfe und schneebedeckte rote Dächer.
Der Außenposten erhält Palisade, Wachturm, Tor, Mannschaftshaus, Zelte und Stall.
Echte Türen, Fenster, Treppen, Betten, Kisten und Arbeitsstationen sowie geprüfte
Zugänge verbinden die Anlagen mit den vorhandenen Spielsystemen.

Kingdoms beginnen mit 50–80 Zivilisten und 26–64 Soldaten; Außenposten mit
4–10 Soldaten. Schwert-, Speer- und Bogenschützen besitzen eigene Ausrüstung,
Wachschichten und Patrouillen. Die Startbereiche sind keine laufenden
Bevölkerungsgrenzen. Erweiterte Bewohnerkennungen erhalten alte IDs; tote Bewohner
und Militärrollen überstehen Speichern, Streaming und spätere Netzwerkbeitritte.

Neue Welten verwenden Strukturrevision 6; bestehende Welten behalten ihre
gespeicherte Generierung. Dorfwappen und Karteninformationen funktionieren auch
mit vorhandenen Dörfern. IDX9/PLY7 bleiben erhalten, Client und Server verwenden
Protokoll 105. F72 ist aus der offenen Liste entfernt; Rechte und frei verwaltbare
Beziehungen bleiben F57.

[Prüfbericht, Referenzen und native Ansichten](tools/benchmarks/results/kingdom-structures-realms-2026-11-06/README.md).

Abnahme 197: 3.697 Workspace-Library-Tests bestanden, acht ignoriert. Geprüft
sind unter anderem Maximalbelegung mit 144 Bewohnern, reale Bettplätze,
Militärrollen über UDP und Save/Reload, begehbare Bergfriedgeschosse und
Toranschlüsse an wirkliches Gelände sowie Kartenfarben und Rechtsklick-Auswahl.
Workspace-Build, Clippy aller Targets mit `-D warnings`, Format, Diff und alle
60 UI-Stylesheets erfolgreich geprüft. Elf native Aufnahmen dokumentieren die
drei Baupläne und fünf Kartenansichten.

## 198. Hangfelder, Begrünte Fundamente Und Scharfe Biomvorschauen

Strukturrevision 7 erhält geländeabhängige Felder und bis zu 24 Blöcke breite
begrünte Übergänge zwischen Höfen und natürlichem Boden. Außenfelder übernehmen
die wirklichen Spaltenhöhen; Bewässerung bleibt in eingefassten lokalen Mulden.
Das bisherige rechteckige Ausschneiden des Hangs und senkrechte Fundamentkanten
entfallen. Die Änderungen bleiben innerhalb der vorhandenen regionalen Suche und
Strukturbudgets. Gespeicherte ältere Revisionen behalten ihre Generierung.

Turmtüren sitzen zwischen den tatsächlichen Mauerpfosten. Der Bergfried hat eine
zur Doppeltür passende Öffnung mit geschlossenen Seiten und Sturz. Die vorhandene
Blattachse passt bereits zur Wand. NPCs schließen selbst geöffnete Türen nach
freier Passage; beide Hälften, Spieler-/Tierabstand und entladene Chunks werden
berücksichtigt. Manuell bediente Türen behalten ihren Zustand.

Beim Wechsel aus einem Reich ins unbeanspruchte Gebiet erscheint die neutrale
Eintrittsanzeige. Neutrale Nachbarchunks starten sie nicht ständig neu;
unabhängige Dörfer erhalten eine eigene neutrale Anzeige.

Kingdoms und Outposts sind in World Create unabhängig von Dörfern abschaltbar.
Master-Schalter, Speicherung, Dedicated-Neustart und Welcome-Replikation erfassen
alle fünf Strukturarten. Alte Einstellungen ohne die neuen Felder behalten ihre
bisherige Dorf-Kopplung. Protokoll 106; IDX9/PLY7 bleiben erhalten.

Acht echte Biomaufnahmen ersetzen die 122×59-Pixel-Ausschnitte durch
1536×768-Pixel-Vorschauen. Eigene Filterung für Fotos, unverzerrte Proportionen
und eine eigene Felsenwüsten-Aufnahme. Die Exportpipeline dokumentiert native
Quellen und Prüfsummen und lehnt Hochskalieren ab.

Sieben alte Teramic-Testverzeichnisse mit 10,37 GB Dateiinhalten wurden aus dem
temporären RAM-Dateisystem auf die Datenplatte verlagert. Alte Pfade bleiben
über Verknüpfungen erreichbar; Größen und Änderungszeiten wurden abgeglichen.
Die Swap-Belegung sank dabei zunächst von ungefähr 24,6 auf 13,4 GiB.
Nach erfolgreicher Systemanmeldung wurde der Swap vollständig zurückgesetzt;
das Protokoll zeigt 0 B Belegung und die wiederhergestellte Priorität 100.

3.918 Workspace-Library-/Bin-Tests bestanden, neun ignoriert. Neue Prüfungen
decken natürliche Feldhöhen, Böschungen und Budgets, Türrahmen und Geschosse,
gepaartes Schließen nach Streaming, neutrale Gebietseintritte und unabhängige
Struktur-Schalter über Speicherung und Netzwerk ab.
Build, Clippy aller Targets mit `-D warnings`, Format, Diff und 60 UI-Stylesheets
erfolgreich. Drei native Bau-Szenen, acht Biomquellen und Menüprüfungen in Full HD
und 4K mit tatsächlich geladenen Bildern und gespeicherten Weltoptionen bestanden.

[Prüfbericht, Aufnahmen und Speicherprotokoll](tools/benchmarks/results/kingdom-terrain-worldcreate-2026-11-06/README.md).

## 199. Kingdom-Rechte, Doppeltüren Und Laternen

Strukturrevision 8 begrünt Kingdoms und Außenposten mit echten Hofbäumen und
erntbaren Bodenpflanzen. Gebäude, Türen und Wege bleiben frei; Winterhöfe
erhalten passende Nadelbäume. Die neue Verteilung ist seltener und hält die
Startsuche mindestens 1.024 Blöcke von Kingdoms und 512 Blöcke von Outposts
entfernt. Koordinaten und Seed bestimmen Abstände und Priorität unabhängig von
Ladereihenfolge und Spielerposition. Bestehende Strukturrevisionen bleiben
gespeichert erhalten; deaktivierte Arten reservieren keinen Startbereich.

F57 ergänzt Founder, Officer, Member, Builder, Worker und Visitor sowie
servergeprüfte Rechte für Bauen, Abbauen, Inventare und NPC-Arbeit. Neue Mitglieder
beginnen als Visitor; frühere normale Mitglieder werden beim Laden zu Member.
Rollen- und Beziehungsverwaltung, Kartenplaketten und Gebietstitel unterscheiden
eigenes, neutrales, verbündetes und feindliches Gebiet. Spielerbündnisse erfordern
beidseitige Zustimmung; Bot-Reiche reagieren gemäß ihrer Haltung und Reichsstufe.
Besitzerfarben und Wappen bleiben erhalten.

Rechte gelten auch für offene Lager, entfernte Mitglieder, laufende NPC-Befehle,
entladene Halterungen und begonnene Baumarbeiten. Förderbänder und Rohrnetze
prüfen jede tatsächlich überschrittene Reichsgrenze. Offenes Realm Territory
behält seine Bau-/Abbau-Freigabe; geschützte Inventare lassen sich dort nicht durch
Blockabbau ausschütten. Eigene Gegenstände können weiterhin weggeworfen werden.
Produktive Berufe und neue Auftragsverwaltung bleiben separat in F58.

Benachbarte passende Türen bilden aus ihren vorhandenen Objektpaaren eine
Doppeltür: gemeinsamer Rahmen, äußere Scharniere, gegensinnig öffnende Flügel.
Bedienung und Rechte prüfen alle vier Zellen vor der Änderung; Kollision,
Auswahl, Wasser und NPC-Durchgänge verwenden dieselbe Geometrie. Abbau entfernt
nur den getroffenen Flügel. Alle vier Holzarten und Ausrichtungen sind erfasst;
zusätzliche gespeicherte Türdaten sind nicht erforderlich.

Laterne und Kette erhalten eigene Voxelmodelle, Platzierung, Rezepte, Itemkatalog,
Auswahlflächen und echte warme Beleuchtung. Laternen stehen auf Blöcken und
Zaunpfosten, hängen an Decken/Ketten und verwenden an Wänden eine Halterung.
Neue Dörfer, Outposts und Kingdoms ersetzen ihre Fackeln durch Laternen.
Wasser, fehlende Halterungen, Streaming und Drops folgen den vorhandenen Regeln.

Protokoll 107 und Index ID10 übertragen beziehungsweise speichern Rollen und
Beziehungen. Alte Indizes werden ausdrücklich migriert; PLY7 bleibt erhalten.

[Prüfbericht und native Ansichten](tools/benchmarks/results/kingdom-rights-lanterns-2026-11-06/README.md).

Abnahme 199: 3.963 Workspace-Library-/Bin-Tests bestanden, neun ignoriert.
Build, Clippy aller Targets mit `-D warnings`, Format, Diff und 60 tatsächliche
UI-Stylesheets sind geprüft. Native Galerien kontrollieren Laternenlicht und
Befestigungen, alle vier Holzarten der Doppeltür, Kartenbeziehungen sowie Rollen,
Bündnisse und Besucheransichten über echte Netzwerkbestätigungen in QHD,
kompakter Auflösung und 4K. F57 ist aus der offenen Liste entfernt; F58 bleibt offen.

## 200. Setzlinge, Apfelbäume Und Verbundene Zinnen

Bot-Reiche erlauben Zugriffe nach Haltung: feindlich keine Bau-, Abbau- oder
Interaktionsaktionen, neutral nur Türen und Fenster, freundlich vollständige
Zugriffe. Die Grundhaltung gilt ohne eigenes Spielerreich; erklärte Feindschaft
und angenommene Bündnisse wirken unmittelbar. Spielerclaims und übernommene
Siedlungen behalten ihre Mitgliederrollen. Villagerhandel prüft Inventarrechte.

F59 ergänzt Oak-, Spruce-, Birch-, Cherry- und Apple-Setzlinge als native
Miniaturbaum-Props. Boden, Licht, Abstand, freie Zellen, Wasser und Rechte bestimmen
Platzierung und Wachstum. Verbaute oder entladene Bereiche pausieren; bestehende
Spielerblöcke und Props werden erhalten. Arten, Fortschritt und genau zugeordnete
Holz-/Laubzellen überstehen Speichern, Streaming und Netzwerkbeitritte. Ersetzte
Spielerzellen verlieren ihre Baumzuordnung. Auch natürliche Bäume liefern passende
Setzlinge anhand ihrer tatsächlichen Generatorart.
Historische Fichten und Kirschbäume mit Oak-Holz verwenden ihre tatsächliche
Generatorart für Blattdrops und vollständige Fällungen.

F60 ergänzt natürliche und gepflanzte Apfelbäume mit breiter Krone, eigenem Laub,
Blüten, unreifen/reifen Früchten, Ernte und erneutem Fruchtansatz. Acht persistente
Fruchtplätze je Zyklus verhindern doppelte Ernte. Rechtsklick prüft Stamm- und
Fruchtrechte, Reichweite, tatsächlichen Strahl, Blattanker, Revisionen und freien
Inventarplatz; Inventar und Fruchtzustand ändern sich zusammen. Ganze Äpfel und
Butzen teilen passende Voxelmodelle für Welt, Hand, Icons und Drops. Verzehr
hinterlässt einen Butzen; Äpfel oder zwei Butzen ergeben mit Dirt einen Setzling.

Natürliche Apfelvorkommen verwenden Ökologierevision 3 für neue Welten. Gespeicherte
ältere Revisionen bleiben erhalten; manuell gepflanzte Bäume funktionieren dort
weiterhin. Protokoll 108 und Index ID11 ergänzen Baum- und Fruchtdaten mit expliziter
Migration der vorherigen Indizes. PLY7 bleibt erhalten. Wachstums- und Fruchtarbeit
sind begrenzt; unveränderte Fruchtticks verändern keine normalen Pflanzenlisten.

Kronen und Zinnen der vorhandenen Materialien 175/180/181/182 leiten ihre
Verbindungen aus tatsächlichen Nachbarformen und Ausrichtungen ab. Das Eckstück
181 erhält zwei Kronen. Alle 16 Verbindungsprofile und die bisherigen 24 Posen
verwenden gemeinsame Mesh-, Auswahl-, Kollisions- und Wasserdaten. Nachbaränderungen
und Sektionsgrenzen aktualisieren die Profile ohne neue Material- oder Speicherdaten.

Auf ausdrücklichen Nutzerwunsch werden keine Grafiktests, Spielclients oder
GPU-Galerien gestartet. Die Abnahme erfolgt ausschließlich kopflos, mit niedriger
Prozesspriorität und einem Cargo-Job.

[Prüfbericht und Referenzen](tools/benchmarks/results/saplings-apples-battlements-2026-11-06/README.md).

Abnahme 200: 4.012 Workspace-Library-/Bin-Tests bestanden, keine fehlgeschlagen,
neun bestehende Tests ignoriert. Build, Clippy aller Targets mit `-D warnings`,
Format, Diff und 60 tatsächliche UI-Stylesheets sind geprüft. Die zusätzliche
Prüfung historischer Fichten und beider Kirschvarianten besteht für echte
Blattdrops und vollständige Queue-Fällungen. F59 und F60 sind aus der offenen
Liste entfernt; F58 und F61 bleiben offen. Es wurden keine Grafiktests oder
Spielclients gestartet.

## 201. Setzlingsanzeige, Natürliche Wuchsformen Und Stapelbare Zinnensäulen

Die Zielanzeige zeigt den bestätigten Setzlingsfortschritt als Balken mit Prozenten.
Identität, Wurzelposition, Art und Stadium werden gegen den tatsächlich betrachteten
Prop geprüft; 100 Prozent bedeutet wartendes Wachstum, keine vorweggenommene Reife.
Apple-Setzlinge erhalten getrennte Apfel-/Laubflächen gegen das Flimmern, Cherry
kleine Blüten auf allen vier Kronenseiten.

Neue ausgewachsene Setzlingsbäume verwenden die größeren natürlichen Generatorformen
und deren Darstellungsstil. Alte ausgewachsene Records behalten ihre bestehende Form;
Generated wird als zusätzlicher Stage-Tag angehängt. Exakte Holz-/Laubzuordnung,
Früchte und Speicherpfade bleiben maßgeblich. Die neue Ökologierevision 4 verwendet
5 Prozent für geeignete Forest-/Plains-Baumkandidaten, ältere Revisionen bleiben
unverändert. Neue Fortschritts- und große Baumdaten verwenden Protokoll 109.

Zinnenmauer (180) bildet Ecken ohne separates Crafting-Item. Ecke (181) bleibt als
Legacy-ID lesbar; Creative-Pick und Abbau liefern 180. Zinnensäulen behalten ihre
vier Kronen und bekommen zusätzliche seitliche Verbinder. Innere Stapelzellen
zeigen reine Schäfte, Fuß und Kopf erscheinen nur an den äußeren Enden. Gemeinsame
Profile steuern Mesh, Auswahl, Kollision, Wasser und Nachbar-Cache.

Keine Grafiktests oder Spielclients werden gestartet.
[Prüfbericht und Referenz](tools/benchmarks/results/sapling-growth-natural-trees-battlements-2026-11-07/README.md).

Abnahme 201: 4.050 Workspace-Library-/Bin-Tests bestanden, keine fehlgeschlagen,
neun bestehende Tests ignoriert. Build, Clippy aller Targets mit `-D warnings`,
Format, Diff und 60 tatsächliche Stylesheets sind geprüft. Bestätigte Section-
importe bewahren die autoritative Baumzuordnung auch beim späten Beitritt;
gewöhnliche lokale Änderungen geben Zellen weiter frei. Neue Apfeläste sind für
64 Seeds flächenverbunden geprüft, alte Ecology3-Formen bleiben unverändert.
Alle 112 Zinnenprofile und 24 Posen sowie HUD, tatsächliche Mesh-/UV-Daten, Teilpakete,
Teilfällung und ID11-Speicherkompatibilität sind kopflos abgesichert. Es wurden
keine Grafiktests oder Spielclients gestartet.

## 202. Gebietshierarchie, Weltoptionen, Setzlinge Und Natürliche Wasserfälle

Setzlinge prüfen bei der Platzierung ihre tatsächliche Miniaturform statt des
späteren Baums. Licht, große Wuchsräume und Baumabstände betreffen das Wachstum.
Natürlicher weicher Bodenbewuchs wird nach vollständiger Rechte-/Kapazitätsprüfung
atomar ersetzt; konkrete Pflanzenfehler werden an den Client gemeldet. Natürliche
Flussfälle behalten beim Landen auf Terrassen ihren Korridor. Gegrabene Kanäle,
gesetzte Quellen, Rohre und Waterlogging behalten ihre gewöhnliche Wasserphysik.
F2 blendet das eigene Egoperspektiven-Item aus.

Claims brauchen zusammenhängendes Land derselben Siedlung, keine Bergfriednähe.
Außenposten, Dörfer und Städte gehören zu einem Hauptkönigreich; Rollen, Mitglieder,
Wappen, Diplomatie und Transport gelten innerhalb der Familie. Die Karte kann ein
Untergebiet als Ziel eines atomaren Claim-Entwurfs wählen. Dörfer/Städte überlagern
eigenes Territorium mit Build-Claims, ohne die darunterliegende Gebietsebene zu
verlieren. Ihr Territorium kann auch zu einem eigenen Außenposten gehören.

Ein Kingdom darf je Level einen Außenposten besitzen. Außenposten starten mit
10 Build-/22 Territorialchunks und erhalten je Level 2/6 weitere. Dörfer starten
mit 10 Build-Chunks, Städte mit 20; weitere Level geben 2 beziehungsweise 4 dazu.
Beide haben kein eigenes Territorium. Städte verlangen Kingdom-Level 3 und zehn
freie Chunks Abstand zum Build-Land des Hauptreichs und seiner Dörfer. Zwischen
Kingdom-/Outpost-Landgrenzen bleiben mindestens 16 freie Chunks. Gründung und
Erweiterung prüfen die Regeln sowie tatsächliche historische Botanlagen.

Alle 20 Regeln sind im Dedicated-Server unter `[kingdom_rules]` und in World Create
konfigurierbar. Dort gliedern die Tabs General, Biome / Structs und Kingdom die
Einstellungen. Singleplayer erhält seine gespeicherten Weltregeln; ausdrückliche
Server-Overrides werden nach dem Laden angewendet und gespeichert. Untersiedlungen
haben standardmäßig eigene Level; alternativ folgen ihre Limits dem Kingdom-Level.

Neue natürliche Anlagen verwenden Strukturenrevision 9 mit eingefrorenem
`structure_border_gap`. Die typübergreifende Auswahl ist begrenzt; bestehende
Revisionen 0–8 bleiben unverändert. Protokoll 110 und Index ID12 ergänzen Hierarchie
und Pflanzenfehler mit expliziter Migration alter Königreichsdaten. PLY7 und
bestehende Baumzuordnungen bleiben erhalten.

Keine Grafiktests oder Spielclients werden gestartet.
[Prüfbericht und Referenz](tools/benchmarks/results/kingdom-territories-saplings-waterfalls-2026-11-07/README.md).

Abnahme 202: 4.097 Workspace-Library-/Bin-Tests bestanden, keiner fehlgeschlagen,
neun bestehende Tests ignoriert. Build von Server und Client, Clippy aller Targets
mit `-D warnings`, Format, Diff und 60 tatsächliche Stylesheets sind geprüft.
Die Registry verlangt bei vorhandenen Claims weiterhin den geschützten Bergfried-
Chunk; konfigurierte Fernclaims bleiben zulässig. Fehlende, fremde oder aufgelöste
Ziele eines Kartenentwurfs werden beim Apply abgewiesen. Die gespeicherte Weltpolicy,
Legacy-Migrationen und die Makrozellenabfrage bis zum 4.096-Chunk-Abstand sind kopflos
abgesichert. Es wurden keine Grafiktests oder Spielclients gestartet.

## 203. Build-Pfade, Rust-Lesbarkeit Und Fluent-TODO

Neue Build-Chunks benötigen einen kantenverbundenen Build-Pfad zum Bergfried ihrer
Siedlung. Territory darf diesen Pfad nicht ersetzen und kann weiterhin an das
zusammenhängende eigene Land andocken. Derselbe atomare Vorher-/Nachher-Vergleich
gilt für Einzelclaims, Batchauswahl, Typwechsel und Freigabe sowie die Legacy-APIs.
Ein alternativer vollständiger Build-Korridor kann im selben Batch entstehen.
Historische Inseln bleiben lesbar und reparierbar, dürfen aber nicht wachsen;
vorher angeschlossene Build-Flächen dürfen nicht abgetrennt werden. Der explizite
Adjazenz-Opt-out bleibt erhalten, die Bergfriedentfernung ist nicht begrenzt.

Die Kingdom-UI prüft Build und Territory getrennt über dieselbe Core-Policy und
cached die Ergebnisse nach den relevanten Revisionen und Einstellungen. Hinweise,
Parent-Territory-Überlagerungen und Bedienbarkeit folgen der tatsächlichen
Serverentscheidung. CPU-Regressionen prüfen Einzel-/Batch-/Releasewege, historische
Layouts, Live-Regelwechsel und tatsächliche UDP-Bestätigungen.

Alle 1.104 Rust-Dateien einschließlich UI-Quellen und Werkzeugen sind auf englische
Rustdocs geprüft. Die vorhandenen Kommentare waren bereits Englisch. Unklare
längerlebige Variablennamen in Welt-, Baum-, Wasser-, Physik-, Netzwerk-, Modell-
und UI-Pfaden sind präzisiert; Spieltexte, IDs und Speicherfelder bleiben erhalten.
F72 erfasst das neue Sprachsystem mit der vorhandenen Fluent-Unterstützung von
TiltUI als offene Aufgabe.

[Prüfbericht und Referenz](tools/benchmarks/results/build-claims-rust-cleanup-2026-11-07/README.md).
Keine Grafiktests oder Spielclients werden gestartet.

Abnahme 203: 4.111 Workspace-Library-/Bin-Tests bestanden, keiner fehlgeschlagen,
neun bestehende Tests ignoriert. Server-/Client-Build, Clippy aller Targets mit
`-D warnings`, Format, Diff und 60 tatsächliche UI-Stylesheets sind geprüft.
Rustdoc-Audit, Quell-/Loghashes und echte Core-/UDP-/UI-Regressionen sind dokumentiert.
Binärasset-Attribute schützen exakte Modell-/Textur-/Audiodaten im Git-Commit;
historische Patch-/Profilingdaten behalten ihre ursprünglichen Leerzeichen.
Der gesamte nicht ignorierte Projektstand wird auf GitHub `main` veröffentlicht.
Es wurden keine Grafiktests oder Spielclients gestartet.

## 204. F62 — Schienenmodule Und Gespeichertes Streckennetz

Die neue Schienenfamilie folgt der Drei-Block-Referenz mit Holzschwellen,
profilierten Stahlrails, Befestigungen und rotem Weichenhebel. Die angehängten
Block-IDs 196–205 umfassen Gerade, zwei Viertelkurven mit Radien 4,5/8,5,
1:4-Steigung, sanfte Ein-/Ausfahrt, linke/rechte Weiche, Kreuzung und Prellbock.
Gemeinsame Core-Profile liefern Anschlüsse, Fußabdruck, Stützpunkte,
Kollisionsformen und sichtbare Flächen für native Assets und Terrain-Slices.
Coplanare Überdeckungen an den Weichen werden geometrisch entfernt.

Die vollständige Bauvorschau dockt an erreichbare freie Anschlüsse an; Höhe und
Neigung müssen exakt passen. Ungültige Entwürfe bleiben rot sichtbar.
Baurechte, Unterbau, Props und Spieler werden über sämtliche belegten Zellen
geprüft. Ein Survival-Item baut ein vollständiges Modul; Abbau an jeder
Sekundärzelle entfernt alles und gibt genau ein Modul zurück. Rechtsklick mit
leerer Hand schaltet die Weiche samt Stahlzungen und Hebel um. Gehaltene
Schienenitems verlängern stattdessen die Strecke. Alle zehn Werkbankrezepte
akzeptieren sämtliche vorhandenen Holzarten. Vorhandenes Wasser bleibt erhalten.

Persistente Objektmetadaten speichern Anker, Besitzerzellen, Ausrichtung und
Weichenstellung über bestehende Journal-/Sectionpfade. Vollständige gespeicherte
Module bilden auch ohne geladene Chunks einen deterministischen Streckengraphen.
Exakte Sechzehntelblock-Anschlüsse berücksichtigen Gegenrichtung und Neigung;
Kreuzungen verbinden ihre unabhängigen Achsen nicht. Nur der aktive Weichenweg
ist befahrbar. Rekonstruktion und Routensuche haben feste Arbeitsgrenzen;
sachfremde Terrainänderungen lassen den Graphcache bestehen.

Die autoritative Mehrzellentransaktion prüft Revisionen, gehaltenen Slot,
Inventar-/Drop-/Statistikkapazität und sämtliche Rechte vor jeder Veröffentlichung.
Weltänderungen kopieren nur betroffene Sparse-Sections und deren Mesh-Halo.
Unterbaublöcke bleiben geschützt, bis das unterstützte Modul entfernt ist.
Protokoll 111 ergänzt den Inhaltsvertrag; bestehende Saveformate und alte IDs
bleiben erhalten. Zugmontage und Fahrbetrieb bleiben unter F63/F64/F53 offen.

[Prüfbericht, Bedienung und Referenz](tools/benchmarks/results/railway-system-2026-11-07/README.md).
Keine Grafiktests oder Spielclients werden gestartet.

Abnahme 204: 4.154 Workspace-Library-/Bin-Tests bestanden, keiner fehlgeschlagen,
neun bestehende Tests ignoriert. Clippy aller Targets mit `-D warnings` ist sauber.
Echte Grid-Autofills für sämtliche Module/Holzarten, Save/Reload und deterministische
Rekonstruktion, sekundäre Raytreffer ohne geladenen Anker sowie tatsächliche
UDP-Clients samt Late Join, Weichenschaltung und Survival-Abbau sind abgesichert.
Der abschließende Workspace-Build erstellt aktuelle Client-/Serverdateien;
Formatierung und Diff sind sauber. Es wurden keine Grafiktests oder Spielclients
gestartet.

## 205. Freier Gleisbau, Holzvarianten Und Prop-Clearance

Ein gemeinsames Track-Item ersetzt die separaten Kurven-/Steigungsrezepte.
Platzierte Startstücke lassen sich durch Auswahl eines freien Anschlusses und
eines Zielpunkts mit einer echten kubischen Bézierkurve verbinden. Der Verlauf
entsteht aus Endpositionen und Tangenten, ohne auf die alten vorgegebenen Radien
zurückzufallen. Vorschau, Kollision, Waterlogging und Streckennetz verwenden
dieselben Profile. Der Zielklick bleibt in 4,5 Metern Reichweite, die Verbindung
ist auf 32 Meter begrenzt. Kardinale Bodenenden lassen eindeutige anschließende
Voxel-Baugruppen zu; Kurvenradien und Höhenverlauf werden frei berechnet.

Eiche, Birke, Fichte und Kirsche bleiben beim Crafting, Platzieren, Kopieren,
Speichern und Abbau erhalten. Terrain, Icons und Vorschau verwenden die echten
Plankenmaterialien. Too Many Items bietet vier Holzvarianten des gemeinsamen
Items; die bisherigen Form-IDs bleiben für vorhandene Welten lesbar. Benutzen
(standardmäßig R) schaltet Auto, linke/rechte Weiche, Kreuzung und Prellbock um.
Alle verwenden dasselbe Item. Shift+Platzieren verwirft eine Verbindungswahl.
Der HUD zeigt Bauform, tatsächliche Tastenbelegung und Materialbedarf an.

Die Kosten betragen aufgerundet ein Gleisitem je vier Meter räumlicher Pfadlänge;
mehrsträngige Bauformen zählen ihre Pfade zusammen. Survival reserviert die
passende Holzart und Menge vor dem Bau. Abbau über jede Sekundärzelle entfernt
die gesamte Baugruppe und gibt dieselbe Menge zurück.

Berührte gewöhnliche Props werden automatisch freigeräumt, einschließlich
gesetzter Dekoration, Setzlinge und Nutzpflanzen. Die Bereinigung prüft die
tatsächlichen Formen und Rechte über den gesamten Propbereich samt Support.
Blöcke, Bienenstöcke und interne Baumfrüchte bleiben Hindernisse. Prop-Tombstones,
Pflanzenmetadaten, Gleiszellen, Inventar und normale Survival-Drops werden vorab
geprüft und atomar veröffentlicht. Die Transaktion kopiert nur betroffene
Sparse-Sections und betroffene Baumdatensätze.

Protokoll 112 überträgt Verbindungsbefehle und Kurvenmetadaten. Die RG13-Hülle
speichert Kontrollpunkte pro besessener Zelle; ein expliziter vierfeldriger
Adapter hält historische Regionlayouts unverändert lesbar. Gameplayindex ID12
und Spielerformat PLY7 bleiben erhalten. Begrenzte Arc-Caches verhindern dauerhaft
angesammelte Vorschauformen; Gültigkeit und Längen erhalten einen gesonderten
begrenzten Metrikcache.

[Prüfbericht und Bedienung](tools/benchmarks/results/free-rail-building-2026-11-07/README.md).
Keine Grafiktests oder Spielclients werden gestartet.

Abnahme 205: 4.186 Workspace-Library-/Bin-Tests bestanden, keiner fehlgeschlagen,
neun bestehende Tests ignoriert. Freie Verbindungen sind über echte UDP-Clients
einschließlich normalem Teleport, Holz-/Materialverbrauch, Late Join und sekundärem
Survival-Abbau geprüft. Source-/Zielrechte sowie direkte Spezialanschlüsse an
fremden Claimkanten sind abgesichert. Alle 169 Itemdefinitionen sind vollständig.
Clippy aller Targets mit `-D warnings`, Format, Diff und der vollständige
Client-/Server-Build sind sauber. Logs und Quell-/Asset-Prüfsummen sind im Bericht
gespeichert; keine Grafiktests oder Spielclients wurden gestartet.

## 206. Gleiskurven, Eigene Weichen Und Schotter

Ein gehaltenes Gleis erzwingt keine leere Blockinformation mehr. Der obere HUD
folgt ausschließlich dem tatsächlichen Ziel. Ein getrennter rahmenloser
Bauhinweis zeigt Tasten, Bedarf und konkrete Fehler, auch wenn eine geometrisch
unmögliche Kurve keinen Ghost erzeugen kann. HUD-Ausblendung und Menüs gelten
auch für diesen Hinweis.

Neue Bodenenden folgen dem Streckenverlauf statt der Kamerarichtung. Der
Bézierplaner behält gültige bestehende Kontrollsätze und sucht bei Bedarf mit
höchstens 36 unabhängigen Griffpaaren nach sicheren Kurven. Dadurch funktionieren
auch ungleich lange 90-Grad-Schenkel. Eigene Weichenabgänge werden als wirkliche
Ports verlängert; seitliches Einsetzen in die Gleismitte bleibt ein Weichenfall.
Radius, Steigung, Länge, Reichweite, Besitz, Unterbau und Spielerfreiheit bleiben
geprüft.

Die Materialien 202/203 sind eigene linke/rechte Weichenitems in allen vier
Holzarten. Eine vollständige Weiche kostet und liefert ein Item. Track 196 behält
Längenkosten und seine Formen Gleis/Kreuzung/Prellbock. Der separate Steller 209
verwendet eine eindeutige unmittelbare Nachbarweiche, echte Zielstrahlen und
Rechte über den Steller und die ganze Baugruppe. Umschalten bleibt atomar.
Der alte rote Hebel wandert aus dem Gleismodell in das eigene Stellermodell;
historische Besitzerzellen bleiben ohne Phantomkollision/-stützen erhalten.
Alte getragene Formitems bleiben über ausdrücklich begrenzte Source-Regeln nutzbar.

Schotter 206, vierstufige Treppe 207 und Slab 208 erhalten eine eigene kantige
512er-Steintextur, echte gemeinsame Formprofile und normale Rezepte. Alte
Treppenformen und Mixed-Slab-Codes bleiben erhalten. Der live ausgewertete
Gleisbettfaktor 1,5 berücksichtigt bestehenden Kies 42, alle Schotterformen und
die tatsächliche Oberseite gemischter Slabs. `speed_limit` und `travel_seconds`
werten vollständige Module und aktive Strecken aus; der Fahrbetrieb bleibt F53.
Die Zielinformation zeigt den Bonus, Gravel-Kontakte und ein bestätigter
Stellerschaltimpuls nutzen vorhandene Sounds.

Protokoll 113 grenzt die neuen Materialien und Itemregeln ab. RG13/ID12/PLY7
bleiben unverändert. Sechs neue Rezepte ergeben 331, sechs Holz-Weichenvarianten
ergeben insgesamt 175 zusätzliche Itemdefinitionen. Die zentrale CPU-Abnahme wird im
[Prüfbericht](tools/benchmarks/results/rail-curves-switches-ballast-2026-11-08/README.md)
festgehalten. Keine Grafiktests oder Spielclients werden gestartet.

Abnahme 206: 4.213 Workspace-Library-/Bin-Tests bestanden, keiner fehlgeschlagen,
neun bestehende Tests ignoriert. Echte Kurven und beide Weichenabgänge,
alle Holzarten, autoritative Stellerplatzierung/-bedienung über UDP mit Late Join,
getimter sekundärer Abbau, leere Legacybesitzzellen ohne Phantomkollision,
Schotteroberseiten und der einmalige Faktor 1,5 sind geprüft. Zielkarten und
Bauhinweise bestehen ihre CPU-Prüfungen. Clippy aller Targets mit `-D warnings`,
Format, Diff, kopfloser Assetexport und aktueller Client-/Server-Build sind sauber.
Keine Grafiktests, Spielclients oder Screenshots wurden gestartet.

## 207. Kontextgleise, Sanfte Stufen Und Freier Unterbau

Die aktuelle Zielerfassung liefert direkte Andockvorschläge für Geradeaus und
echte Kurven zwischen freien Gleisenden. Nur ein sichtbarer Andockpunkt mit
zwei tatsächlich verschiedenen Geometrien übernimmt das Mausrad; sonst bleiben
Hotbar und Offline-Palette normal nutzbar. Die Radereignisse wirken nicht später
nach. Slot, aktives Item und Ansicht wechseln sofort, das Targeting wird vor
der Interaktion mit dem neuen Item wiederholt. Alte Bauabsichten einer anderen
Holzvariante werden nicht übernommen. Shift+Platzieren merkt zusätzlich einen
manuellen Verbindungsstart und verwirft einen bereits gemerkten Start.

Eine direkt angrenzende Stufe um einen Block kann das letzte vollständige gerade
Viermeterstück in eine glatte kubische Rampe umformen. Der hintere Anschluss
bleibt exakt erhalten, das neue obere Ende dockt am erhöhten Gleis oder an der
nahen Oberkante des Bodens an. Vertikale Wandtreffer werden ausgewertet.
Dynamische Verbindungen erlauben jetzt mindestens 2,5 Meter Radius und maximal
50 Prozent Steigung; alte native Kurven/Steigungen bleiben geometrisch gleich.
Der Rampenplaner prüft höchstens 49 Griffpaare, ebene Kurven weiterhin 36.

Protokoll 114 überträgt Kontextauswahl und einen stabilen Fingerabdruck des
Bauvorhabens. Die Authority rekonstruiert Geometrie, Quell-/Zielports und Kosten
aus dem tatsächlichen Zielstrahl. Veränderte Vorschläge, fehlende Rechte,
veraltete Revisionen oder falsche Holzart werden vor jedem Commit abgelehnt.
Ein Rampenumbau reserviert nur den positiven Mehrbedarf; es gibt keine doppelte
Rückgabe des alten Stücks. Alte/neue Zellen, Wasser, Props und Besitz werden
atomar aus betroffenen Sparse-Sections veröffentlicht. Lokale Portabfragen
ersetzen globale Scans; die Suche erzeugt keine Terrainchunks.

Die gesonderte Sperre für Abbau und Änderungen unter vorhandenen Schienen ist
entfernt. Normale Rechte, Werkzeuge, Zielstrahlen und Abbauzeiten bleiben nötig.
Vorhandene Gleise behalten Geometrie, Wasser und Routing; der Schotterbonus fällt
bei fehlender Auflage sofort weg. Neue Platzierungen prüfen weiterhin ihren
Unterbau. RG13/ID12/PLY7 und die vorhandenen Block-/Item-/Rezept-IDs bleiben gleich.

[Prüfbericht und Bedienung](tools/benchmarks/results/contextual-track-building-2026-11-08/README.md).
Keine Grafiktests oder Spielclients werden gestartet.

Abnahme 207: 4.233 Workspace-Library-/Bin-Tests bestanden, keiner fehlgeschlagen,
neun bestehende Tests ignoriert. Echte Stufenwände, Rampen in vier Ausrichtungen,
erhaltene Nachbaranschlüsse, atomare Prop-/Wasser-/Besitzänderungen, Rechte und
Überlauf-Rollbacks sind geprüft. Die direkte Kontextwahl läuft über echte
UDP-Clients mit Survival-Verbrauch, Late Join und veralteter Bauabsicht.
Mausrad, sofortiger Hotbarwechsel, erneutes Targeting und manueller Shift-Start
bestehen ihre CPU-Prüfungen. Normaler Unterbauabbau erhält Gleise und entfernt
den fehlenden Schotterbonus. Clippy aller Targets mit `-D warnings`, Format,
Diff, kopfloser Assetexport und aktueller Client-/Server-Build sind sauber.
Keine Grafiktests, Spielclients oder Screenshots wurden gestartet.

## 208. Geländerampen, Kurven Mit R Und Direkte Kollisionsdaten

Die Rampenerkennung am freien unteren Gleisende berücksichtigt tatsächliche
Geländestufen ohne vorausgesetztes oberes Gleis. Eine begrenzte Suche ermittelt
die erste passende Plattformkante. Das vorhandene Endstück und kurze Lücken
bis zur Stufe bilden eine gemeinsame sanfte Rampe; der hintere Anschluss bleibt
erhalten. Vorschau und Authority verwenden dieselbe kanonische Geometrie und
prüfen weiterhin Landehöhe, Unterbau, Freiraum, Holzart, Besitz und Rechte.

R wechselt Auto, Kurve und Prellbock vor der aktuellen Kontextplanung. Reale
Kurven lassen sich am passenden Anschluss direkt wählen; der manuelle
Start-/Zielpfad bleibt verfügbar. Neue Kreuzungen werden auch serverseitig
abgelehnt, einschließlich altem Kreuzungsbestand. Gespeicherte Kreuzungen
bleiben lesbar und abbaubar. Die vorhandenen IDs und Saveformate bleiben gleich.

Der Gleishinweis zeigt eine kurze Form, separaten Materialbedarf, kompakte
Tasten und einen kurzen Status. Konfigurierte Belegungen bleiben korrekt;
ein schwarzer Zwei-Pixel-Schatten verbessert die Lesbarkeit auf hellem Terrain.

Der Kollisionsaufbau wird von der sichtbaren Terrain-Neuvermaschung entkoppelt.
Bestätigte Blockdaten können die Kollision direkt versorgen. Entfernte
Generator-Overrides mit unbekanntem Grundwert bleiben bis zur echten
Neupublikation gesperrt, überholte Workergebnisse werden weiter verworfen.
Damit entfällt das bisherige Warten auf den Terrain-Mesh vor dem
Kollisionsjob. Ein erforderlicher aktueller Kollisionsjob bleibt abgesichert.
Gleise liefern ihre geclippten Kollisionsflächen für jede belegte Zelle,
einschließlich sekundärer Rampenabschnitte über Sectiongrenzen.

[Prüfbericht und Bedienung](tools/benchmarks/results/track-ramps-input-collision-2026-11-08/README.md).
Keine Grafiktests oder Spielclients werden gestartet.

Abnahme 208: 4.251 Workspace-Library-/Bin-Tests bestanden, keiner fehlgeschlagen,
neun bestehende Tests ignoriert. 60 echte Blickstrahlfälle ohne oberes Gleis,
vier Ausrichtungen, kurze Abstände, frisches Player-Targeting mit signierter
Bauanfrage und autoritativer Rampenbau mit realem Spielerkörper sind geprüft.
R-/Mausradpriorität, freie Kurven und neue/veraltete Kreuzungsbestände sind
abgesichert. Kollisionen entstehen bei ausdrücklich ausstehendem Render-Mesh;
alte Worker, unbekannte Nachbarwerte und fehlerhafte Deltas bleiben gesperrt.
Sämtliche Gleiszellen liefern auch über negative Sectiongrenzen und bei
entladenem Anker ihre erwarteten Kollisionsflächen. Clippy aller Targets mit
`-D warnings`, Format, Diff, kopfloser Assetexport und der aktuelle vollständige
Client-/Server-Build sind sauber. Keine Grafiktests, Spielclients oder
Screenshots wurden gestartet.

## 209. Kurzes Startgleis Und Zielabhängiges Strecken

Der Track-Bau beginnt mit einem genau einen Meter langen und drei Blöcke breiten
Startgleis. Danach folgt die vollständige Vorschau dem anvisierten Ziel:
Gerade, Kurve oder sanfte Steigung. Die nächste Platzierung baut diese
zusammenhängende Verbindung. Ein Klick auf ein vorhandenes freies Ende wählt
es als Start, ohne ein automatisches Viermeterstück anzubauen.

Nach der bestätigten Platzierung wird das tatsächliche neue freie Ende zum
Weiterbauen ausgewählt. Eindeutige lokale Tokens werden konkreten gesendeten
Anfragen zugeordnet. Erst die passende Annahme und vollständig replizierte
Geometrie einschließlich Holzart erlauben den Übergang. Abbruch, Item-/Sitzungs-
wechsel, gesperrte Eingabe, veränderte Quellen und verspätete Antworten können
keine alte Bauabsicht reaktivieren. Ablehnung erhält einen weiterhin gültigen
Start für den nächsten Versuch; ausstehende Antworten haben eine feste Grenze.

Protokoll 115 ergänzt SeedRail mit tatsächlichem Zieltreffer und einem stabilen
Fingerabdruck. Die Authority plant das Startgleis selbst und prüft Sichtlinie,
Reichweite, ausgewählten Bestand, Holz, sämtliche drei Zellen, Unterbau, Props,
Rechte und Spielerfreiheit vor der bestehenden atomaren Transaktion. Vorschau,
Materialbedarf, Abbau und Streckenanschlüsse verwenden gemeinsame Profile.

Neue Startstücke benutzen vorhandene dynamische Kontrollpunkte und Material 196.
Alte gespeicherte Viermeterstücke und native Profile behalten ihre Geometrie.
RG13/ID12/PLY7, Material-IDs, Holzdefinitionen, Rezeptzahl und die Mengenregel
für gestreckte Verbindungen bleiben erhalten. Die Hilfen zeigen Start, Ziel,
Materialbedarf und ausstehende Bestätigung kompakt mit echten Tastenbelegungen.

[Prüfbericht und Bedienung](tools/benchmarks/results/seed-stretch-track-building-2026-11-08/README.md).
Keine Grafiktests oder Spielclients werden gestartet.

Abnahme 209: 4.258 Workspace-Library-/Bin-Tests bestanden, keiner fehlgeschlagen,
neun bestehende Tests ignoriert. Exakte 1×3-Startstücke in vier Ausrichtungen und
allen Holzarten, kontinuierliche Gerade-/Kurven-/Rampenplanung sowie eindeutige
Bestätigungen mit beiden Replikationsreihenfolgen sind geprüft. Doppelklicks,
Abbruch, Itemwechsel, alte/abgelehnte Antworten und Zeitüberschreitung können
keine fremde Baukette aktivieren. Der echte UDP-Lauf baut mit zwei Clients ein
Fichten-Startgleis und seine Fortsetzung, prüft Verbrauch und Metadaten sowie
einen späteren Beitritt mit identischem Streckennetz. Clippy aller Targets mit
`-D warnings`, Format, Diff, kopfloser Assetexport und aktueller vollständiger
Client-/Server-Build sind sauber. Keine Grafiktests, Spielclients oder
Screenshots wurden gestartet.

## 210. Präziser Gleisbau, Bewegungsbestätigung Und Fahrzeugmontage

Gleise verwenden einen eigenen 32-Meter-Baustrahl. Ein tatsächlich tragender
Block unter dem Modul reicht aus. Die genaue Zielplanung folgt dem Strahltreffer
mit halben seitlichen Rasterabständen; Blockgrenzen lösen keine sprunghaften
unterschiedlichen Kurven mehr aus. Vorschau und signierter StretchRail-Auftrag
verwenden dieselbe Geometrie, Quelle und Holzart. Normale Blockaktionen behalten
4,5 Meter Reichweite; Zielports außerhalb des Baubereichs und verdeckte Ziele
bleiben gesperrt, einschließlich echter Geometrie montierter Fahrzeuge.

Eine Bewegungsbestätigung entspricht genau einem tatsächlich simulierten festen
Schritt. Wartende gültige Eingaben werden einzeln verarbeitet und verfallen
nicht nach 250 ms. Kurze Paketlücken wiederholen keine alte horizontale Bewegung;
am Boden wird der Halt geprüft, in der Luft bleibt die Wartezeit begrenzt und
erneuert sich nicht durch sporadische Pakete. Damit verliert die Clientvorhersage
bei Baubursts und schneller Bewegung keine bereits bestätigten Schritte.

F63 ergänzt die Materialien 210–217 mit Fahrgestell, Drehgestell, Rahmen,
Kupplung, Führerstand, Sitz, Laderaum und Montagestelle samt nativen Modellen,
gemeinsamen Kollisionsprofilen, Katalogeinträgen und Werkbankrezepten. Verbundene
Aufbauten werden als Lokomotive, Tender, Personen- oder Güterwagen montiert und
wieder zerlegt. Teile, Ausrichtungen, Besitzer, Masse und private Ladung gehen
gemeinsam zwischen Welt und Fahrzeug über. Veraltete Revisionen, fremde Besitzer,
geschützte Zellen, belegter Raum und benutzte Sitze verhindern Teiländerungen.

Die Montagestelle öffnet per Rechtsklick eine responsive Fahrzeugverwaltung.
Kupplungen sind beidseitig, prüfen passende Enden und schließen Kreise aus.
Laderäume besitzen 18 echte Slots; der Server prüft Ladungs- und Spielerrevision
sowie Craftingreservierungen. Sitze erhalten bestätigte lokale und entfernte
Spielerposen. Die konfigurierte Fly-Down-Aktion verlässt den Sitz über einen
erneut geprüften freien Ausstieg; Pferd und Zugsitz schließen sich gegenseitig aus.

Darstellung, Treffer und zusammengesetzte Physikkollisionen verwenden dieselben
Bauteile. Die Räder entsprechen zwei Metern Spurweite; ein gemeinsamer
Modellversatz bringt sie auf den Schienenkopf. Auf- und Abbau übergeben die
Kollision erst mit bereitstehendem Ersatz. Kausale Abschnittsversionen sichern
auch zusammengefasste Änderungen beim schnellen Zerlegen, Weiterbearbeiten und
erneuten Montieren ab. Identische Ersatzgeometrie übernimmt den vorhandenen
Kollisionskörper; entfernte Physikbereiche halten keine alten Körper dauerhaft fest.

Protokoll 116 repliziert öffentliche Fahrzeugansichten und Sitzbindungen, aber
keine privaten Ladungsinhalte an Zuschauer. ID13 erweitert den eingefrorenen
ID12-Index um die Zugregistry; RG13 und PLY7 bleiben erhalten. Frühere Welten
laden mit leerer Zugregistry, verbrauchte Fahrzeug-IDs werden nicht wiederverwendet.
F53, F54 und F64 bleiben als Fahrbetrieb, Detailbaukasten und freier Zugbau offen.

[Bauanleitung und CPU-Prüfbericht](tools/benchmarks/results/train-assembly-movement-2026-11-08/README.md).

Abnahme 210: 4.310 Workspace-Library-/Bin-Tests bestanden, keiner fehlgeschlagen;
neun bestehende Tests ignoriert. Die vollständigen UDP-Abläufe prüfen Montage,
private Ladungsübertragung, Sitzbindung, Ausstieg, Zuschauer, späteren Beitritt
und Demontage. Schnelle Bewegung mit Baubursts, Sprünge und Paketlücken bestehen
ihre Regressionsprüfungen. Kausale Kollisionsübergänge, schnelle Wiederaufbauten,
Spurweite und alte Speicherindizes sind ebenfalls geprüft. Keine Grafiktests,
Spielclients oder Screenshots wurden gestartet.
Clippy aller Targets mit `-D warnings`, Format, Diff, kopfloser Assetexport und
der aktuelle vollständige Workspace-Build sind sauber.

## 211. Stylesheet-Laden, Eingaberückstand Und Gleisbau Um Die Eigene Figur

Die nicht unterstützte CSS-Kurzform inset im Zug-Stylesheet ist durch vier
explizite Seitenabstände ersetzt. Alle 61 ausgelieferten Stylesheets werden nun
im CPU-Test mit dem echten TiltUI-Assetparser geprüft. Die Zugvorlage gehört
auch zur bestehenden kopflosen Template-Abnahme.

Kurze nur serverseitige Simulationspausen konnten einen dauerhaften
Eingaberückstand erzeugen: Ein neuer Input pro Client-Tick und höchstens ein
verbrauchter Input pro Server-Tick lassen danach keinen Aufholspielraum. Der
Client begrenzt seinen Vorlauf nun auf ungefähr 300 ms, abhängig vom festen Takt
und begrenzt auf 8 bis 64 Eingaben. Er wartet bei voller Vorhersage auf echte
Bestätigungen. Bereits simulierte Eingaben werden nicht verworfen; der Server
behält seine tatsächliche Ein-Schritt-Abrechnung und Missbrauchsgrenze 128.

Bestätigte Schritte verlassen die Warteschlange auch bei noch ausstehendem
Kollisions-Replay. Der getrennte 30-Sekunden-Watchdog wird nach empfangenen
Snapshots ausgewertet, damit gepufferter Fortschritt zuerst zählt. Wartende
Sprungimpulse bleiben bei begrenzter Vorhersage erhalten. Fertige, weiterhin
gültige Terrainkollisionen werden vor der festen Aufholschleife mit expliziter
Physikpose aktiviert; tatsächliche SpatialQuery und alte Workerstände sind
geprüft. Die globale Avian-Schedule wird nicht verändert.

Gleisvorschau und Authority lassen den anfragenden Spieler selbst im neuen
Gleis stehen. Nur seine echte Entity beziehungsweise sein lokales Replikat
werden ausgenommen. Andere Spieler bleiben auch bei identischer Position
geschützt; Rechte, Materialbedarf, Signaturen und normale Blockregeln bleiben
erhalten. Tiefe Schienenüberschneidungen einer ruhenden Figur können über den
gemeinsamen Motor um höchstens 1,02 m angehoben werden. Echte Physiküberschneidung,
Halt oder erlaubter Schwebeflug, geladener Bewegungsraum und freier Kopfraum sind
erforderlich. Normale Bloecke lösen das nicht aus; es gibt keinen seitlichen
Versatz und keinen Teleportauftrag.

Abnahme 211: 1.251 Tests in Network, Physics, Player und UI bestanden, keiner
fehlgeschlagen; ein bestehender Test ignoriert. Ein realer UDP-Lauf mit 3600
unabhängig getakteten Frames erreicht 60 simulierte Sekunden, 275 blockierte
Server-Ticks, 90 Blockänderungen und maximal 18 offene Eingaben ohne Disconnect.
Maximale beobachtete Positionsabweichung: 0,000079 m. Starts, Geraden, Kurven,
Rampen, gleich positionierte andere Figuren, Decken, Schwebeflug, normale
Festkörper und Quelldatenwechsel sind geprüft. Keine Grafiktests, Spielclients,
Screenshots oder GPU-Abnahmen wurden gestartet.

[Prüfbericht](tools/benchmarks/results/rail-input-recovery-2026-11-08/README.md).
Clippy aller Workspace-Targets mit `-D warnings`, Format, Diff und der aktuelle
vollständige Workspace-Build sind sauber.

## 212. Freier Zugbau, Montagevorlagen Und Asynchrone Baukollisionen

Die Montageoberflaeche nimmt Mausereignisse unter dem nichtinteraktiven
Spieloverlay wieder an. Dekorative Ebenen blockieren keine Eingaben;
Text-/Icon-Klicks erreichen ihren eigentlichen Button. Die Typauswahl bleibt
auch waehrend einer laufenden Abfrage bedienbar und zeigt ihren aktiven Zustand.

Lokomotive, Tender, Personen- und Gueterwagen bieten eine optionale Vorlage
auf einem passenden geraden Gleis. Gruene Kaestchen markieren fehlende Teile,
rote belegte oder falsch ausgerichtete Stellen. Nach dem Schliessen bleibt die
Vorlage sichtbar; Zielhinweis und Fortschritt helfen beim Bauen. Fertige Teile,
erfolgreiche Montage, Abbruch und Welt-/Verbindungswechsel bereinigen die
entsprechenden Platzhalter. Die Vorlage editiert keine Welt und verbraucht
keine Gegenstaende. Die allgemeine Bodenpflicht fuer Moebel/Zugteile entfaellt:
an vorhandenen Blockseiten darf schwebend weitergebaut werden. Spezifische
Halteregeln fuer Pflanzen, Fackeln, Laternen und andere Befestigungen bleiben.

F64 erweitert montierte Fahrzeuge um selbst gesetzte registrierte Bloecke,
Formen, volle Ausrichtungscodes, Holzarten, Farben und Wappen. Verbundene
Fenster, gemischte Doppeltueren, Rohrverbindungen, Becken und Bauteilverbindungen
behalten ihre Geometrie. Kollisionsvolumen werden mit ausschliesslich eigenen
Fahrzeugnachbarn erfasst; identische Verbundvolumen werden nur einmal montiert.
Die urspruengliche Position bestimmt weiterhin die zufaellige Dekorvariante.

Private Inventare, Verarbeitung, Brennstoff, Fluessigkeiten, Filter und
Oberflaechenzustand wandern atomar in das Fahrzeug und bei Demontage zurueck.
Normale Truhen lassen sich auch montiert als geschuetzte Ladung benutzen.
Andere Maschinen pausieren ohne Weltanschluesse. Mehrteilige Objekte muessen
vollstaendig enthalten sein; Rechte, Kapazitaeten, Zustand und Freiraum werden
vor der gemeinsamen Veroeffentlichung geprueft. Landschaft, Gleise, Montagestellen
und freies Weltwasser werden nicht eingesammelt. Separat simulierte angehaengte
Pflanzen/Weltobjekte und lebende registrierte Baeume bleiben geschuetzt.
Fahrbetrieb und Dampfantrieb bleiben F53.

Protokoll 117 traegt die oeffentliche Form-/Farbbeschreibung ohne private
Maschinendaten. Grenzen fuer Teile, Kollisionsvolumen und private Nutzlasten
begrenzen Speicher und Nachrichten. Zugindex ID14 liest ID13 ueber dessen
unveraenderte historische Datenstruktur; alte Speicherindizes bleiben lesbar.

Die aufwendige Terrain-Modellgeometrie wird gemeinsam mit dem Collider im
Hintergrund berechnet. Der Hauptthread uebergibt begrenzte Voxel- und
Objektmomentaufnahmen. Fahrzeugmeshes und Fahrzeugkollisionen verwenden ebenfalls
begrenzte Hintergrundjobs. Auch ueberholte native Jobs zaehlen bis zum Abschluss
gegen die Grenze von zwei Builds; alte Ergebnisse koennen nach Umbau oder
Demontage nicht publiziert werden. Revisionen, Daten-/Profilpruefung und sichere
Kollisionsuebergaben bleiben erhalten.

[Bauanleitung](docs/train-building.md).

Abnahme 212: 4.152 Workspace-Library-Tests bestanden, keiner fehlgeschlagen;
neun bestehende beziehungsweise explizite Messtests ignoriert. Geprueft sind
unter anderem echte registrierte HTML-Klickhandler, alle Vorlagenausrichtungen,
schwebender Weiterbau, echte Physikkontakte und veraltete Hintergrundjobs,
private Zustandsrueckgabe, alte Speicherindizes sowie Montage, normale Truhe,
gefaerbte Formbloecke, Beobachter und spaeterer Beitritt ueber echtes UDP.
Eine separate CPU-Mikromessung prueft die Entlastung des Hauptthreads.
Keine Grafiktests, Spielclients, Screenshots oder GPU-Abnahmen gestartet.

[Pruefbericht](tools/benchmarks/results/free-train-building-2026-11-08/README.md).
Der aktuelle vollstaendige Workspace-Build, Clippy aller Targets mit
`-D warnings`, Format und Diff-Pruefung sind sauber.


## 213. Brennstoffbetrieb, Fahrhebel Und Vollstaendiger BR52-Baukasten

Fahrgestell und Rahmen sind volle Bloecke. Groessere Raeder liegen auf den
Schienen auf; Kupplungen, Sitze, Laderaeume, Kessel und Daecher schliessen an
ihre Montageflaechen an. Die Bauvorlage erlaubt 3 bis 15 Meter Mittenabstand
zwischen den beiden Drehgestellen. Normale Bloecke, Formen und Holzvarianten
bleiben als freier Fahrzeugaufbau verwendbar; der Laderaum hat keine feste
Position relativ zum Fahrer.

Der neue Brennstoffmotor nutzt ein echtes privates Inventar und die vorhandenen
Ofenbrennstoffe. Der Besitzer steuert im Fuehrerstand einen siebenstufigen
Fahrhebel, Bremse und Rueckwaertsfahrt. Konfigurierte Tastaturbelegung, native
HTML-Buttons und Slider sind angebunden; die Anzeige verwendet echte
Geschwindigkeit in Bloecken/s beziehungsweise Metern/s. Motor und Ladung sind
vom eigenen Fuehrerstand aus erreichbar, auch bei Einbau hinter dem Fahrer.
Raeder und Gestaenge laufen passend zu den gefahrenen Metern; der sichtbare
Fuehrerstandhebel folgt der gewaehlten Stufe.

Fahrzeuge folgen aktiven Gleiswegen, Kurven und Rampen. Gekuppelte Wagen werden
zusammen geprueft und veroeffentlicht. Das bestehende Live-Gleisbettlimit gibt
seinen einmaligen Schotterbonus. Unbekannte Bereiche, unpassende Gleise,
Hindernisse, Personen, fremde Fahrzeuge und fehlende Rechte stoppen die Fahrt.
Geschwenkte Weichen unter einem Zug duerfen seine Radkontakte nicht versetzen.
Begrenzte Bewegungshuellraeume erfassen auch duenne Hindernisse zwischen zwei
Posen; gleichzeitig geplante Gruppen reservieren ihren Weg gegeneinander.

Montage setzt angehaltene, entkuppelte Fahrzeuge am aktuellen Ort in die Welt
zurueck. Der Fahrer steigt zuerst aus; verbleibende Mitfahrer verhindern den
Rueckbau. Ganze Formen, Tueren-/Fensterpartner, Rohrachsen, Inventare,
Maschinenmetadaten und angefangener Motorbrennstoff werden passend mitgedreht.
Die Versionspruefung verwendet die Fahrzeugrevision, und die UI erkennt die
wirkliche Serverbestaetigung. Einsteigen und externe Inventarzugriffe verwenden
die tatsaechlich transformierten Teile statt gerundeter Zellmitten; ein
verdeckter Fuehrerstand bleibt hinter festen Waenden unzugaenglich. Rueckbau bleibt auf geladenem, freiem, geradem und
ebenem Gleis neben einer Montagestelle begrenzt.

F54 enthaelt alle 76 neuen BR52-Komponenten mit eigenen Modellen, Rezepten,
Namen und Metadaten: Kessel/Rauchkammer, Rahmen/Puffer, Raeder/Gestaenge,
Fuehrerhaus/Daecher, Tender und Anbauteile. Fenster und Tueren haben echte
Fluegel. Schwarzes Eisen, dunkler Stahl, Blech und offenes Gitter bieten je
Block-, Slab-, Treppen-, Eck- und Schraegenformen. Gemischte Slabs und alle
Bauausrichtungen verwenden passende Formen und Gitteroeffnungen. Die fehlenden
18 Holzvarianten im TMI-/Vorschauregister sind ergaenzt. Dampfventile,
Druckinstrumente und Tender-Wassertank sind als Aufbauteile gekennzeichnet;
echte Wasserversorgung und Kesseldruck bleiben unter F53 offen.

Protokoll 118 trennt kompakte Fahrtposen von Blockgeometrie; unveraenderte ruhende
Posen werden nicht wiederholt gesendet. Vorhandene native Kinematikkoerper und
Renderwurzeln bewegen sich ohne erneutes Collider-Cooking oder Meshing. Jeder
animierte Teil behaelt seinen eigenen Drehpunkt. Vorbereitete starre Matrizen
vermeiden Trigonometrie pro Formecke. Die letzte Fahrtpose bestimmt auch dann
die korrekte Collider-Uebergabe, wenn Demontage im selben Nachrichtenlauf folgt.

Zugindex ID15 speichert Fahrt, Streckencursor und Brenndauer. ID13-/ID14-Decoder
bleiben eingefroren und lesbar. Historische eingebaute Platten-/Radprofile
werden auf die neuen Grundkoerper aktualisiert; freie Formen und private
Nutzlasten bleiben erhalten.

[Bauanleitung und Steuerung](docs/train-building.md).
[CPU-Pruefbericht](tools/benchmarks/results/train-driving-br52-2026-11-08/README.md).


Abnahme 213: 4.206 Workspace-Library-Tests bestanden, keiner fehlgeschlagen;
neun bestehende Tests ignoriert. Der gezielte Zuglauf besteht mit 125 Tests.
Enthalten sind native Physikkontakte, Kollisionen ohne erneutes Cooking bei
Fahrt, Sichtschutz am Fuehrerstand, aktive Gleiswege, Kurven/Rampen, private
Ladung und Brennstoff sowie Montage nach Fahrt ueber echtes UDP. Die Asset-
Pruefung besteht fuer 84 Zugmodelle und 415 eindeutige Rezepte. Keine Grafiktests,
Spielclients, Spielfenster, Screenshots oder GPU-Abnahmen gestartet.

Der aktuelle vollstaendige Workspace-Build von Client und Server besteht
(8 min 55 s), ebenso Clippy aller Targets mit `-D warnings`, Format und Diff.
Die Programme wurden zur Abnahme nicht gestartet.


## 214. Dunkle Inventarvorschauen Blockieren Den Start Nicht Mehr

Die Vorschaupruefung akzeptiert sichtbare schwarze Metallteile anhand ihrer
Alpha-Flaeche. Bisher verlangte sie zusaetzlich einen RGB-Kanal ueber 8; dadurch
konnten dunkle Lokdaecher, Nietenplatten und Metallformen trotz vorhandener
Geometrie als leer gelten. Nach zehn Sekunden wurde der Fehler an den
Ladebildschirm gemeldet und verhinderte den Menuewechsel.

Der transparente Kamerahintergrund und die vorhandene Mindestflaeche von mehr
als 64 Pixeln bleiben die Grundlage der Erkennung. Schwarze deckende Pixel
zaehlen jetzt als Inhalt. Vollstaendig transparente Bilder, fast transparente
Pixel und zu kleine Stoerflecken werden weiterhin abgelehnt; echte Asset- und
Readbackfehler bleiben sichtbar.

Die Regression prueft RGB-unabhaengige Flaechen und den wirklich registrierten
Screenshot-Rueckgabehandler mit synthetischen Bilddaten, einschliesslich
Veroeffentlichung, Ladefortschritt, Abschluss und Fehlerfall. Es wird kein
Screenshot aufgenommen und kein Grafikbackend gestartet. Die native
Asset-Pruefung kontrolliert zusaetzlich Dreieckswindung und projizierte
Flaechen aller 84 Zugmodelle aus dem vorhandenen Vorschauwinkel.

Abnahme 214: Alle 52 Vorschau-Tests bestehen, einschliesslich der zwei neuen
Regressionen. Clippy aller Workspace-Targets mit `-D warnings` und die
erweiterte Asset-Pruefung sind sauber.
[CPU-Pruefbericht](tools/benchmarks/results/dark-inventory-previews-2026-11-08/README.md).

Der vollstaendige Workspace-Build besteht einschliesslich Linking (2 min 30 s).
Keine Grafiktests, Spielfenster, echten Screenshots oder GPU-Abnahmen gestartet.


## 215. Zugachse Aus Drehgestellen Und Gruene Lok-Aufbauvorlage

Die Montage verwendet die durch Drehgestelle vorgegebene Achslinie. Ein anders
rotierter Fahrgestellwuerfel darf die Montage nicht mit einer falschen
Gleismeldung verhindern. Ein kopierter Spielstand zeigte genau diesen Fall:
zwei passende Drehgestelle mit vier Metern Abstand ueber einem durchgehenden
gezeichneten Gleis, dazu ein um 90 Grad anders gedrehter Fahrgestellblock.
Die Achse wird jetzt aus Drehgestellpositionen und -ausrichtung bestimmt;
Aussehen, Blockposen und private Nutzlasten bleiben bei Montage erhalten.
Historische vorzeichenbehaftete Fahrbasen und gespeicherte Neigung bleiben
kompatibel, wenn die bisherigen Achsen bereits zusammenpassten.

Die Gleispruefung verwendet beide realen Achskontakte und bezieht benachbarte
Gleiseigentuemer ein. Projektion auf begrenzte Streckenabschnitte vermeidet
Abtastluecken auf langen oder ungleichmaessig parametrisierten Geraden.
Fehlendes Gleis, falsche Achslinie und ein Abstand ausserhalb von 3–15 Metern
werden getrennt gemeldet. Neue Fehlercodes werden mit Protokoll 119 uebertragen.

Die normale Lokauswahl zeigt neben dem unveraenderten funktionalen Grundgeruest
auch Kessel, Schornstein, Fuehrerhausseiten und Dach als optionalen Aufbau.
Beim Standardabstand umfasst die Vorlage 39 Zellen; nach 13 fertigen Grundteilen
bleiben 26 gruene Aufbauplaetze sichtbar. Nach Neustart oder Ausblenden wird ein
vorhandenes Fahrgestell gegenueber einem naeheren leeren Gleis bevorzugt.
Vorlagen bleiben bei Ablehnung und nicht zugehoerigen Fahrzeugaktionen erhalten.
Auch bei vorzeitig geschlossenem Fenster wird der Montageabschluss noch
verarbeitet und entfernt dann die Platzhalter.

Der Vorschlag laesst Fahrerplatz, Einstieg und Kopfraum frei. Die Sohlbank des
passenden Fuehrerhausfensters wird auf 0,08 m gesenkt, damit die vorhandene
Sitzhoehe freie Sicht durch den vorgeschlagenen Aufbau ermoeglicht.

Abnahme 215: Alle 4.218 Workspace-Library-Tests und 135 gezielte Zugtests
bestehen, keiner fehlgeschlagen; neun bestehende Tests ignoriert. Der konkrete
Aufbau, vier Richtungen, gezeichnete und native Gleise, unveraenderte
Blockzustaende beim Rueckbau, UI-Klickhandler und 26 echte gruene Platzhalter
sind als CPU-Faelle abgedeckt.
[Pruefbericht](tools/benchmarks/results/train-axis-body-guide-2026-11-08/README.md).

Der vollstaendige Workspace-Build besteht einschliesslich Linking (7 min 37 s),
ebenso Clippy aller Targets mit `-D warnings`, Format und Diff. Keine
Grafiktests, Spielfenster, echten Screenshots oder GPU-Abnahmen gestartet.


## 216. Zuglager Und Motor Per Klick Bedienen

Die Zugoberflaeche erkennt Slotklicks auch auf verschachtelten Bildern und
Stapelzahlen. Die Laderaumauswahl verwendet bei fehlender Listenauswahl den
aktuell besetzten Wagen. Der Spieler kann Inventarstapel direkt verschieben
und bereits gehaltene Stapel auf passende leere Plaetze legen. Gehaltene
Gegenstaende werden angezeigt; Nichtbrennstoffe koennen aus der Motoransicht
ins Spielerinventar zurueckgelegt werden.

Oeffentliche Fahrzeugrevisionen fuer Fahrt, Bremse oder andere Behaelter
verwerfen die private Inventaransicht nicht mehr. Private Brennstoffmeldungen
aktualisieren den offenen Motor auch bei der ersten Entnahme. Veraltete
Transaktionen werden weiterhin vom Server abgelehnt; die Oberflaeche fordert
den aktuellen Stand an und wiederholt den Klick hoechstens einmal, solange
Verbindung, Besitzer, Behaelter und angeklickte Gegenstaende unveraendert sind.
Schliessen, ein geaenderter Quellstapel oder Zieltyp brechen die Wiederholung ab.
Die Inventare werden erst nach Serverbestaetigung veraendert.

Abnahme 216: Alle 4.228 Workspace-Library-Tests bestehen, keiner fehlgeschlagen;
neun bestehende Tests ignoriert. Dazu gehoeren native Button-/Bild-/Zahlenklicks,
gehaltene Kohle und Nichtbrennstoffe, begrenzte Wiederholung und deren Abbruch,
private Cache-Invalidierung sowie leeren Motor befuellen, Fahrt, Verbrauch und
Ladungserhalt beim Rueckbau ueber echtes UDP.
[Pruefbericht](tools/benchmarks/results/train-cargo-coal-click-2026-11-08/README.md).

Der vollstaendige Workspace-Build besteht einschliesslich Client-/Server-Linking
(4 min 09 s), ebenso Clippy aller Targets mit `-D warnings`, Format und Diff.
Keine Grafiktests, Spielfenster oder GPU-Abnahmen gestartet.


## 217. Gerenderte Zug-Inventare Bedienen Und Einheitlich Skalieren

Dynamische `[id]`-Bindings erzeugen in der verwendeten TiltUI-Version keine
ElementId. Die bisherigen Callbacktests erzeugten diese IDs von Hand und
umgingen dadurch den Fehler. Slots, Fahrzeugzeilen und Fahrhebelstufen verwenden
jetzt statische Aktions-IDs und den vorhandenen typisierten `$set`-Mechanismus.
Die Slot-Regressionen pruefen diesen Store-/Handlerweg.

Motor und Laderaum verwenden die 33,6-px-Zellen und dieselbe responsive
Inventarvergroesserung wie das Spielerinventar. Ein festes Raster haelt neun
Spalten auch bei Rundung und DPI-Skalierung zusammen. Rahmen, Innenabstand und
Zellhintergrund passen zum normalen Inventar.

Die explizite Live-Fixture startet die echte gerenderte Zugoberflaeche und
klickt durch Bevy-Picking auf materialisierte Controls. Sie prueft Einlegen
und Zurueckholen von 128 Kohle sowie sieben normalen Lagerbloecken. Antworten
werden fuer diese isolierte UI-Fixture mit der echten Inventartransaktion
gebildet; die bestehenden Zugtests pruefen zusaetzlich echte UDP-Autoritaet.
Spielstaende werden fuer die UI-Fixture nicht geladen.

Abnahme 217: Vier native UI-Klicks und Mengenpruefung erfolgreich, 145 gezielte
Zugtests bestanden. Client-Build mit Linking und Clippy aller Workspace-Targets
mit `-D warnings` bestehen. Der Live-Client beendet sich nach erfolgreicher
Pruefung; Aufnahmen und Logs stehen im
[Pruefbericht](tools/benchmarks/results/train-inventory-native-2026-11-08/README.md).


## 218. Zugbewegung Glaetten Und Baupruefungen Begrenzen

Zugkoerper, Laufwerk und lokaler Fahrer verwenden eine gemeinsame geglaettete
Darstellung der bestaetigten Netzwerkbewegung mit identischem Zeitpunkt je Frame. Neue Pakete setzen am aktuellen
Darstellungsstand an; Winkel folgen dem kurzen Weg ueber die Nullgrenze.
Teleports, grosse Spruenge und laengere Empfangspausen setzen die Darstellung
zurueck. Die Fahrerkamera verwendet die Sitzposition direkt und fuegt keine
zweite Interpolation aus dem Physiktakt hinzu.

Die konservative Zughuelle wird aus unveraenderter lokaler Geometrie
wiederverwendet. Fahrt, Inventarrevisionen und entfernte Blocksetzungen erzeugen
keine erneute komplette Formberechnung. Die genauen Formen bleiben fuer die
anschliessende Kontaktpruefung erhalten. Geometrieaenderungen besitzen einen
anderen Cache-Schluessel; der Cache bleibt auf 64 Eintraege begrenzt.
Kollisions-Snapshotvorbereitung erhaelt zusaetzlich ein Framebudget von zwei
Millisekunden zwischen Abschnitten, damit Bauereignisse verteilt werden.

Neue gezeichnete Gleisboegen verlangen 16 Meter horizontalen Kurvenradius;
gerade Rampen behalten ihre bisherige Regel. Die Validierung alter gespeicherter
Gleiskurven bleibt unveraendert. Protokoll 120 verhindert eine Verbindung mit
Clients/Servern, die noch andere Bauvorschlaege berechnen. Zugindex ID15 bleibt
unveraendert.


Abnahme 218: Alle 4.235 Workspace-Library-Tests bestehen, neun bestehende Tests
bleiben ignoriert. Client-/Server-Build mit Linking, Formatpruefung und Clippy
aller Targets mit `-D warnings` bestehen. Der native Bautest misst 114
Platzierungsframes mit maximal 12,60 ms und 66,61 ms asynchroner Zeit bis zum
bestaetigten sichtbaren Block. Der isolierte Lauf enthaelt keinen fahrenden Zug;
Fahrt, Kamera und UDP-Verhalten werden automatisiert geprueft.
[Pruefbericht und Aufnahme](tools/benchmarks/results/train-motion-build-curves-2026-11-08/README.md).


## 219. Zugwerkstatt, Prellboecke, Kessel, Beschriftung Und Licht

Prellboecke besitzen eine feste Halteebene vor dem Anschlag. Bremsvorschau und
Bewegungsfreigabe beruecksichtigen den fuehrenden Wagenkasten und die Kupplung,
auch bei mitgezogenen Wagen und Rueckwaertsfahrt. Wegfahren vom Anschlag bleibt
moeglich. Ein blockierter Verbund wird vor der gemeinsamen Veroeffentlichung
angehalten.

Die Werkstatt baut ihre sichtbare Vorlage auf Knopfdruck. Der Server prueft
Gleislage, geladene Zellen, Bau-/Inventarrechte, Figuren, Fahrzeuge und
Materialbestand. Nur fehlende passende Teile werden gesetzt und bezahlt;
Creative benoetigt keine Materialien. Sparse Sections, Geometrieepochen und
Inventar werden gemeinsam veroeffentlicht; die frische Inventarantwort wird
auch bei Bauaktionen direkt repliziert. Rahmen, Kessel und Verkleidung bleiben
anschliessend normal editierbare Weltbloecke.

Zugnamen mit bis zu 24 druckbaren Zeichen werden im Namensfeld bearbeitet und
besitzgeprueft uebertragen. Nummernschild-Metall und Schrift liegen getrennt;
Namensaenderungen ersetzen die kleine Schriftgeometrie ohne Wagenneubau.
Zugindex ID16 uebernimmt ID13-/ID14-/ID15-Daten ueber eingefrorene Decoder.
Client und Server verwenden Protokoll 121.

Auf einem drei Bloecke breiten Zugrahmen erhaelt ein neu gesetzter Kessel eine
persistente 3x3-Querschnittspose. Eigene volle Verkleidung auf dem Rahmen wird
mitberuecksichtigt. Modell, Normalen, Tangenten, Auswahl, Hintergrundkollision,
Randabschnitte und eingefrorene Fahrzeuggeometrie verwenden dieselbe Pose.

Lok-Scheinwerfer erzeugen warme gerichtete Spotlights; die neue herstellbare
Wagen-Deckenleuchte strahlt nach unten. Bewegte Lampen erben den dargestellten
Wagentransform. Maximal zwoelf nahe Lampen und 32 Schilder begrenzen die
Zusatzarbeit. Die neue entfernungsgerechte Stereo-Klangmischung verwendet
deterministisch synthetisierte Lok-/Gleisklaenge und die vorhandene Ofenaufnahme.
Rhythmus und Laufklang folgen Geschwindigkeit und bestaetigtem Motorzustand;
Master-, Ambiente- und Stummschaltung werden beruecksichtigt.

Beim Streaming werden kalte Kurven vor ihrer unveraenderten Importvalidierung
im Hintergrund vorbereitet. Ein Zwei-Millisekunden-Budget verteilt die
anschliessenden Kontrollimporte. Kleine Belegungsdaten bleiben in einem
separaten begrenzten Cache erhalten, auch wenn die schweren Geometrie-/Meshprofile
aus dem Geometriecache verdraengt wurden.

Abnahme 219: 4.465 Workspace-Tests bestanden, zehn bestehende Tests ignoriert;
beide gezielten Schrift-/Lichttests nach der letzten Geometriekorrektur erneut
bestanden. Client-/Server-Build mit Linking, Clippy aller Targets mit
`-D warnings`, Formatierung und Diff-Pruefung bestanden. Die native
Montageansicht wurde in 1080p und 720p aufgenommen; Namensfeld, Baubutton und
unterer Schliessen-Button liegen innerhalb des eingepassten Rahmens. Keine
manuelle Fahrt oder Messung der bisherigen 0,4-s-Aussetzer in der Problemwelt.
[Pruefbericht und Aufnahmen](tools/benchmarks/results/train-workshop-stops-lights-2026-11-09/README.md).


## 220. Drehgleis, Modellbreiten Und Neue Zug-Montageansicht

Die Montageansicht folgt der neuen Referenz mit Fahrzeugtypen links, echter
Modellvorschau und Drehgestell-Abstand in der Mitte, Name und Bauaktionen rechts
sowie Fahrzeugliste, Fahrpult und Aktionen darunter. Der untere Schliessen-/ESC-
Button entfaellt. Eine einzelne begrenzte Vorschaukamera rendert die vorhandenen
Bauteilmodelle in ein eigenes 1024x576-Ziel; Hintergrundaufbau und Studio-
Umgebungslicht erhalten Metalloberflaechen ohne Aenderung der Weltmaterialien.

Die Auswahl 1x/3x bestimmt die Vorlagenbreite. Breite Vorlagen erhalten seitliche
Rahmen und den 3x3-Kessel, schmale Vorlagen bleiben einblockbreit. Die gemeinsame
Spurweite bleibt erhalten. Die Serveraktion rekonstruiert dieselbe Breite und
berechnet ihren echten Materialbedarf. Gleise und Zugbauteile besitzen den
TMI-Tab Trains. Zug-Inventare verwenden dieselben auf Bildpunkte ausgerichteten
Slotmasse wie andere Inventare und eine begrenzte Vergroesserung.

Das neue herstellbare Drehgleis besitzt eine kreisfoermige Plattform und einen
25-Meter-Fahrweg. Ein gestoppter, gebremster und entkuppelter Zug wird gemeinsam
mit dem Gleis um 90 Grad gedreht. Auch leere Plattformen lassen sich an der
Montagestelle ausrichten. Der gesamte Drehraum wird einschliesslich der
Kreisboegen zwischen den Proben auf Hindernisse, Personen, Fahrzeuge und Rechte
geprueft. Abgewiesene Aktionen aendern weder Plattform noch Fahrzeug.

Die Reparatur von Gleisen unter stehenden Zuegen prueft die wirklichen Formen
anstelle einer ganzen reservierten Blockzelle. Retirierte Gleisverweise werden
an der unveraenderten Fahrzeugposition neu gebunden. Unveraenderte Gleismodule
teilen zwischengespeicherte Vollstaendigkeitspruefungen; Schienenkontakt-Abfragen
vermeiden temporaere Zellvektoren. Pro Frame werden hoechstens zwei fertige
Terrain-Kollisionen aktiviert. Monotone Interpolation erhaelt konstante
Koordinaten beim Schneiden von Gleisflaechen an Blockgrenzen.

Client und Server verwenden Protokoll 122; Zugindex ID16 bleibt unveraendert.

Pruefbericht: [Phase 220](tools/benchmarks/results/train-turntable-width-ui-2026-11-09/README.md).

Verifiziert: 4474 Workspace-Tests bestanden, 10 regulaer ignoriert; Build,
Clippy mit -D warnings und Formatierung erfolgreich. Native Montage-/Fahransicht
bei 720p/1080p sowie vier echte Motor-/Lager-Klicks geprueft. Nativer Bau-/
Abbautest: 139 Platzierungsframes, Median 6.84 ms, P95 9.98 ms, Maximum 48.44 ms.
Dies ist eine isolierte Messung ohne Vorher-/Nachher-Vergleich im Problemspielstand;
sie belegt keine allgemeine Freiheit von Streaming-Haengern.


## 221. Dampfstufen, Hoeheres Fuehrerhaus Und Gleisprofil-Wiederverwendung

Die neue breite Bauvorlage setzt den Fuehrerstand ueber den 3x3-Kessel. Ein
vollstaendiger dunkler Unterbau, Treppen, nach aussen gesetzte Seitenwaende und
Frontfenster schaffen Sicht und Innenraum. Duennwaende lassen sich anhand der
angeklickten Kante innen oder aussen ausrichten. Bestehende Aufbauten werden
nicht umgebaut; die aktualisierte Vorlage steht nach Montage bereit.
Schornsteine und Dome erhalten eine begrenzte sichtbare Anschlusssockel-Geometrie.
Schwarzstahl, Messingdetails und roter Rahmen naehern die Grundmodelle der
Referenz an. Fahrende befeuerte Loks erzeugen begrenzten Schornsteinrauch.

Die sechs Fahrstufen entsprechen 4, 10, 16, 20, 30 und 50 Metern pro Sekunde vor
dem vorhandenen Schotterbonus. Stufe 4 verlangt mindestens vier zusammenhaengende
Kesselmodule, Stufe 5 sechs. Stufe 6 verlangt zusaetzlich Motor, Ventil, Anzeige,
Tank, Wasser und mindestens acht bar. Brennstoff heizt den Kessel; Dampfbetrieb
verbraucht Wasser. Rueckwaerts stehen relativ zum Fuehrerstand zwei Stufen zur
Verfuegung. Der Tank nimmt echte Wassereimer auf und gibt leere Eimer zurueck;
Creative fuellt ihn vollstaendig. Bei Demontage kommen volle Wassereimer zurueck,
Restwasser und Druck werden abgelassen. Wasser und Druck sind gespeichert und
werden mit kompakten Bewegungsdaten synchronisiert. Die Rohr-/Kinetikanbindung
von F53 bleibt in TODO.md offen.

Die Speicherung erlaubt nun Brennstoff- und Restbrandzustaende auf dem Zugmotor
218. Der vorherige Validator behandelte diesen Zustand beim Speichern als
ungueltigen Ofen. Neue bedingte Bevy-Systeme erhalten einen korrekten initialen
Aenderungstick; Warnfilter und bestehende Systemticks werden nicht umgestellt.

Vorbereitete Gleisflaechen werden zwischen Chunk-Aufbaujobs geteilt. Gleichzeitige
kalte Anfragen teilen eine Berechnung; der Cache begrenzt Eintraege und erfasste
Vertex-/Index-Nutzdaten. Form, Holz und Materialzuordnung bleiben Teil des
Schluessels. Dadurch wird unveraenderte Gleisgeometrie bei Terrainbearbeitungen
nicht fuer jeden betroffenen Abschnitt erneut geschnitten.

Client und Server verwenden Protokoll 123. Zugindex ID17 migriert ID16 und
uebernimmt die bisherigen aelteren Zugmigrationen.

[Bedienung](docs/train-building.md) und
[Pruefbericht](tools/benchmarks/results/train-steam-cab-performance-2026-11-09/README.md).

Der integrierte Server fuehrt seine synchronen Systeme auf dem eigenen
Betriebssystem-Thread aus. Dadurch koennen parallele Renderer-Abfragen keine
langen Server-Simulationsaufgaben aus dem gemeinsamen Compute-Pool uebernehmen.
Wasserverbindungen pruefen die freie Flaeche an der gemeinsamen Zellseite;
aufwendiges Zerschneiden beider gesamten Schienen-Wasservolumen entfaellt.
Die Darstellung nutzt weiterhin dieselben realen Festkoerperformen. Rauch folgt
der interpolierten Fahrzeugpose, damit er auch bei schneller Fahrt am
Schornstein beginnt.

Verifiziert: 4488 Workspace-Tests bestanden, zehn ignoriert; Build, strenges
Clippy und Formatierung erfolgreich. Native Montage in 720p/1080p, vier echte
Inventarklicks und Modell/Rauch geprueft. Drei 25-Sekunden-Messungen in frischen
Kopien der Test-Welt: Blockbau maximal 27.23 ms, Gleisbau 28.62 ms, Streaming
ueber rund 237 Meter 25.16 ms pro Frame. Die urspruenglichen acht Weltdateien
sind unveraendert. Diese Stichproben sind keine allgemeine Framerate-Garantie.

## 222. Sicheres Zug-Aussteigen, Weltbeitritt Und Isolierte Montagevorschau

Die Ausstiegssuche erreicht auch freien Boden neben breiten, erhoehten Kabinen;
Terrain und andere Fahrzeuge blockieren weiterhin den geprueften Uebergang.
Waehrend des Sitzens beziehungsweise eines noch offenen Ausstiegs bleibt die
letzte stehende Speicherposition erhalten. Alte Sitzpositionen werden vor dem
Weltbeitritt gegen echte Zugformen geprueft und in freien Raum korrigiert, statt
nach dem Laden mit „Spawn changed before terrain became ready“ abgewiesen zu
werden. Das Fahrpult hat einen expliziten Aussteigen-Button.

„Zug zerstoeren“ entfernt nach zweiter Bestaetigung Fahrzeug und Ladung dauerhaft.
Der Besitzer muss in Reichweite sein; Stillstand, freie Sitze, geloeste Kupplungen,
Revision und Baurechte werden serverseitig geprueft. Der eigene Fahrer steigt
vorher aus. Zuverlaessige Loeschmarkierungen verhindern auf Replikaten das Warten
auf eine nicht stattfindende Blockwiederherstellung. Montage bleibt die Aktion
zum Wiederherstellen bearbeitbarer Bloecke. Client und Server: Protokoll 124;
bestehende Saveformate bleiben unveraendert.

Die neue 3x-Vorlage erhoeht Sitz, Boden, Fenster und Dach um weitere zwei Bloecke
und verbindet den Zugang mit einer hinteren Treppe. Eigenbauten werden nicht
ungefragt umgebaut. Die Montagekamera nutzt Ebene 25 statt der mit der
Weltminiatur geteilten Ebene 28; Terrain-/Grasbloecke geraten dadurch nicht mehr
in die Vorschau. Gefressene Bodenwachstums-Grasbueschel werden nicht mehr in den
fuer sie ungueltigen allgemeinen Futter-Nachwuchstimer eingetragen.

4496 Workspace-Tests bestanden, 10 ignoriert; ein separat fehlendes originales
FIVERR-GLB-Referenzasset wurde gezielt ausgefiltert, dessen Loeschung blieb
unangetastet. Strenges Clippy, Build, Formatierung und Whitespace-Pruefung bestanden.
Native Montage-UI samt echter Maus-Bestaetigung in 1080p/720p und isolierten
Render-Ebenen geprueft. Die aktuelle Test-Welt zweimal nativ ueber ihren echten
Integrated Server geladen, dazwischen gespeichert; acht Originaldateien per
SHA-256 unveraendert. Reale UDP-Tests pruefen auch alte Sitz-Saves und Loeschung
bei Besitzer sowie Beobachter. Details und Screenshots:
[Train Exit / Rejoin / Preview](tools/benchmarks/results/train-exit-rejoin-preview-2026-11-09/README.md).

## 223. Kontinuierliche Kupplungen, Ruhigere Zugdarstellung Und Bergfried-UI

Kupplungen pruefen echte Weltpositionen und kontinuierliche Ausrichtungen statt
gerundeter Blockzellen. Dadurch funktionieren auch passende Zwischenpositionen
auf Kurven und Steigungen. Bei der zweiten Fahrzeugauswahl werden kompatible
freie Enden vorgewaehlt; die manuelle Auswahl bleibt erhalten. Die hintere
Kupplung der Bauvorlage zeigt fuer alle vier Baurichtungen nach aussen.

Ein begrenzter 60-ms-Darstellungspuffer gleicht unregelmaessige Paketankuenfte aus.
Wagenkasten, Raeder und Mitfahrer teilen denselben Darstellungszeitpunkt.
Spruenge setzen den Puffer zurueck; unbekannte Strecke wird nicht extrapoliert.
Client und Server verwenden Protokoll 125, Speicherformate bleiben unveraendert.

Die 3x-Lokvorlage hat grosse rote Speichenraeder mit 1,7 m Durchmesser,
Stahlreifen, Gegengewichten und Kurbelzapfen. Auch kleine Raeder und Drehgestelle
verwenden die ueberarbeitete Radgeometrie. Animation, Vorschau und Kollisionsform
beruecksichtigen den neuen Radius; Kurbelzapfen und Gestaenge drehen gleichsinnig.
Individuelle Fahrzeuge werden nicht automatisch umgebaut.

Der Bergfried zeigt Uebersicht, Mitglieder/Rechte und Diplomatie in einem
Kupfer-/Braunrahmen mit Wappen, Aufwertungskosten, Landzaehlern und Claimaktionen.
Symbolsuche, Farbwahl, Rollen und Aufloesen bleiben echte bedienbare Aktionen.
Kleine Fenster verwenden eine verdichtete Uebersicht. Der Karten-Diplomatietooltip
hat begrenzte Schriftgroesse und Textumbruch; ungueltige Aktionsicons sind ersetzt.

4500 Workspace-Tests bestanden, 10 ignoriert, ein bereits separat geloeschtes
FIVERR-Referenzasset gezielt ausgefiltert. Strenges Clippy, Build, Formatierung,
Whitespace und CPU-Lint aller 85 Zugteile bestanden. Native Bergfriedpruefung
mit echtem Server, Rollenwechsel und Visitor-Reconnect in QHD/1024x768/4K;
Montage in 1080p/720p und Rad-/Rauchdarstellung geprueft. Die nativen Zugbilder
sind Darstellungsfixtures; sie ersetzen keinen Langzeit-Fahrtest.
[Pruefbericht](tools/benchmarks/results/train-coupling-running-gear-bergfried-2026-11-09/README.md).

## 224. Fahrwerkskorrektur, Kuppeln Ohne Station Und Ausstieg Auf Wagenboeden

Die grossen Treibraeder stehen auf derselben 2-m-Spur wie die Drehgestelle.
Bei damit ausgestatteten Fahrzeugen werden die kleinen Drehgestellraeder nicht
zusaetzlich gerendert. Alte gespeicherte Treibrad-Kollisionsprofile verwenden
ebenfalls die korrigierte kanonische Form. Dadurch bleiben Darstellung und
Streckenfreigabe auch fuer vorhandene Aufbauten konsistent.

Koppeln und Loesen funktionieren aus der Fahrzeug-Montageansicht ohne gesetzte
Montagestelle. Der bisherige fruehe Ruecksprung ohne Serveranfrage entfaellt.
Besitz, Baurechte, Stillstand, Revision und entweder Naehe oder Bindung an eines
der Fahrzeuge werden serverseitig geprueft. Client und Server: Protokoll 126;
Speicherformate bleiben unveraendert.

Die gemeinsame Ausstiegssuche beruecksichtigt einen seitlichen Weg aus hohen
Kabinen und freie, physisch vorhandene Wagenboeden. Ein trockener Terrainboden
ist damit nicht mehr die einzige erlaubte Landeflaeche. Hindernisse, andere
Fahrzeuge und ungeladene Bereiche bleiben gesperrt. Eine Kopie der aktuellen
Test-Welt reproduziert den nahen Wagen; der Ausstieg funktioniert jetzt auch bei
1 m Kupplungsabstand und an allen Sitzen des Personenwagens.

Vor dem Hauptmenue werden Modelle, Texturen, Schriften, Audio und UI geladen.
Die 617 Inventarvorschauen werden standardmaessig bei Bedarf gerendert.
Ein kontrollierter nativer Startvergleich mit denselben Einstellungen benoetigte
18,45 s mit vollstaendigem Vorschaukatalog und 5,77 s mit der neuen Vorbereitung.
Das sind einzelne Start-Smoke-Messungen, keine allgemeine Ladezeitgarantie.

4506 Workspace-Tests bestanden, 10 ignoriert; das separat fehlende originale
FIVERR-GLB-Referenzasset bleibt gezielt ausgefiltert. Strenges Clippy, Build,
Formatierung, Whitespace und Asset-Lint bestanden. Details und native Belege:
[Fahrwerk, Ausstieg Und Startzeit](tools/benchmarks/results/train-clearance-dismount-startup-2026-11-10/README.md).

## 225. Hoehere Inventare, Vollstaendiges Icon-Preload Und Flache Weichen

Inventare mit Item-Browser erhalten 20 Prozent mehr logische Hoehe:
Spielerinventar 439,2 statt 366 px, Geraete 480 statt 400 px. Slots bleiben
33,6 px gross, Ergebnisfelder 38,4 px. Die responsive Begrenzung laesst Titel
und HUD sichtbar und vergroessert Schrift beziehungsweise Icons nicht mit.

Alle 617 endlichen Inventarvorschauen werden wieder vor dem Hauptmenue geladen.
16 getrennte Renderplaetze bearbeiten begrenzt mehrere Vorschauen gleichzeitig;
jede besteht weiterhin die GPU-Pruefung auf sichtbare Pixel. Modelle, Texturen,
Schriften, UI und Audio bleiben im Preload. Der native Vergleich benoetigt bei
gleichen Einstellungen 9,032 statt 19,148 s mit serieller Vorbereitung. Das sind
Einzelmessungen mit warmen Caches inklusive Aufnahmen und Prozessende.

Neue Weichen haben einen 20 m langen S-Abzweig mit 4 m Seitenversatz und maximal
rund 17,1 Grad Richtungsabweichung. Beide Ausgaenge bleiben parallel. Fahrpruefungen
mit 8 und 15 m Radstand passieren beide Wege linker und rechter Weichen mit
gepruefter Bewegung und Kollisionsfreiheit. Vorhandene alte 90-Grad-Weichen
behalten ihre Anschluesse; fuer den flachen Verlauf muessen sie mit mehr Platz
neu gebaut und ihre Anschlussgleise angepasst werden. Umschalten, Replikation
und Vorschau verwenden dasselbe Profil; Gleis-Assets wurden daraus neu exportiert.

Gleisbau und einzelne Verbindungen reichen nun bis 64 m. Nahe freie Endpunkte
rasten auch beim Zielen knapp daneben auf den Boden ein. Client und Server
rekonstruieren denselben Bogen und pruefen weiterhin die kompletten angeschlossenen
Module auf Baurechte. Viertelmeter-Unterteilung bleibt bei begrenztem Budget erhalten.
Client und Server verwenden Protokoll 127; persistente Schemas bleiben unveraendert.

4512 Workspace-Tests bestanden, 10 ignoriert, eine separat fehlende originale
FIVERR-GLB-Datei ausgefiltert. Native Layoutpruefungen bestehen mit 52 Aufnahmen
ueber breite, kompakte und 4K-Ansichten. Original-Spielstaende wurden nicht
migriert. Details: [Inventare, Preload Und Weichen](tools/benchmarks/results/inventory-preload-shallow-switches-2026-11-10/README.md).
