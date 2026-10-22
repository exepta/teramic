# Phase 81: Hoehlenverteilung, Mineralformen Und Spectator

## Weltvertrag

Neue Welten speichern `cave_structure_revision = 2`. Fehlende Werte bleiben 0,
bereits gespeicherte 1 bleibt 1. Welt-ID, Seed, Generatorversion 5, Material-IDs
und Regionsformat bleiben unveraendert. Bestehende Welten werden nicht automatisch
umgeformt. Fuer die neue Hoehlenlandschaft eine neue Welt erstellen; manuelles
Umschalten einer bestehenden Welt ist keine getestete Migration.

Die bestehenden 192-m-Regionen beherbergen jetzt in vier statt zwei von je
fuenf beziehungsweise drei Auswahlfaellen grosse Raeume. Die Radien wachsen
um bis zu 28%, bleiben aber auf 78 m begrenzt. Ein stetiges, niederfrequentes
Feld schliesst Teile des bisherigen engmaschigen Tunnelnetzes. Raumrander sind
unregelmaessiger; zwischen den grossen Kammern bleiben massive Felsbereiche.

Salz bleibt auf die bisherige Cliffted-Desert-/10%-Auswahl begrenzt. Zusammen-
haengende Salzflecken wechseln mit normalem Stein. Geschwungene Kanaele mit
breiteren Becken und trockenen Terrassen ersetzen die runde Wasserflaeche in
Salz-/Steinraeumen. Belegung, Oberflaeche und Stroemung verwenden das vorhandene
Wassersystem. Dies ist lokale Hoehlenhydrologie, kein global simuliertes
Einzugsgebiets-/Grundwassersystem. Seltene Wurzeln und kleine Pilze koennen
ausserhalb der Kammern in Tunnelgaengen entstehen; Halt wird am urspruenglichen
Dichtefeld geprueft. Abbau erzeugt keine nachtraeglichen Pflanzenkandidaten.

## Mineralgeometrie Und Interaktion

Salz, Tropfstein und Eis verwenden breite, kantige Schaefte mit kurzen Endfasen
statt langer glatter Kegel. Salz/Eis haben unterschiedlich bevelte prismatische
Querschnitte, Tropfstein unregelmaessige Rippen und waermere Facetten.
Neue Mineralgruppen enthalten versetzte Laengen und breitere Ansaetze; die
Gruppentickets werden entsprechend seltener. Natuerliche Salzlaengen bleiben
auf zehn Zellen begrenzt. Die Referenz dient der Form-/Farbgestaltung, nicht
als Behauptung einer identischen Beleuchtung oder pixelgleichen Landschaft.

Rendern und Picking verwenden weiterhin dieselben Dreiecke. Schaefte benoetigen
16 Seitendreiecke je Zelle statt 32; nur Endzellen haben eine zusaetzliche Fase.
Keine Entities je Facette, keine neue Pipeline oder Materialien je Kristall.
Die vorhandenen Teilzellen-Collider folgen dem gemeinsamen Breitenprofil.

Ein Rechtsklick mit passendem Kristallmaterial auf eine bestehende Kette
verlaengert deren freies Ende, unabhaengig von der angeklickten Seite/Zelle.
Fuer einen neuen benachbarten Schaft den tragenden Fels anklicken. Das bereits
vorhandene Salz-Kristallresourceitem kann wieder als Segment gesetzt werden.
Client und Authority teilen Zielbestimmung und Kettenpruefung. Die angeklickte
Zelle muss in Reichweite sein; das freie Ende darf weiter entfernt liegen.
Unbeladene Enden, fehlender Halt, blockierte Ziele, veraltete Revisionen und
Ketten ab 32 Zellen werden abgelehnt. Verbrauch/Edits bleiben im vorhandenen
Inventar-/Regionsjournal, ohne zweiten Speicherpfad.

## Spectator

`/gamemode spectator` und `/gamemode 2` verwenden die bisherigen Berechtigungen,
Chatvorschlaege und serverseitige Modus-Speicherung. Freier Flug hat keine
Terrainkollision oder Ausdauer-/Ertrinkenskosten. Weltinteraktion, Mob-Angriffe,
Inventartransaktionen und Itemaufnahme sind serverseitig gesperrt.
Spectator-Avatare erscheinen nicht in den Bewegungssnapshots anderer Spieler.
Eigener Koerper, gehaltenes Item und dessen Fackellicht werden ausgeblendet.
Eine neutrale, kameraeigene Hoehlen-Aufhellung verbessert die Orientierung,
ohne die Beleuchtung anderer Spieler oder Oberflaechen-Biome zu aendern.
Zum Verlassen des Modus muss sich der Spieler in geladenem freiem Raum befinden.
Netzwerkprotokoll 46: Client und Server gemeinsam neu bauen/starten.

## Pruefungen Und Messungen

Messkonfiguration: Ryzen 5900X, RX 6950 XT/RADV/Vulkan, native 1920x1080,
Sichtweite 8, vertikal 3, hohe Wasserqualitaet mit SSR, 10:00 angehalten,
30 s stationaere Messzeit. Keine Compiler waehrend der Messfenster.
Die beiden expliziten Inspektionslichter der Hoehlen-Testszene sind Teil
des Messaufbaus, nicht der natuerlichen Weltbeleuchtung.

Der vorherige Client liegt lokal unter `target/phase81/terra-client-before`.
`logs/before.log` verwendet den Phase-80-Weltvertrag 1. Neue Testwelten liegen
unter `configs/`; ein Render-only-Vergleich verwendet weiterhin die unveraenderte
Konfiguration aus `../cave-structures-2026-10-19/configs/`.
Historische Seegras-/Streamingregressionen werden durch diese Kurztests nicht
als behoben betrachtet.

| Messung | Vorher, Vertrag 1 | Nachher, Vertrag 1 | Neue Welt, Vertrag 2 |
| --- | ---: | ---: | ---: |
| FPS | 118.20 | 116.03 | 134.95 |
| p95 Frame, ms | 9.645 | 10.187 | 8.925 |
| p99 Frame, ms | 10.564 | 11.280 | 9.922 |
| Initiales Laden, s | 4.120 | 4.660 | 3.873 |
| Peak RSS, KiB (measure.py) | 932000 | 938072 | 885384 |
| Peak VRAM, KiB | 3407464 | 3409036 | 2809664 |
| Geladene Sections | 2835 | 2835 | 2831 |
| Gerenderte Sections | 2752 | 2752 | 1812 |
| Mesh-Assets | 8835 | 8835 | 6865 |
| Terrain-Komposition, Worker-s gesamt | 3.278 | 3.521 | 2.318 |
| Indirekte Draw-API-Aufrufe | 44 | 44 | 40 |

Rohdaten: `logs/before.log`, `logs/after-same-world.log`,
`logs/after-new-world.log`. Der unveraenderte Weltvergleich ist rund 1.8%
langsamer und hat eine schlechtere p99; aus einem einzelnen Kurzpaar ist
weder allgemeine Paritaet noch eine eindeutige Ursachenanalyse abzuleiten.
Die neue Welt hat andere Geometrie und eine an den Raumradius angepasste
Kamera, ist daher kein direkter Beschleunigungsnachweis. Draw-API-Aufrufe
sind keine GPU-internen Subdrawzahlen; separate GPU-Frametimes wurden in
diesen Laeufen nicht erfasst. RSS-Sampling und Bevy-Momentwerte unterscheiden
sich zeitlich; die Tabelle verwendet einheitlich den externen Prozesssampler.
Die Messungen liefen vor der abschliessenden Reduktion der reinen Spectator-
Aufhellung und einer Mineral-Laengenabsicherung fuer sehr niedrige Raeume.
Beide Messkameras verwenden normalen Modus und hohe Raeume.

## Validierung

- `cargo fmt --all --check`: bestanden.
- `cargo check --workspace`: bestanden (`logs/check-final.log`).
- `cargo test --workspace`: 1278 bestanden, ein vorhandener Langzeit-UDP-Soak
  regulaer ignoriert (`logs/test-final.log`).
- `cargo clippy --workspace --all-targets -- -D warnings`: bestanden
  (`logs/clippy-final.log`).
- `cargo build --workspace`: bestanden (`logs/build-final.log`).
- Dedicated Server ohne DISPLAY/WAYLAND_DISPLAY: Start, 27 Sections und
  sauberer Abschluss bestanden (`logs/server.log`). Der normale Dependency-
  Baum enthaelt kein bevy_render/bevy_pbr/bevy_winit/terra_render/terra_ui.
- Native Vulkan-Galerie: je zwei Ansichten fuer Salz, Tropfstein, bepflanzte
  und Wasserfallhoehle; acht Screenshots unter `captures/`, nonblank geprueft
  und Bilder manuell angesehen. Keine ERROR/WARN in `logs/native.log`.
- Spectator-Galerie ohne Inspektionslichter: acht weitere erfolgreiche
  Aufnahmen (`captures-spectator-final/`, `logs/spectator-final.log`). Salz-
  und Tropfsteinansichten manuell geprueft. Der erste Versuch mit Ambient
  650 ueberstrahlte helle Flaechen; final 100 bei voller Hoehlenblendung
  erhaelt Materialkonturen und verbessert die Sicht gegenueber normal 25.2.

Regressionstests decken deterministische groessere Raeume, getrennte
Tunnelbaender, Salz-/Steinverteilung, Flussufer, Raum-/Regionsraender,
spaerliche verankerte Gangflora, Mineralindizes und gemeinsame Hitgeometrie,
Kettenenden an Boden/Decke, fehlenden Halt, blockierte Enden, Maximalhoehe,
negative Koordinaten und autoritativen Einzelverbrauch ab. Der UDP-Modustest
wechselt in Spectator und zurueck und prueft gespeicherten Modus,
Flugberechtigung und das Ausblenden beim zweiten Client.

Ein erster Testlauf fand eine inkonsistente Raumform am lokalen Salz-
Biomrand: Dort wechselte nicht nur das Material, sondern der ganze Raumtyp.
Revision 2 haelt jetzt das Raumprofil durchgaengig und beschraenkt nur die
Salzablagerungen auf das erlaubte Biom. Die finalen Tests sind gruen.
Ein vorheriger Linklauf scheiterte an ungueltigen inkrementellen
terra-render-Artefakten; gezieltes `cargo clean -p terra-render` und Neubau
behoben das ohne Quelltext-/Dependency-Workaround.

## Grenzen

Die lokale Raumhydrologie ist kein globales unterirdisches Flussnetz.
Minerale werden weiterhin als einzelne spielbare Metersegmente erzeugt,
ihre Facetten bleiben Abschnittsbatches. Raumformen und Beleuchtung sind
eine stilisierte Annaeherung, keine identische Kopie der Referenz.

Spectator besitzt noch keinen Kamerabesitz anderer Spieler. Populationsfokus,
kosmetische Wassereffekte und die Blockplatzierungs-Obstruktion anderer Spieler
sind noch nicht vollstaendig vom Spectator-Koerper entkoppelt. Die wichtigsten
Interaktionen, Schaden, Kollision und Sichtbarkeit sind separat abgesichert.
Mehrstuendige Multiplayer-/Streamingabnahme und weitere GPUs bleiben offen.
