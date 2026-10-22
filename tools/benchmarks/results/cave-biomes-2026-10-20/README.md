# Phase 82: Gemeinsame Hoehlenbiome Fuer Raeume Und Verbinder

## Ursache Und Integration

Die bisherigen Spezialoekologien waren an ein Raumprofil in jeder 192-m-Zelle
gebunden. Das Profil besass wiederkehrende runde Boeden und Kuppeln. Verbinder
waren das vorhandene Paar dreidimensionaler Noise-Felder sowie explizite
Eingangsportale; eine unabhaengige Connector-Graph-Engine existiert nicht.
Revision 2 konnte die Tunnelbreite in grossen Bereichen negativ machen und
damit das Netz vollstaendig unterbrechen.

Revision 3 erweitert diese vorhandenen Felder. Tunnelbreiten bleiben positiv;
der konfigurierbare Faktor 0.84 duennt sie moderat aus. Raumzentren, Radien,
Hoehen, Drehungen, Seitenverhaeltnisse, Doppelkammern, Bodenrelief und Decken
variieren deterministisch. Seltene grosse Raeume bis 90 m Radius bleiben im vorhandenen
Regionsbudget. Kammern und Eingangstunnel werden nicht durch einen zweiten
Generator ersetzt.

## Gemeinsamer Sampler

`TerraGenerator::cave_biome_at([x,y,z])` liefert primaeres/sekundaeres Biom,
gewichtete Mischung, Feuchtigkeit, stabile Region-ID und Grenznaehe.
Gewarpte, unregelmaessig gewichtete Voronoi-Zellen verwenden den Weltseed.
Die konfigurierbaren Durchmesser sind Zielgroessen, keine kreisrunden Grenzen
oder garantierten exakten Gebietsdurchmesser. Eine Region umfasst mehrere
vorhandene Kammern und deren dreidimensionale Verbindungspassagen.

Site-Auswahl gewichtet Normal/Salt/Lush/Cascade mit 45/20/20/15, nach trockener,
feuchter beziehungsweise wasserreicher Klimaeignung. Lush-Bewuchs wird mit
Tiefe geringer, Salz bevorzugt ausreichend ueberdeckte Gesteinsbereiche.
Die Prozentwerte sind Auswahlgewichte, keine garantierten Weltflaechenanteile.
Die neue Vorgabe ersetzt fuer Revision 3 die alte Beschraenkung von Salz auf
zehn Prozent der Cliffted-Desert-Kammern. Alte Vertraege behalten diese Regel.

Eine feste Nachbarschaft wird pro raeumlicher Cachezelle vorbereitet. Die
gemischten Gewichte werden einmal pro X/Z-Spalte berechnet und mit der
vorhandenen PreparedColumn vertikal wiederverwendet. Einzelne Voxel fuehren
keine Nachbarregionssuche aus. Tief im unveraenderten Gestein entfaellt die
Material-/Dekorationsauswertung; Prop-Tickets verwerfen ungeeignete Spalten
vor dem Hoehenscan. Caches sind begrenzt, nicht Voraussetzung fuer Determinismus.

`cave_ecology_sample` beruecksichtigt die Connector-Schalter. Dieselben Regeln
liefern fuer Kammern und Gangausschnitte Salzschichten, moosige Waende,
zusammenhaengende Erdflecken, kleine Kristalle, Ranken, Wurzeln und Pilze.
Kleine Farne/Grasgruppen verwenden das vorhandene Prop-Datenmodell und
Section-Batching mit stabilen 64-Bit-IDs. Originales Dichtefeld, Anker und
Lichtraum werden geprueft; nachtraeglicher Abbau erzeugt keine neue Flora.
Minerale in schmalen Gaengen bleiben an der Wandzone und halten mindestens
drei freie Zellen jenseits der Dekoration. Feuchtigkeit erreicht die
bestehenden Materialbedingungen auch ausserhalb von Kammern. Hoehlenpilze
verwenden weiterhin das vorhandene budgetierte lokale Lichtsystem.

## Wasser

Kaskadenraeume behalten geometrisch geformte Becken, Felsstufen und die
bestehenden Wasserfall-/Schaum-Batches. In feuchten Connector-Regionen wird
zusaetzlich das vorhandene Tunnelvolumen auf kleine Aquifertaschen geprueft.
Ein begrenzter Flood-Fill verwirft Komponenten mit offenen Raendern,
unbekanntem/tiefem Boden oder Ueberlappung einer anderen Kammer-Wasserhoehe.
Auch seitliche Oeffnungen unter dem Wasserspiegel werden ausgeschlossen.
Kurze geschlossene Komponenten bekommen einen gemeinsamen Wasserstand.
Laengere Komponenten erhalten maximal 35 cm Gefaelle zum tieferen Beckenende;
die Stroemung folgt freien benachbarten Wasserzellen, nicht einer Geraden
durch Felswaende. Das ist eine lokale prozedurale Stroemungsapproximation,
keine globale hydrologische Massenbilanz.
Occupancy, Meshing, Schwimmen und Edits nutzen dasselbe WaterColumns-System.
Dies erzeugt passende kleine Becken in Gaengen, aber keinen Wasserlauf in
jedem Verbinder und kein globales unterirdisches Fluss-/Grundwassernetz.

## Konfiguration Und Kompatibilitaet

Die existierende boolesche Option `world.caves` bleibt erhalten. Daher liegen
die neuen Tabellen in `configs/world.toml` unter `[cave_biomes]`,
`[cave_biomes.distribution]`, `[cave_biomes.regions]` und
`[cave_biomes.connectors]`, nicht unter einem inkompatiblen `caves`-Objekt.
Gewichte werden validiert und zur Auswahl normalisiert; NaN, negative Werte,
Gesamtgewicht null und ungueltige Groessen/Uebergaenge werden abgelehnt.

Neue Welten speichern `cave_structure_revision = 3` samt Einstellungen.
Vorhandene Vertraege 0/1/2 bleiben unveraendert; bestehende Dedicated-Saves
besitzen weiterhin ihren eigenen gespeicherten Vertrag samt Biomkonfiguration.
Keine stille Retrogen-Migration. Generatorversion 5, Seed, Block-IDs und
Regions-/Spielerformat bleiben erhalten. Netzwerkprotokoll 47 erfordert
gemeinsam aktualisierte Clients und Server.

## Debug Und Reproduktion

F1 zeigt unterirdisch Cave-Biome, Mischung/Feuchtigkeit, Region-ID,
Grenznaehe und Connector-/Raumzugehoerigkeit im vorhandenen TiltUI-Overlay.
Die Grenzdistanz ist ein Abstandsmass des gewichteten Voronoi-Feldes,
keine exakte Entfernung zur naechsten begehbaren Hoehlenwand.

`cargo run -p terra-world --example cave_biomes_benchmark -- 3 /tmp/caves.ppm`
misst 3072 Sections in identischen Koordinaten und exportiert optional einen
2048 x 2048 m grossen horizontalen Debug-Schnitt bei Y=-42.5.
Grau: Normal, Rosa: Salz, Gruen: Lush, Blau: Cascade; hell: Hohlraum,
dunkel: Gestein, weiss: Regionsgrenzen. `biomes.png` ist die formatkonvertierte
Datenausgabe, kein Screenshot und keine erfundene Renderansicht.

## Pruefungen Und Messungen

### CPU-Generierung

`target/debug/examples/cave_biomes_benchmark 2` beziehungsweise `3`:
Seed 42042, 256 X/Z-Spalten, je 12 Sections, insgesamt 3072 Sections,
gleiche Koordinaten, dev-Profil opt-level 1, keine GPU-Uploads.
Messungen nacheinander ohne gleichzeitig laufenden Compiler.

| Vertrag | Spalten vorbereiten | Sections erzeugen | Gesamt |
| --- | ---: | ---: | ---: |
| Revision 2 | 0.195 s | 1.333 s | 1.528 s |
| Revision 3, final | 0.237 s | 1.532 s | 1.769 s |

Die neue Geometrie mit Oekologie, Bewuchs und Aquiferpruefung kostet in diesem
Schnitt **15.8% mehr CPU-Zeit**. Keine allgemeine Performanceparitaet.
Die erste Fassung benoetigte 2.379 s; fruehe Gesteins-/Prop-Ablehnung,
vorbereitete Gewichte und wiederverwendete Oberflaechenhoehen reduzierten dies.
Checksummen: Revision 2 `39986259`, Revision 3 `39148176`.
Die Welten sind bewusst geometrisch verschieden.

### Native GPU-Laeufe

Linux/Wayland, Vulkan/RADV, Radeon RX 6950 XT, physisch 1920x1080,
Sichtweite 8, vertikal 3, VSync aus, 30 s Messfenster nach initialem Laden.
Jeweils derselbe Cave-Smoke-Harness mit zwei expliziten Inspektionslichtern.
Kein Kompilieren waehrend der Messungen; sonstige Desktop-Anwendungen liefen
weiter. `native-before.log`, `native-after-legacy.log`, `native-after-rev3.log`
enthalten die Rohdaten.

| Messung | FPS | p95 / p99 ms | Laden | Peak RSS | Peak VRAM |
| --- | ---: | ---: | ---: | ---: | ---: |
| Alter Build, Revision 2 | 134.87 | 8.826 / 9.660 | 3.443 s | 859.0 MiB | 2743.8 MiB |
| Neuer Build, Revision 2 | 134.48 | 8.896 / 10.081 | 3.501 s | 864.0 MiB | 2742.3 MiB |
| Neuer Build, Revision 3 | 137.20 | 8.784 / 10.181 | 4.186 s | 934.1 MiB | 3729.5 MiB |

Nur die ersten zwei Zeilen sind ein geometrisch identischer Buildvergleich:
2831 geladene Sections, 1812 gerenderte Sections, 6865 Mesh-Assets,
3250 Props in 422 Batches, 12 Wassertiles, 40 indirekte PBR-API-Draw-Aufrufe.
FPS -0.3%, p99 +4.4%; kurze Einzelmessungen, keine garantierte Paritaet.

Revision 3 sucht absichtlich eine andere natuerliche Salzhoehle und ist **kein
Vorher-/Nachher-FPS-Vergleich**: 2251 geladene Sections, 2088 gerenderte Sections,
8371 Mesh-Assets, 24209 geladene Props inklusive Oberflaechenbewuchs in 679 Batches,
4 Wassertiles, 62 indirekte PBR-API-Draw-Aufrufe. Die Kamera und die mitgeladenen
Oberflaechen unterscheiden sich. Terrain-Workerzeit 3.831 s, groesster Job
16.067 ms; GPU-interne Subdrawzahlen und GPU-Passzeiten wurden nicht gemessen.

### Visuelle Und Headless-Pruefung

`native-gallery.log` und `captures/` enthalten acht echte native Ansichten,
je zwei fuer Salt, Dripstone, Lush und Cascade. Vier Ansichten wurden zusaetzlich
visuell geprueft: unterschiedliche Decken-/Bodenformen, Salz-/Moosfelder,
Ranken/Bewuchs, Pfeiler, Becken und das neue F1-Overlay sind sichtbar.
Keine Shader-, Slab-Allocator- oder sonstigen Renderfehler in diesen Laeufen.
Inspektionslichter erzeugen teilweise starke Highlights; die Bilder sind
keine Abnahme der natuerlichen Beleuchtung oder einer exakten Referenzoptik.
Die Connector-Zuordnung ist im Datenschnitt und in Tests geprueft, nicht durch
eine native Kamerafahrt entlang aller neun Biomkombinationen.

Der Dedicated Server absolvierte mit entfernten `DISPLAY`/`WAYLAND_DISPLAY`
seinen Smoke-Test (27 Sections, Exit 0). Der normale Server-Dependency-Graph
enthaelt weder `bevy_render`, `bevy_pbr`, `bevy_winit` noch TiltUI.

`cargo fmt --all --check`, `cargo check --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings` und
`cargo build --workspace` sind erfolgreich. Beide vollstaendigen
`cargo test --workspace`-Laeufe bestanden **1287 Tests**, ohne Fehler.
Ein bereits vorhandener expliziter 180-Sekunden-UDP-Dauertest bleibt ignoriert.
Nach der strengeren Wasserpruefung bestanden zusaetzlich 218 fokussierte
World-Tests. Der finale Workspace-Lauf prueft auch die aktualisierten
Wasser-/Stroemungs-Assertions; Rohprotokoll `logs/workspace-tests-final.log`.

Neue Tests pruefen reproduzierbare Samples, negative Koordinaten,
Grenzkontinuitaet, alle sechs unterschiedlichen Biompaare und drei gleichen
Paare, Connector-/Kammermaterialien, Anker/Freiraum, Props, Konfigurationsschalter,
Section-/Reload-Gleichheit und seitlich sowie unten geschlossene Wasserbecken.

### Verbleibende Grenzen

- Lange Mehrspieler-/Streaminglaeufe und weitere GPUs bleiben in TODO.md offen.
- Regionsdurchmesser sind Richtwerte eines unregelmaessigen gewichteten Feldes;
  weder exakte Kreise noch genaue Prozentanteile werden garantiert.
- Aquifere sind konservative lokale Taschen und kurze Laeufe. Es gibt kein
  globales unterirdisches Flussnetz und keine neue hydrologische Simulation.
- Die bestehende lokale Hoehlenbeleuchtung wird weiterverwendet, kein neues
  GI-/Skylight-System. Vollstaendige menschliche Art-Abnahme bleibt separat.
- Alte Welten behalten ihre Geometrie. Fuer die neue Verteilung eine neue Welt
  erstellen; gespeicherte Revisionsnummern nicht nachtraeglich umschreiben.
