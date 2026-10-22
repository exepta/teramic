# Phase 55: Tiere, Hoehlen Und Chatverlauf

## Architektur

`terra-core::creatures` definiert stabile Weltidentitaeten, vier Arten, Verhalten,
Gesundheit und Tierprodukte. Ein 32-m-Habitat besitzt vier deterministische
Spawnkandidaten; negative Koordinaten verwenden euklidische Division. Identitaeten
sind aus Habitat und Kandidatenindex injektiv kodiert. Bereits untersuchte,
auch leere Habitate und tote Tiere bleiben gespeichert. Kein zeitbasierter Respawn.

`terra-world::creatures` benutzt geladene Voxel plus EditJournal. Unbekanntes
Terrain ist eine Barriere. Die lokale Navigation prueft Koerperfreiraum, mehrere
Bodensupportpunkte, Ein-Meter-Stufen, Abgruende und Wasser. Blockaenderungen wirken
ohne neuen Navmesh-Bake. Das ist lokales Steering, keine globale A*-Wegsuche:
komplexe Labyrinthe werden nicht garantiert geloest. Tiere reagieren auf nahe
Spieler und Treffer mit Flucht. Ruhe, Grasen und Umherlaufen wechseln zeitgesteuert;
Schweine laufen haeufiger, Kuehe langsamer. Grasen heilt langsam, aendert keine
Terrainbloecke. Fische bleiben im tatsaechlichen Zellwasser, erhalten 40 Prozent
Stroemung und nehmen nach Wasserverlust Schaden.

Der bestehende autoritative Server simuliert alle Arten, sowohl integriert als
auch dediziert: maximal 128 Tier-Updates pro 100-ms-Takt, zwei Habitatproben je Takt,
64 m aktive Reichweite. Bei mehr Kandidaten werden diese im Round-Robin bedient;
die entfernte Simulation pausiert, es gibt kein teures Nachsimulieren. Maximal
16384 gespeicherte Identitaeten und 65536 untersuchte Habitate begrenzen den RAM;
bei Erreichen der Grenze stoppt die Neubesiedlung. Region-Paging bleibt Backlog.

Pro Client werden bis zu 128 nahe Tiere innerhalb von 80 m ueber die vorhandene
Sequenced-Unreliable-Snapshotstrecke uebertragen. Neue Snapshots ersetzen die
Sichtmenge; das entfernt ausgeladene/getoetete Renderobjekte und erlaubt Late Join.
Der Client interpoliert Position und Rotation und entscheidet niemals ueber Spawn,
Gesundheit oder Loot. Bewegungen verwenden konservative Koerpervolumen; keine
Pixel-/Dreieckskollision pro animiertem Tier und kein Stapeln von Tierkoerpern.

## Darstellung Und Interaktion

Vier eigenstaendige vertexgefaerbte Voxelmodelle orientieren sich am Referenzbild:
braun-cremefarbene Kuh mit Hoernern/Flecken, wolliges Schaf, rosa Schwein und
orange-cremefarbener Fisch. Geometrie wird einmal pro Art gebaut und als Mesh-Handle
geteilt: sieben gegliederte Teile an Land, drei beim Fisch, keine Entity
pro Detailvoxel. Beine, grasender Kopf und Schwanz sind animiert. Eine identische
Reproduktion der Referenz wird nicht behauptet.

Linksklick greift das anvisierte Tier an. Serverseitig werden Blick, Entfernung
(4.5 m), geladene Sichtstrecke, Terrain-/Tierverdeckung, Berechtigung, Lebenszustand
und 450-ms-Cooldown geprueft. Survival-Angriffe kosten Ausdauer; Werkzeuge treffen
staerker. Creative toetet sofort. Rechtsklick fuettert verletzte Schafe/Kuehe mit
Gras und Schweine mit Pilzen aus dem gehaltenen Slot. Kein Zaehmen/Zuechten.
Fleisch, Fisch und Wolle gehen durch die bestehenden 3D-Drops, Pickup, Inventare
und binaere Speicherung. Nahrung verwendet das bestehende Use/Eat-Verhalten.
Itemnamen und Beschreibungen liegen einzeln in `assets/terra/items/*.json`.

Vier leise, synthetisierte Tier-/Blubberstimmen laufen in einem gemeinsamen
Audiostream, mit bestehenden Master-/Ambience-Reglern. Abstand und Unterwasserfilter
verwenden den Spielerkoerper; die Kamera beeinflusst nur das Stereo-Panning.
Keine Audio-Entity pro Tier. Das sind prozedurale stilisierte Signale, keine
aufgenommenen Naturlaute; menschliche Mix-/Klangabnahme bleibt offen.

## Speicherung Und Kompatibilitaet

REG6 erweitert die bestehende Regionentransaktion um Tierpopulationen, nach
Heimathabitat abgelegt. Bewegte Tiere koennen Regionsgrenzen ueberqueren, ohne
ihre Identitaet zu wechseln. REG5/REG4/REG3/REG2/REGN bleiben lesbar. PLY3,
Weltformat 2, Generator 5, Block-/Welt-IDs und Seeds bleiben bestehen.
Tierprodukte sind am Ende des vorhandenen Item-Enums angehaengt; alte binaere
Item-Diskriminanten bleiben erhalten. Client und Server brauchen Protokoll 29.

Neue Welten nutzen Geologievertrag 4. Tropfstein erhaelt dieselbe begrenzte
Freiraum-/Picking-/Rendercache-Pipeline wie Salz: Deckenspitzen bis 14 m,
Bodenspitzen bis 3.85 m, nur jeder 24. geeignete Traeger hat eine grosse Gruppe,
zwei Arme mit insgesamt 24 Dreiecken. Kleine Einschluesse belegen nur ein Viertel
der uebrigen Tropfsteinflaechen; lange Wurzeln werden breiter, bleiben aber in
der geprueften X/Z-Zelle. Alte massive Tropfsteinkoerper bleiben in
Geologie 0/1/2/3 erhalten, keine zerstoererische Migration. Lange Spitzen sind
auswaehlbare Dekoration am Traeger, keine eigenen Bewegungscollider. Die
bestehenden vertikalen 14-Zellen-Snapshots reichen weiterhin aus.
Langsamer dreidimensionaler Noise erzeugt zusammenhaengende Erdflecken in tiefen
Hoehlenwaenden; Salztraeger werden nicht zu Erde ersetzt. Fuer diese neue
Gesamtgeologie eine neue Welt erstellen.

## Chat

Pfeil hoch/runter blaettert ausschliesslich durch die letzten 32 gesendeten
Eingaben, auch bei sichtbaren Befehlsvorschlaegen. Ein ungesendeter Entwurf wird
am unteren Ende wiederhergestellt. Tab/Shift-Tab und Klick bleiben fuer die
Vorschlaege; Menueoeffnung loescht den Eingabeverlauf nicht. Der Verlauf ist
sitzungslokal und wird nicht auf die Festplatte geschrieben.

## Nachweise

Alle folgenden Befehle erfolgreich, mit unveraendertem Lockfile und zwei
Build-Jobs; Netzwerk-Tests seriell (`RUST_TEST_THREADS=1`):

```sh
cargo fmt --all --check
cargo check --locked --workspace
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace
```

883 Tests bestanden; der schon vorhandene explizite 180-s-UDP-Soak ist weiterhin
ignoriert und wurde hier nicht zusaetzlich gestartet. Logs: `terra55-final-*.log`.
Neue Tests pruefen Identitaeten, Spawnreihenfolge/Reload, unbekannte Zellen,
Waende/Abgruende, Wasserentzug, Modelle/Indizes, Chatentwurf, lange/sparse Spitzen,
zusammenhaengende Erdflecken, REG6 mit bewegten/toten Tieren und Tierprodukten.
Ein echter UDP-Test verwendet zwei Clients, prueft ungueltigen Blick, Treffer,
replizierte Entfernung und Drops sowie einen dritten spaeter beitretenden Client.

Native Vulkan-Pruefung auf Radeon RX 6950 XT, Mesa 26.2.4, Wayland:
`--smoke-test-creatures` erzeugte `captures/creatures-0.png` und `creatures-1.png`.
Alle vier Modelle sichtbar, unterschiedliche Bein-/Schwanzposen geprueft.
Dies ist ausdruecklich eine Modellvorfuehrung; der Fisch wird darin ueber dem
Boden gezeigt, kein Nachweis nativen Schwimmens. Die Schwimmlogik pruefen die
Voxel-/Wassertests. `--smoke-test-geology` erzeugte sechs natuerliche Ansichten;
Cheese- und Tropfsteinkammern wurden visuell auf Erdflecken, verankerte lange
Spitzen und freie Zwischenraeume geprueft. Die Bilder verwenden ein Testlicht.
Nach der ersten Bildpruefung wurden kleine Tropfsteine ausgeduennt und der
Geologielauf mit dem finalen Build wiederholt. Keine vollstaendige Art-Abnahme.

`terra-server --no-save --smoke-test` lief mit entferntem DISPLAY/WAYLAND_DISPLAY
erfolgreich auf einem kurzlebigen eigenen Port (`headless.log`). Der bereits
laufende Benutzer-Server wurde nicht beendet oder veraendert.

## Messungen

Der isolierte Test `creature_tests::bounded_herd_navigation_stays_valid_under_many_updates`
aktualisiert 128 Landtiere gegen zwei Spielerpositionen ueber 100 Takte auf einer
geladenen flachen Testwelt. Finale Messung ohne parallelen Compiler:
0.024036 s Gesamtzeit, p95 0.255065 ms und p99 0.289386 ms pro 128er-Takt
(`herd.log`). Das misst lokale Navigation, nicht Netzwerk, Autosave, GPU,
Fischstroemung oder eine komplexe bewegte Herde. Es gibt keine historische
Tier-KI-Baseline, weil das System vorher nicht existierte.

Zusaetzlich ein zeitnahes Buildpaar derselben ruhenden Waldszene, jeweils 60 s,
Seed 42042, alte gespeicherte Geologie 0, Sichtweite 8, VSync aus,
1920 x 1080 physisch. Identische Konfiguration in `configs/`, gefrorener
Vorher-Build und passende Vorher-Assets ohne die neuen Tieritem-Definitionen.
Keine Compiler oder zusaetzlichen Benchmarks liefen parallel. Vorhandene
Desktopprogramme und der Benutzer-Server blieben in beiden Laeufen unveraendert.

| Messwert | Vorher | Nachher |
| --- | ---: | ---: |
| Durchschnittliche FPS | 210.35 | 206.51 |
| p95 CPU-Wall-Frame (ms) | 6.016 | 6.013 |
| p99 CPU-Wall-Frame (ms) | 7.022 | 7.079 |
| Initiale Szenenbereitschaft (s) | 1.828 | 1.810 |
| Peak RSS laut Messwrapper (KiB) | 752708 | 733408 |
| Peak DRM VRAM (KiB) | 2016320 | 2016320 |
| Peak DRM GTT (KiB) | 69780 | 69780 |
| Geladene / gerenderte Sections | 2041 / 875 | 2041 / 875 |
| Mesh-Assets / sichtbare Mesh-Entities | 3487 / 1745 | 3511 / 1745 |
| Repraesentierte Props / Batches | 25808 / 328 | 25808 / 328 |
| Terrain-Kompositionen / summierte Workerzeit (s) | 894 / 0.748 | 894 / 0.749 |

FPS in diesem Paar rund 1.8 Prozent niedriger, p95 praktisch gleich, p99 leicht
hoeher. 24 zusaetzliche Mesh-Assets sind die geteilten Tierkoerperteile.
Diese Offline-Waldszene enthaelt **keine aktiven oder sichtbaren Tiere** und kein
Wasser: sie misst nur den Grundaufwand des Builds, keine 128-Tiere-FPS oder
Kosten neuer Hoehlen. Keine allgemeine FPS-/Speicherparitaet oder Verbesserung
behaupten. Separate GPU-Pass-Zeitreihen stehen in beiden Logs; keine gemessene
GPU-Gesamtframezeit oder vollstaendige Draw-/Subdrawzahl.

Rohdaten: `before-forest.log`, `after-forest.log`, mit
`tools/benchmarks/measure.py`, `TERRA_BENCH_SECONDS=60`,
`TERRA_RENDER_DIAGNOSTICS=1`, `TERRA_MEASURE_GPU_MEMORY=1`, `--smoke-test-forest`.
Native Laeufe verwendeten `env -u DISPLAY ALSA_CONFIG_PATH=/dev/null`; es war
kein Audioausgang verfuegbar. Tierklang wurde numerisch, nicht akustisch geprueft.
Der fehlgeschlagene Vorversuch `discarded-relative-assets.log` verwendete einen
relativen Bevy-Assetpfad und ist aus allen Messwerten ausgeschlossen. Der finale
Vorher-Lauf nutzt einen absoluten `TERRA_ASSET_DIR`.
