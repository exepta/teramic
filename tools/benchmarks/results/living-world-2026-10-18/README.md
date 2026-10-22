# Phase 59: Fackeln, Lebendige Tierwelt Und Ziel-HUD

## Architektur

Keine Generator-, Seed-, Block-ID- oder Dependency-Aenderung. Bestehende
Tierpopulation, Prop-IDs, Section-Batches und EditJournal werden erweitert.
Client und Dedicated Server verwenden Netzwerkprotokoll **33** und muessen
gemeinsam aktualisiert werden. Regionsschreiber **REG8**, kompatible Leser
fuer REG7 und alle zuvor unterstuetzten Formate; Spielerdateien bleiben PLY3.
Der kleine Weltindex nutzt IDX3 mit Lesern fuer IDX2/INDX. Die laufende
Oekologie-Uhr steht ausschliesslich im Index; dadurch werden unveraenderte
Tier-/Blumenregionen nicht bei jedem Speichern wegen eines Zeitstempels neu
geschrieben. Regionseigene Nektar-Deadlines bleiben relativ zu dieser Uhr.
Nektar, Regenerationszeit, Nestvorrat, Reifung und Bienen-Zuordnung liegen in
den vorhandenen binaeren Regionsdateien, nicht in einer zusaetzlichen TOML.

`assets/terra/mobs/{sheep,cow,pig,fish,bee}.json` konfiguriert Namen, Leben,
Beduerfniskapazitaeten, Abmessungen, Geschwindigkeit, Gruppengroesse und
Schlafverhalten. Ungueltige Definitionen fallen mit einer Meldung auf
eingebettete Defaults zurueck. Aenderungen benoetigen einen Neustart;
gespeicherte Beduerfnisse werden beim Laden an reduzierte Grenzen angepasst.
Die Clientdarstellung skaliert die wiederverwendeten Modelle entsprechend.
Client und Server benoetigen dieselben Gameplay-Assets.

## Fackeln Und Schatten

Die beiden Feuer-Materialslots verwenden eine animierte Variante des
vorhandenen ExtendedMaterial-Vertexpfads einschliesslich Motion-Prepass.
Eine Material-Key-Spezialisierung kompiliert den Feuerzweig nur fuer diese
Slots, nicht fuer normales Windlaub oder Pilzfasern.
Feuer ist selbstleuchtend und unbeleuchtet darstellbar; vorhandene budgetierte
PBR-Punktlichter beleuchten weiterhin den 18-m-Bereich. Keine Flame-Entity je
Voxel und kein neuer Fullscreen-Pass. Das Lampenbudget bleibt qualitaetsabhaengig.

Bodenmontage und vier geneigte Wandposen teilen eine Transformation fuer
Modell, Auswahlvolumen und Lichtposition. Mounts liegen im vorhandenen
BlockObjectState (Facing 4..7); Moebel akzeptieren weiterhin nur Facing 0..3.
Serverseitig validierte Trefferflaechen bestimmen die Pose. Deckenmontage ist
nicht erlaubt. Entfernen eines Stuetzwuerfels entfernt die angehaengte Fackel
ueber denselben Editpfad und erzeugt einen vorhandenen 3D-Itemdrop.

Die Schattenproxy lag bisher auf der kanonischen Blockaussenflaeche, waehrend
das sichtbare Relief bis .06 m nach innen reicht. Dadurch konnte die Proxy
eigene Vertiefungen schwarz verdecken. Die Innenflaechen opaquer Proxies liegen
nun .065 m hinter ihrer Flaeche; vier schmale Randflaechen verbinden sie mit
den unveraenderten kanonischen Aussenkanten. Einfaches Zuruecksetzen der ganzen
Flaeche wurde nach nativer Pruefung wegen Lichtlecks verworfen. Die geschlossene
Variante benoetigt acht statt vier Positionen und fuenf statt einer Schatten-
Quadflaeche pro urspruenglicher Greedy-Flaeche, weiterhin im selben Batch.
Blattmasken behalten ihre Geometrie. Punktlicht-Radius
und Bias sind angepasst. Das ist eine budgetierte Proxy-Approximation, keine
exakte Schattengeometrie jedes Reliefdetails. Hoehlen-Grundlicht steigt von
12 auf 42, ohne den Aussenbereich oder Himmels-IBL in geschlossenen Hoehlen
aufzuhellen. Tiefe Raeume bleiben ohne Fackel deutlich dunkler als draussen.

Kleinvegetation wirft keine langen gerichteten Schatten mehr. Ein gemeinsames,
gecachtes Mesh zeichnet weiche Boden-Kontakte: maximal 768 Pflanzen innerhalb
32 m, mit Entfernungsfade, maximal vier Aktualisierungen pro Sekunde und ohne
Neuberechnung bei unveraenderter Welt/Position. Keine Kontaktflecken fuer
haengende Nester oder Baumpilze. Bodenkontakte sind nicht dynamische GI.

## Tierverhalten Und Oekologie

Bienenkoerper .095 m breit/hoch und .115 m lang, separater kleiner Stachel,
Fluegel und animierter Ruessel beim Sammeln. Bis vier neu erzeugte Bienen je
Nest; bestehende Identitaeten bleiben bestehen. Alte Zweiergruppen werden
nicht durch zusaetzliche zufaellige Identitaeten ersetzt.

Blumen besitzen 100 Nektar. Ein abgeschlossener Besuch entnimmt hoechstens
25, ohne die Blume zu loeschen. Leere Blumen regenerieren nach 180 aktiven
Simulationssekunden. Bienen tragen maximal 100, bringen ab 50 zum Nest und
meiden leere Blueten. Nachts suchen sie ihr eigenes Nest, verschwinden darin
und verlassen es tagsueber. Nestkapazitaet zehn; neue Gruppen bleiben vier.
100 gespeicherter Nektar werden nach 300 Sekunden zu einer Einheit Honig,
mit 2.5 Prozent Zeitreduktion je anwesender Biene (vier: 270 Sekunden).
Reifung, Inhalt und Anwesenheit sind serverautoritativ und persistent.
Honigentnahme und Bestaeubung sind noch keine Spielmechanik.

Kuehe, Schafe und Schweine falten nachts ihre Beine und legen sich hin;
nahe Spieler wecken die Gefahrenreaktion. Die KI prueft kurze alternative
Richtungen mit Vorausschau und vorhandener Boden-/Wasserpruefung. Schwimmende
Sauegetiere erhalten Auftrieb, schwimmen zu Ufern und folgen bestehender
Stroemung. Das ist lokale Hindernisvermeidung, kein globales Navmesh/A*.
Fische springen gelegentlich ballistisch aus oberflaechennahem Wasser und
kehren in echte fluessige Zellen zurueck. Unbeladene Zellen bleiben Barrieren.
Bein-/Hufvolumen ueberlappen nicht mehr an derselben Abschlussflaeche und
liegen etwas ueber dem Boden, um Kontaktflimmern zu vermeiden.

## Wasserreaktionen Und Audio

Lokale und replizierte Spieler-/Tierkoerper, nicht die freie Kamera, erzeugen
Praesentationsereignisse fuer Wassereintritt, Oberflaechenschwimmen und Atem.
Staerke folgt Eintrittsgeschwindigkeit; Spawn und Teleport erzeugen keinen
falschen Aufprall. Fische duerfen spritzen, aber niemals Atemblasen erzeugen.
Saeugetierblasen steigen gelegentlich auf. Ein Mesh buendelt Spritzer,
Hohlringe und Spuren, mit 64/192/384 Partikeln fuer Low/Medium/High.
Diese Effekte transportieren keine zusaetzlichen Fluessigkeitszellen.

Flug ist lauter, belegte Nester summen, Ein-/Ausflug besitzt eigene Cues.
Schwimm-/Splash-Foley nutzt bearbeitete vorhandene CC0-Wasseraufnahmen;
Vogelambiente wird bandbegrenzt und leiser gemischt. Nest-Cues sind eigene
Synthese, keine als echte Aufnahmen ausgegebenen Samples. Vorhandener Mixer,
Koerper-Hoerposition, Hoehlenhall, Unterwasserfilter und Audiooptionen bleiben
erhalten. Quellen und reproduzierbare Bearbeitung in `assets/audio/CREDITS.md`.
In dieser Umgebung ist keine akustische Kopfhoerer-/Lautsprecherabnahme moeglich.

## Zielanzeige

Reine TiltUI-Komponente `target-card` oben mittig im normalen Gameplay,
ausgeblendet bei Debug, Menues, Chat und Inventar. Block-/Propname, Registry-ID,
Hand oder echtes Werkzeug-Vorschaubild, Mindeststufe, korrekter gruener Haken
oder rotes X und Abbaufortschrittsbalken verwenden vorhandenes Picking,
BlockProperties und autoritativen Mining-Fortschritt. Mobkarten zeigen Status
und Lebensbalken mit zentriertem Zahlenlabel.

`hud_stats` in einzelnen Prop-/Block-JSONs steuert `bar` oder `text` fuer
`nectar`, `honey`, `occupants`, `processing`. Fehlende Schluessel blenden die
Stat aus. Beispiele: `assets/terra/props/flower.json`, `bee_nest.json`.
Aktuelle lebende Vorratswerte gehoeren zu Blumen und Nestern; normale
Steinbloecke erfinden keinen Vorrat. F3+E zeigt bei Bienen getragenen Nektar
und den Stand der zuletzt besuchten Blume.

## Validierung

Native Vulkan-/Wayland-Pruefung auf AMD RX 6950 XT, RADV/Mesa 26.2.4,
1920 x 1080: acht Hoehlenansichten in `caves-accepted/`, darunter Low,
Medium, High, Ultra, dunkler Seitentunnel, Boden-/Wandfackel und Spritzer
ueber dem unterirdischen See. `creatures/` zeigt Nektarsammeln mit normalem
Ziel-HUD und liegende Landtiere. Die Lebenszahl ist nativ zentriert geprueft.
Die Seeansicht injiziert kosmetische Wasserereignisse fuer die Darstellung;
sie ist kein End-to-End-Nachweis eines physischen Sprungs aller Mobarten.
Wasserkontakt- und Fischsprungregeln haben separate automatische Tests.

Die handgefertigte Hoehlen-Testszene hatte anfangs fremde Terrainmeshes:
noch laufende Jobs der vorherigen Welt konnten trotz gleicher Edit-Epoche
spaeter ihre Ergebnisse hochladen. Der Fixture-Austausch verwirft nun die
betroffenen Snapshots und Jobs vor dem Einsetzen der Testsections. Ein
Regressionstest sichert gerade den Fall unveraenderter Epoche ab. Das
aendert keine Weltgenerierung und kostet im normalen Streaming nichts.

Alle 32 Audio-Clips wurden dekodiert und ihre Signale automatisch geprueft.
Mangels Audiogeraet keine behauptete akustische Qualitaetsabnahme.
Weitere GPUs, stundenlange Streaminglaeufe und reale Mehrrechner-Tests
bleiben offen. Konkrete Workspace-/Server-Prueflogs liegen neben diesem
Bericht; Kurztests sind kein Mehr-GPU-/Langzeitnachweis.

Final erfolgreich (jeweils mit unveraendertem Lockfile):

- `cargo fmt --all --check`
- `cargo check --workspace --locked`
- `RUST_TEST_THREADS=1 cargo test --workspace --locked`: 938 bestanden,
  null fehlgeschlagen; ein separat auszufuehrender 180-s-UDP-Soak ignoriert.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo build --workspace --locked`
- `env -u DISPLAY -u WAYLAND_DISPLAY ... terra-server --smoke-test`:
  eigener ephemerer UDP-Port, 27 Sections, erfolgreicher fensterloser Exit.

Die Tests decken unter anderem begrenzte Nektarentnahme/Regeneration,
Nestkapazitaet/Honigreifung, Schlaf-/Aufweckregeln, Fischsprung und Rueckkehr,
Wasserreaktionsbudgets/gueltige Mesh-Indizes, Wandpose/Auswahl, Assetvalidierung,
REG7-/IDX2-Migration und eine Oekologie-Uhr ohne Region-Neuschreibungen ab.
Ein bestehender realer Zwei-UDP-Client-Test prueft jetzt auch replizierten
Blumennektar und Nesthonig bei maximalem Spielerinteresse.

Zusaetzlich explizit ausgefuehrt: der ignorierte Zwei-Profil-UDP-Soak lief
180.005 Sekunden erfolgreich, 27749 Hostupdates und je 4625 beobachtete
Snapshots auf beiden Clients. Das ist ein lokaler Transport-/Identitaets-
Stabilitaetstest, kein realer Mehrrechner- oder Tierherden-Langzeittest.
Seine Startbedingung wartet nun auf den ersten bestaetigten Zustand beider
Clients, nicht nur auf Welcome/Ready. Der vorherige sofortige Testabbruch
ist als `udp-soak-start-race.log` erhalten. Ein parallel gestarteter Workspace-
Lauf kollidierte auf einem ephemeren Status-TCP-Port; dieser Fehlversuch
liegt in `workspace-concurrent-port-collision.log`. Der finale Workspace-
Lauf erfolgt ohne konkurrierende Netzwerktests.

## Ruhendes Waldpaar

Je 60 Sekunden, gleicher Seed 42042, Landschaftsrevision 0/Erzrevision 1,
acht Chunks Sichtweite, 10 Uhr pausiert, VSync aus, identische Kamera
(-622, 53.88851, 680), gleicher Grafikconfig und nativer Aufloesung.
Keine Compiler oder weiteren Testclients waehrend der Messung.
Der zuvor bereits laufende Benutzer-Server und Desktop blieben unveraendert.
Offline-Szene ohne aktive Tiere, Netzwerk oder Wassereintritt; deshalb kein
Population-/Netzwerk- oder Streamingbenchmark. Debug-Workspaceprofil wie
beim normalen `cargo run`, kein Release-Vergleich.

| Messung | Vorher (Phase 58) | Final (Phase 59) |
| --- | ---: | ---: |
| Durchschnitt FPS | 207.35 | 222.80 |
| p95 CPU-Wall-Frame, ms | 6.070 | 5.874 |
| p99 CPU-Wall-Frame, ms | 7.375 | 6.985 |
| Szenenbereitschaft, s | 1.948 | 1.962 |
| Terrain-Komposition, Worker-s | 1.277 | 1.283 |
| Prop-Komposition, Worker-s | 0.195 | 0.184 |
| Peak RSS, KiB (externer Sampler) | 772248 | 757604 |
| Peak DRM VRAM, KiB | 2137148 | 1875000 |
| Peak DRM GTT, KiB | 69780 | 69780 |
| Geladene / gerenderte Sections | 2041 / 875 | 2041 / 875 |
| Mesh-Assets | 3521 | 3524 |
| Sichtbare Mesh-Entities | 1745 | 1672 |
| Repraesentierte Props / Batches | 25808 / 328 | 25808 / 328 |
| GPU Early Prepass Mittel, ms | 0.326 | 0.395 |
| GPU Main Opaque Mittel, ms | 1.036 | 1.018 |

Kein allgemeines Beschleunigungs- oder Speicherparitaetsversprechen aus einem
Messpaar. Die GPU-Prepasskosten steigen um rund 0.069 ms; die geschlossene
Schattenhuelle hat mehr Geometrie. Keine gemessene Gesamt-GPU-Framezeit und
keine vollstaendige Drawcallzahl: sichtbare Entities sind keine Drawcalls.
Die zusaetzlichen drei Mesh-Assets sind Ruessel und gebuendelte Effektmeshes,
keine Entity je Partikel. DRM/RSS-Peaks schwanken mit Upload-/Treiberallokation;
die niedrigeren Endwerte gelten nicht als allgemeine Speichereinsparung.

`forest-intermediate-shader.log` konserviert den verworfenen Lauf vor der
Material-Key-Spezialisierung (221.27 FPS, GPU Early Prepass 0.400 ms).
Der finale Unterschied von 0.006 ms beweist fuer sich keine GPU-Beschleunigung
durch diese Spezialisierung; sie garantiert strukturell getrennte Shaderzweige.

`forest-before.log` verwendet das vor Aenderungen erhaltene Phase-58-Binary.
Dessen eingebetteter UI-Pfad wurde in einer separaten Kopie auf isolierte
Templates umgeleitet: der alte Komponenten-Katalog kennt `target-card` nicht.
Gameplay-/Renderer-Code blieb dabei unveraendert. Der isolierte Assetbaum
behaelt den urspruenglichen Laubshader und entfernt nur neue, vom alten Parser
nicht unterstuetzte `hud_stats`. Reproduktion: `isolate59-ui.mjs` und
`prepare59-baseline.mjs` im uebergeordneten Benchmark-Verzeichnis.
