# Project Terra

Aktueller UI-/Wetterabgleich: gemeinsame Inventar-/Katalogslots, ueberarbeitete
Rahmen und Karten, begrenzte Wegpunktanzeige und sichere Wasserabfragen bei
Wetter-Edits. [Phase 187: Pruefbericht](tools/benchmarks/results/inventory-frame-weather-2026-11-03/README.md).

Rust-/Bevy-Sandbox mit abbaubarem Block-Voxel-Terrain und einzeln entfernbaren
Pflanzen. Stilziel sind terrassierte Landschaften, kleinteilige Vegetation,
gemalte Materialien und atmosphaerisches Licht. Das nachgereichte Referenzbild
dient als Orientierung fuer den aktuellen Geometrie-, Vegetations- und
Wasserabgleich; vollstaendige Referenzqualitaet wird nicht behauptet.

Die aktive Welt verwendet jetzt Minecraft-artige **16 x 16 Chunk-Spalten mit
gestapelten 16 x 16 x 16 Sections**. Das ehemalige Debug-Hoehenfeld ist entfernt.
Dies ist eine eigene Engine-Implementierung, kein Minecraft-Protokoll oder
Anvil-Dateiformat.

## Technologien

Rust Edition 2024 / mindestens Rust 1.97.1, Bevy 0.20.0, Avian3D 0.8.0,
Lightyear 0.31.0, TiltUI aus dem separaten lokalen Repository
`../../tilt-us/tilt-ui` auf `preview-020`,
Serde 1.x, TOML 0.9.x, serde_json 1.x und Tracing 0.1.x.
Noise: noise 0.9.0. Block-Meshing:
[block-mesh 0.2.0](https://docs.rs/block-mesh/0.2.0/block_mesh/).
Voxel-Raycasts:
[voxel-traversal 0.4.0](https://docs.rs/voxel-traversal/0.4.0/voxel_traversal/).

Die historische 0.20-Migration samt nativen Kurztests und gemessener
Dorf-Performance-Regression ist im damaligen
[Migrationsbericht](tools/benchmarks/results/bevy-020-2026-10-26/README.md) dokumentiert.
Seit Phase 134 gibt es kein `vendor/` und keine lokalen Bevy-Quellpatches mehr:
Bevy kommt unveraendert aus der Registry. TiltUI-Korrekturen erfolgen direkt
im separaten TiltUI-Repository; Teramic verwendet voruebergehend lokale
Pfadabhaengigkeiten statt eines Git-Pins, bis der Push freigegeben ist.
Cargo.lock fixiert die Registry-Versionen, nicht den lokalen TiltUI-Quellstand.
Bevy/Avian/Lightyear haben gezielte Features; externe Crates werden im Dev-Profil
optimiert. Release verwendet Thin LTO. Aktueller Arbeits-/Validierungsstand:
[Phase 134](tools/benchmarks/results/cave-fps-no-vendor-2026-10-26/README.md).

## Aufbau

Die `lib.rs`-Dateien enthalten Moduldeklarationen und oeffentliche Exporte.
Die Implementierung liegt in fachlich benannten Modulen:

| Crate / App | Ordner und Aufgaben |
| --- | --- |
| terra-core | `config/`: Client, Server, Welt, Laden, Fehler und Validierung; `gameplay.rs`: Spielmodus, Inventar und Ressourcen; `identity.rs`, `constants.rs`, `plugin.rs` |
| terra-voxel | `blocks/`: Sections, Koordinaten, Indexierung, Raycasts; `meshing/`: Greedy-Quads und Geometrie; `density/`: bisherige Dichte-Sample-API; `materials.rs` |
| terra-world | `generation/biomes/`: Klima und fuenf gewichtete Profile; `landforms.rs`, `surface.rs`: Terrassen und Bodenschichten; `columns.rs`: wiederverwendbare Spaltendaten; `generation/vegetation/`: Baeume und Bewuchsmasken; `water/`: Zellsolver und separate Wasseroberflaechen; `edits/`: gemeinsames Edit-/Gameplay-Journal; `streaming/`: Interesse, Batch-Jobs, Ressourcen, Edits und Plugin |
| terra-render | `blocks/`: gemeinsame Cube-Vorlagen, JSON-/Textur-Registry und optionale Custom-GLBs; `meshing/`: verbundene Kanten, LOD, Geometriecache und Worker-Batching; `props/`: Micro-Geometrie, verankerter Wind und Section-Batches; `building/`: Debug-Bauvorschau, Bruchanimation und Ressourcenmarker; `previews.rs`: echte Blockansichten fuer TiltUI; `water/`: erweitertes PBR-Material, Upload und Lebenszyklus; Szene und Auswahlrahmen |
| terra-player | `camera/`: Lauf-/Flugsteuerung, Avian-Character-Controller und Mausfreigabe; `interaction/`: Targeting und Abbau/Setzen |
| terra-network | Lightyear/UDP-Netcode, versionierter Handshake, serverseitige Autoritaet, Client-Replikat, Prediction/Reconciliation und bestaetigte Section-Deltas |
| terra-physics | Gemeinsame Character-Simulation fuer Client und Server, zentrales Avian-Plugin und asynchrone Terrain-Collider |
| terra-storage | `manifest.rs`: versionierter Speichervertrag |
| terra-ui | TiltUI-Plugin, Einstellungen mit Live-Vorschau und generierter Komponentenkatalog |
| apps/client | Bootstrap, native Szenen-/Interaktions-Smoke-Tests und kurzer Einstiegspunkt |
| apps/server | Gemeinsamer Headless-Bootstrap fuer Integrated/Dedicated Server, verwalteter Serverthread, automatischer Join-Handshake und Signalbehandlung |

`src-ui/` enthaelt die echten TiltUI-Komponententripel, Einstellungsmenue, Overlay und Fadenkreuz.
`configs/` enthaelt client.toml, server.toml und world.toml sowie die gespeicherten Menue-Einstellungen.
`assets/terra/blocks/` enthaelt datengetriebene Cube-/Spezialdefinitionen, vier gemeinsam genutzte
aktive PNG-Texturen und sieben wiederverwendete prozedurale Texturmuster. Die zehn nicht mehr benoetigten Standard-GLBs,
ihre `model`-Verweise und der alte Exporter `tools/assets/build_blocks.py` wurden
entfernt. Das spart 15875208 Bytes an ausgelieferten Modell-Assets, keine
zusaetzlichen Draw Calls: der aktuelle Cube-Pfad hatte diese Dateien bereits
nicht mehr geladen. Es gibt weiterhin genau eine Block-Registry und keinen
Texturatlas. Die optionale Custom-Fixture unter `fixtures/` bleibt erhalten.
`tests/headless.rs` prueft das echte Server-Plugin-Set.
Es gibt keine zyklischen Crate-Abhaengigkeiten.
Offene Arbeiten stehen in [TODO.md](TODO.md), abgeschlossene Schritte und ihre
Test-/Messberichte kompakt in [PATCH.md](PATCH.md).

## Installation und Start

Rust 1.97.1+, rustfmt, Clippy und ein nativer Linker werden benoetigt.
`rust-toolchain.toml` waehlt die passende Toolchain automatisch.
Fuer den aktuellen lokalen Entwicklungsstand muss auch das TiltUI-Repository
unter `../../tilt-us/tilt-ui` vorhanden sein; die Runtime wird nicht nach Teramic kopiert.
Der Linux-Client braucht X11/Wayland, xkbcommon, Fontconfig und einen Vulkan-Treiber.
Der Server braucht keine Display-Session und keine Grafikbibliotheken.

```bash
rustup component add rustfmt clippy
cargo build --locked -p terra-client
cargo build --locked -p terra-server
cargo run --locked -p terra-client
cargo run --locked -p terra-server
```

Der normale Client startet im **TERAMIC Main Menu**, ohne Weltgenerierung oder
automatisch gestarteten Host. Singleplayer oeffnet die persistente Weltenliste.
Erst **Play World** startet einen Integrated Server: eigener Headless-Bevy-World
auf einem verwalteten Thread, Loopback-UDP und derselbe serverautoritative
Protokollpfad wie im Multiplayer. Beim Verlassen wird gespeichert und der Thread
gestoppt/gejoint; es bleibt kein separater Serverprozess zurueck.

**Multiplayer mit Dedicated Server**, lokal beispielsweise in zwei Terminals:

```bash
cargo run --locked -p terra-server -- --bind 127.0.0.1:5000
cargo run --locked -p terra-client -- --connect 127.0.0.1:5000
```

Keine Join-Dateien und keine manuell konfigurierten oder verteilten privaten
Serverschluessel. Der automatische Handshake handelt intern kurzlebige Netcode-
Transportcredentials aus; die bewahrte Lightyear-Verschluesselung braucht weiterhin
einen nur im Serverprozess lebenden Transportschluessel. Das ist kein Account-Key.
`--connect-token` und `--token-out` sind abgeloest. Protokollstand jetzt **73**:
Client und Server gemeinsam neu bauen/starten. Seeds und Welt-IDs bleiben gleich.

Im LAN den Server mit `--bind 0.0.0.0:5000` starten. **Multiplayer** entdeckt ihn
automatisch, auch ohne gespeicherten Eintrag; auswaehlen und **Join Server**.
**Add Server** speichert alternativ Name, Adresse und Port. LAN-Auswahl kann damit
dauerhaft gespeichert werden. Firewall: Spielport **UDP und TCP** (Standard 5000),
zusaetzlich **UDP 54545** fuer IPv4-Multicast/Loopback-Discovery. Veraltete LAN-
Eintraege verschwinden nach sieben Sekunden; Status, Spielerzahl und Antwortzeit
stammen aus echten Abfragen. Ein privater Singleplayer-Host wird nicht beworben.

Jeder Client laedt seine stabile UUID und seinen Namen aus `configs/profile.toml`.
Fuer zwei Spieler auf demselben Rechner braucht jeder einen eigenen Profilpfad,
z.B. zweiter Client `TERRA_PROFILE_FILE=/tmp/terra-player2.toml cargo run --locked
-p terra-client`. Dieselbe UUID kann nicht zweimal gleichzeitig beitreten.
Ein lokales UUID-Profil ist **keine verifizierte Online-Anmeldung**. Ohne externen
Login-/Sessiondienst werden unverifizierte Profile nur auf privaten/Loopback-
Verbindungen akzeptiert. `require_verified_accounts = true` verweigert den
Serverstart, solange kein Account-Verifier existiert, statt Sicherheit vorzutäuschen.
Internet-Authentifizierung und Mehrrechner-LAN-Tests sind weiterhin offen.

### Menues Und Speicherung

Neue Welten aus **Create World** erhalten deterministische Fluesse und
Wasserfaelle. Der gespeicherte Weltparameter `rivers = true` aktiviert diese
Erweiterung. Bestehende Manifeste ohne diesen Parameter bleiben bei `false`,
damit Terrain, Edits und Seeds nicht stillschweigend veraendert werden.
Bei einem **frischen** Dedicated-Save kann `rivers = true` in `configs/world.toml`
vor dem ersten Start gesetzt werden. Einen bestehenden Save nicht durch eine
andere Generator-Konfiguration uminterpretieren; die Manifestpruefung verhindert das.

Wasser besitzt Stroemungsanimation und geformte Fallflaechen in denselben
Section-Batches. Kleine Gefaelle werden kontinuierlich interpoliert; gemeinsame
Wasserecken verhindern die bisherigen periodischen Stufen-/Schaumstreifen.
Erst ab **5 Bloecken Hoehenunterschied** entsteht ein senkrechter Wasserfall.
Tief unter dem Gelaende verlaufende Fluesse behalten jetzt ihr Felsdach und
schneiden darunter einen echten gewoelbten Tunnel aus. Verdeckte Enden reichen
bis zu 40 m als wasserfuehrende Galerie in eine gerundete Abschlusskammer.
Wo bereits mindestens 30 m natuerliches Gefaelle innerhalb von 64 m vorhanden
sind, kann ein bis zu 44 m hoher Katarakt mit offenem Fallschacht entstehen.
Der Unterlauf wird dabei abgesenkt, nicht durch angehobenes Wasser aufgestaut.
Physik, Wasseroberflaeche und Headless-Generierung verwenden denselben Hoehlenboden;
Baeume und Bodenpflanzen bleiben auf dem aeusseren Gelaende.
Der Flusspegel beruecksichtigt die niedrigeren Ufer im gecachten Pfad. Nur das
eigentliche Flussbett wird gefuellt; die breitere Gelaende-Uebergangszone erzeugt
keine frei schwebenden Seen. Wasserraender schliessen am festen Boden an.
Wasserfaelle besitzen verbundene Lippen, gekruemmte Baender und gezielten Kamm-/
Aufprallschaum. Wellen und Noise behalten ihre Phase ueber Section-Grenzen.
Sichtbarer Aufprallschaum und ein einzelner begrenzter Spruehbatch mit maximal
96 weichen Quads ergaenzen nahe Faelle bis 48 m; Low schaltet den Spruehnebel ab.
Dieser Standardpfad ist eine leichte Nebelapproximation. Optional aktiviert
`water_volumetric_mist = true` in den Grafikoptionen bei High echte raygemarchte
Bevy-FogVolumes: maximal vier getrennte Aufprallbereiche innerhalb 40 m,
24 Schritte und eine geteilte weiche 16x16x16-Dichtetextur. Ohne nahe Faelle,
bei Low/Medium oder ausgeschaltetem Schalter entfallen Volumes, Kameraeffekt
und volumetrisches Sonnenlicht. Der zusaetzliche Pass bleibt standardmaessig aus.
Die hydraulische Fallkante beeinflusst nicht mehr die gesamte Talbreite:
ausserhalb des Kanals gehen kontinuierliche, unregelmaessige Erosionsrampen
in die Haenge ueber, statt eine breite plane Steilwand zu erzeugen.

Flussstroemung ist nicht nur eine Animation. Die gemeinsame Abfrage der realen
Wasserbelegung liefert Richtung, Eintauchen und Fallwasser-Kontakt fuer Spieler,
dynamische Avian-Koerper und lose Items. Vierzig Prozent der lokalen
Referenzgeschwindigkeit (2.4 m/s) wirken als Mitnahme; aktives Gegenschwimmen
bleibt moeglich. Stille Seen/Meer haben keine erfundene Stroemung, feste
Wasserbloecke sind keine Fluessigkeit. In neu ueberfluteten Zellen beruehrte Props
werden ueber ihre bestehenden IDs im Entfernungsjournal entfernt. Das
aktualisiert nur notwendige Prop-Batches, bleibt nach Reload bestehen und ist
keine zusaetzliche manuelle Ernte.
Flussbreite und Endbecken gehen kontinuierlich ineinander ueber. Die breiteren,
gerundeten Ufer schneiden ausschliesslich ins bestehende Gelaende ein, statt
kuenstliche Stuetzwaende anzuheben. Diese Korrektur betrifft die Generierung
von Welten mit `rivers = true`: zum Vergleichen eine neue Welt verwenden;
bestehende Flusswelten regenerieren beim Laden das korrigierte Grundterrain,
ihre gespeicherten Edits bleiben an ihren bisherigen Koordinaten. Welten ohne
Fluesse bleiben unveraendert. Client und Server gemeinsam neu bauen/starten,
damit beide dieselbe korrigierte Flussgeometrie erzeugen.
Unter Wasser reagieren Nebel, Farbstimmung und Umgebungshelligkeit
auf die Kameratiefe; beim Auftauchen wird die normale Umgebung wiederhergestellt.
Ufer-, Fluss- und Wasserfallgeraeusche verwenden dagegen den **Spielerkopf** als
Hoerpunkt, auch in Third Person. Untertauchen blendet einen echten Tiefpass ein.
Unter **Options > Audio** sind Master, Ambience, Water & Shore, River Current,
Waterfalls und Mute einstellbar. Die Pegel werden miteinander multipliziert,
erst mit Apply angewendet und in `configs/audio.toml` gespeichert. Der Fluss
verwendet ein leiseres, tiefpassgefiltertes Stroemen mit einzelnen gedaempften
Plaetscherimpulsen statt des bisherigen hellen Rauschanteils.
Stilles Wasser verwendet eine eigene, nochmals leisere und weicher gefilterte
Stimme. Natuerliche Fluesse addieren nicht gleichzeitig die alte lautere
Flachwasser-Grundstimme. Wasserfallrauschen bleibt getrennt geregelt.
Phase 20 ergaenzt erodierte Tunnelportale, wechselnde Bettiefen und gelegentliche
Seitenarme mit Wiedervereinigung. Der begrenzte autoritative Zellsolver fuer
Ausfluss nach Uferabbau ist implementiert und mit nativen Edit-Tests validiert.
Fluessigkeitszustaende verwenden das vorhandene Section-Journal;
es gibt weiterhin keine Wasser-GLBs oder einzelnen Wasser-Entities. Architektur
und bewusste Grenzen stehen unter **Zellbasiertes Wasser** weiter unten.

**Logische Props koennen nicht mehr ueber Graphics ausgeblendet werden.**
Die Vegetationsdichte-Option ist entfernt; alte TOML-Werte werden kompatibel
gelesen, aber nicht angewendet oder erneut gespeichert. Rendern und Picking
beruecksichtigen alle nicht entfernten Props. LOD, Frustum-Culling und Section-
Batching bleiben erhalten. Im Survival-Modus liefern einzeln abgebaute Pflanzen
Ressourcen; diese koennen im Inventar gesammelt und auf freiem, trockenem
Naturboden wieder gepflanzt werden. Alle Prop-Arten sind visuell um 30 Prozent
groesser; IDs, Ankerpositionen und gespeicherte Skalierungswerte bleiben erhalten.
Rendering, Wind und Picking verwenden gemeinsam `WorldProp::visual_scale()`.

Details und Messungen der aktuellen Erweiterung:
[Verbundene Tunneldecken Und Kantiger Bewuchs](tools/benchmarks/results/tunnels-botany-2026-10-15/README.md),
[Flusstunnel, Hohe Faelle Und Audio-Optionen](tools/benchmarks/results/river-tunnels-audio-2026-10-15/README.md),
[Wasser, Audio Und Atmosphaere](tools/benchmarks/results/water-audio-2026-10-15/README.md).

Dedicated speichert standardmaessig direkt unter `worlds/` (konfigurierbar in
`configs/server.toml` mit `save_directory`). CLI: `--save-dir PATH` oder bewusst
`--no-save`. Manifest/Seed/Generator-Konfiguration werden vor Laden geprueft;
eine exklusive OS-Dateisperre verhindert zwei gleichzeitige Schreiber. Das
vorhandene Journal wird alle zehn Sekunden bei Aenderungen asynchron und beim
geordneten Shutdown final atomar gespeichert. Fuer Backups den Server stoppen.
Beim Wechsel vom ausgelieferten alten Standard `saves/dedicated` werden dessen
validierte Zustaende nach `worlds/` uebernommen, sofern dort noch keine Dedicated-
Welt liegt. Der alte Ordner bleibt als Sicherung; abweichende Seeds/Konfigurationen
werden nicht stillschweigend uebernommen.

Optionale individuelle Rechte in `configs/server.toml` (UUID durch eigenes
lokales Profil ersetzen; Aenderungen gelten beim naechsten Beitritt/Neustart):

```toml
[player_permissions."11111111-2222-4333-8444-555555555555"]
build = true
flight = false
environment = true
```

Ohne Eintrag bleibt Bauen erlaubt; Flug/Umgebungswerkzeuge folgen `allow_flight`.
Diese Rechte werden serverseitig durchgesetzt. Eine lokale UUID ist trotzdem
kein sicherer Accountnachweis fuer oeffentliche Server.

Die aktuelle Performance-/Gameplay-Erweiterung ist mit Grenzen, Messungen und
Nachweisen unter [Phase 15](tools/benchmarks/results/performance-gameplay-2026-10-15/README.md)
dokumentiert: separate Mikrodetaildistanz (32 m, Hysterese 28/36 m), feinere Halme,
Stufen/Schwimmen/Third Person, Schnee-/Feuerjournal und gebatchter Niederschlag.

Alle Menues verwenden ausschliesslich die bestehenden TiltUI-Komponenten unter
`src-ui/`: Main Menu, Singleplayer, Multiplayer, Options (Graphics/Controls/Audio),
Game Menu (ESC), Loading Screen, Inventory/Materials, Survival-HUD und Dialoge. Gameplay ist waehrend der Navigation
gesperrt. Das Game Menu liegt ueber der laufenden Welt: Resume, Options, Save
World, Save & Main Menu, Quit sowie echte Welt-/Profil-/Sitzungsdaten. Save World
wartet asynchron auf den integrierten Host; im Multiplayer speichert allein der
Server und dieser Button ist deaktiviert. Die Serversimulation laeuft im Menue weiter.
Der Loading Screen zeigt Verbindungsphasen und messbaren Terrain-/Mesh-Fortschritt;
das Gameplay wird erst nach initialer Synchronisierung, Generierung und Meshing frei.

`configs/graphics.toml`, `configs/controls.toml`, `configs/audio.toml`,
`configs/profile.toml` und `configs/servers.toml` werden
beim ersten Start mit Defaults erzeugt, validiert geladen und atomar gespeichert.
`TERRA_CONFIG_DIR` setzt den gemeinsamen Konfigurationsordner; optional setzt
`TERRA_MENU_CONFIG_DIR` nur fuer Menue-Praeferenzen einen abweichenden Ordner.
Bestehende Grafikpraeferenzen aus `configs/client.toml` werden einmalig uebernommen;
diese Datei bleibt die Basis fuer technische Worker-/Renderbudgets. Danach sind die neuen Dateien
die Quelle fuer die im Menue einstellbaren Praeferenzen. Defekte Dateien werden
nicht stillschweigend ueberschrieben; ein sichtbarer Fehler begleitet den Fallback.

**Apply** speichert und aktiviert, **Reset** setzt den aktuellen Tab nur im Entwurf
zurueck. **Back** fragt bei ungespeicherten Aenderungen nach. Aufloesung und
Fenstermodus werden 15 Sekunden vorgeschaut und ohne Bestaetigung zurueckgesetzt,
einschliesslich gleichzeitig geaenderter Audio- und Controls-Einstellungen.
Exklusives Vollbild verwendet einen tatsaechlich unterstuetzten Monitor-Modus;
Borderless verwendet die Desktopaufloesung. Die UI-Skalierung wird auf kleinen
Fenstern begrenzt, damit Dialoge und Aktionsleisten erreichbar bleiben.

Alle vorhandenen Bewegungs-, Bau-, Mausfreigabe- und Menueaktionen sind als
Tastatur- oder Maustasten neu belegbar. Escape bricht die Erfassung ab, ohne das
Menue zu schliessen; Konflikte werden abgewiesen. Inventory/Materials (Default E)
und Kameraperspektivwechsel (Default F5) sind ebenfalls rebindbar. Noch nicht
vorhandene Aktionen wie ein allgemeines Interact bleiben deaktiviert markiert.
Alte Controls-Dateien erhalten fuer Inventar eine freie Belegung, ohne bestehende
Tasten zu ueberschreiben. Debug, Blockgrenzen und Chat sind ebenfalls frei
belegbar; Kombinationen wie F1+B werden als zwei Tasten erfasst.

Welten liegen unter `worlds/<Name>/` (`TERRA_WORLD_DIR` ist optional).
`world.toml` erweitert das vorhandene WorldManifest um Titel, Spielversion,
Spielmodus und Last Played. Sichere eindeutige Ordnernamen aendern keine Welt-ID;
alte Hex-ID-Ordner werden beim Oeffnen namensbasiert verschoben.
`regions/r.<x>.<z>.region` speichert binaer die Section-Edits, Prop-Zustaende,
Wasserquellen/Fuellstaende, Bodenzustaende, Containerinhalte, Ofenzustaende und lose Ressourcen fuer jeweils
32x32 Chunk-Spalten, einschliesslich aller Y-Sections. Negative Koordinaten nutzen
euklidische Division. `players/<UUID>.data` speichert getrennt Inventar, Cursor,
Revisionen, echte serverseitige Abbau-/Bau-/Prop-Erntestatistiken, Todeszaehler
und Gesundheit/Ausdauer/Hunger/Sauerstoff. Neue Spielerdateien verwenden `PLY7`
einschliesslich Saettigung, Nahrungsregeneration, Status-Effekten, Schildrest und Ausruestung;
explizite Leser fuer `PLY6`, `PLY5`, `PLY4`, `PLY3`, `PLY2` und `PLYR` erhalten Inventare und alte Zaehler.
Neue Regionen verwenden `RGA3` mit Crusher-/Anbauzustaenden, Tiervertrauen und
Spinnennetz-Zuordnungen; `RGA2`, `RGA1`, `RGA0`, `REG9`, `REG8`, `REG7`, `REG6`, `REG5`, `REG4`, `REG3`, `REG2` und `REGN` bleiben lesbar.
`level.data` enthaelt nur den kleinen Index und globale Drop-Zaehler/Revisionen.
Das ist ein versioniertes TERAMIC-Format, keine Minecraft-/NBT-Dateikompatibilitaet.
Das Welt-Speicherformat ist deshalb jetzt 2, der interne Binaerheader hat Version 1.
Generatorversion 5, Seeds und Welt-/Block-IDs bleiben gleich. Der aktuelle
Netzwerkstand ist Protokoll 65; Client und Server gemeinsam aktualisieren.
Version-1-Metadaten werden mit Sicherung vor dem Datentransfer auf 2 gesetzt,
damit alte Programme die Regionsdateien nicht als leere Editliste missverstehen.
Die bereits vorhandenen Postcard- und Bevy-Math-Versionen werden direkt genutzt;
keine Dependency-Upgrades. Regiondaten nutzen Postcard-Varints, Typ-/Versionsheader,
Laengenpruefung und nichtkryptografische Integritaetspruefsummen.

Integrated und Dedicated verwenden denselben Speicherpfad. Geaenderte Dateien
werden vorbereitet und fsync-gesichert; ein Commitplan wird zuletzt freigegeben.
Ein unterbrochener Commit wird vor Laden vollstaendig fertiggestellt, ein noch
nicht freigegebener verworfen. Unveraenderte Regions-/Spielerdateien werden nicht
neu geschrieben. Alte `edits.toml` werden erst vollstaendig validiert, dann
uebernommen; die Originaldatei bleibt als `.legacy-backup`. Fehlerhafte Binaerdaten
fallen nicht auf einen veralteten TOML-Stand zurueck. Fuer Backups Server stoppen.
Auch ein geloeschter Index bei noch vorhandenen Regionen/Spielerdaten ist ein
sichtbarer Fehler und startet keine leere Ersatzwelt.
Spaetere Dimensionen koennen ihre Regionen unter `dimensions/<ID>/` erhalten;
derzeit gibt es nur die vorhandene Hauptwelt, kein vorgetaeuschtes Dimensionssystem.
Der integrierte Host laedt vor seinem ersten Tick, speichert alle zehn Sekunden
im Hintergrund und
beim Verlassen. Kein zweites Edit-System und keine Generatorversionsaenderung.
Der explizite `GameMode` stammt im Singleplayer aus den Weltmetadaten und beim
Dedicated Server aus `server.game_mode` (`"survival"` oder `"creative"`, Default
`"survival"`). Er wird nicht aus Flugrechten abgeleitet. Creative baut ohne
Haltezeit ab und verbraucht keine Ressourcen; Survival verwendet serverseitige
Abbauzeiten, persistente Ressourcen und Materialverbrauch beim Bauen. Drei
persoenliche 2x2-Rezepte und autoritative Survival-Spielerwerte sind vorhanden;
Werkbaenke und Werkzeuge mit eigenen Abbauwerten sind noch nicht implementiert.
Regeln und Bedienung stehen unter **Player Stats Und Survival (Phase 43)**.

Technische Berichte, native Aufnahmen und Testnachweise:
[UUID, LAN und Game Menu](tools/benchmarks/results/lan-menus-2026-10-15/README.md),
[urspruengliches Menu UI](tools/benchmarks/results/menu-2026-10-15/README.md).

Konfigurationen werden unabhaengig vom Arbeitsverzeichnis aus dem Repository
geladen. `TERRA_CONFIG_DIR=/pfad/zu/config` erlaubt einen anderen Config-Ordner.
Fehlende Dateien sind Fehler; fehlende Felder erhalten Defaults. Unbekannte
Felder und ungueltige Werte werden abgelehnt.
`TERRA_ASSET_DIR=/pfad/zu/assets` erlaubt externe Assets fuer ausgelieferte Binaries.

## Bedienung

| Eingabe | Aktion |
| --- | --- |
| WASD | Laufen; im Flugmodus frei bewegen |
| Maus bewegen | Frei umschauen, ohne eine Maustaste zu halten |
| Leertaste | Auf dem Boden springen; im Flugmodus steigen |
| Linke Shift-Taste | Im Flugmodus sinken, kein Sprint |
| Linke Ctrl-Taste | Beim Laufen sprinten; Flugtempo x3: 60 m/s |
| Linke Alt-Taste | Im Flugmodus Praezision x0.25: 5 m/s; hat Vorrang vor Ctrl |
| Tab | Maus freigeben / wieder einfangen |
| Linksklick bei freier Maus | Maus wieder einfangen |
| Linke Maustaste | Survival: zum Abbauen halten; Creative: einzelnen Block oder Prop sofort abbauen |
| Mausrad | Block aus der Asset-Bibliothek auswaehlen |
| Mittlere Maustaste | Anvisierten Block in Creative kopieren; vorhandenen Hotbarstapel bevorzugen |
| Q / Shift + Q | Eine Einheit / den ganzen aktuellen Stapel in die Welt werfen |
| R | Lebensmittel im ausgewaehlten Hotbarplatz essen, halten; bisherige Benutzen-Belegung bleibt erhalten |
| B | Ausgewaehlten Block setzen oder Lebensmittel essen, zum Essen halten; folgt Place Block / Eat in Controls |
| Shift + B | Essbaren Pilz platzieren statt essen; Shift zusaetzlich zur Place-Belegung halten |
| E | TiltUI-Inventar / Creative-Materialauswahl oeffnen oder schliessen (rebindbar) |
| F1 | Debug-Overlay ein/aus (standardmaessig aus; Schalten beim Loslassen, rebindbar) |
| F1 + B | Gespeichertes Debug-Platzierungsflag umschalten; keine Vorschau fuer Vollbloecke (rebindbar) |
| F | In Creative zwischen Laufen und Fliegen wechseln |
| F5 | First Person -> Rueckansicht -> Vorderansicht -> First Person (rebindbar) |
| Escape | Pause/Menu; in Options Back mit Verwerfungsabfrage |
| Fenster schliessen | Client beenden |
| Ctrl+C / SIGTERM | Server geordnet beenden |

Das Fadenkreuz und der Auswahlrahmen zeigen das naechste sichtbare Block-/Prop-Ziel.
Reichweite: 8 Meter. B setzt Bloecke an Blockflaechen; Pflanzen sind keine Bauflaechen.
Essen folgt derselben gespeicherten Place-Belegung, auch einer Maustaste oder
Tastenkombination; keine Default-Belegung wurde geaendert. Gezieltes Oeffnen
von Stationen und Fuettern von Tieren hat Vorrang vor Place-Essen.
Ist Shift bereits Teil der gespeicherten Place-Belegung, bleibt diese zum Essen nutzbar.
Nur ausrichtbare Bauten erhalten eine pulsierende orange Modellkontur als
Platzierungsvorschau. Normale Vollbloecke wie Stein oder Dirt erhalten keine;
gefuellte gruene/rote Vorschauflaechen gibt es nicht mehr, auch nicht mit F1+B.
In Survival ist dafuer ein Exemplar des ausgewaehlten Blocks im Inventar noetig;
Creative stellt alle registrierten Baumaterialien ohne Verbrauch bereit.
Im geoeffneten Inventar sind Blick, Bewegung, Abbau und Platzierung gesperrt;
Escape schliesst zuerst das Inventar, ohne zugleich das Game Menu zu oeffnen.
Das TiltUI-Overlay zeigt FPS, App-CPU, App-RAM, Chunk, XYZ und den anvisierten
Block-/Prop-Namen. Standard ist eine First-Person-Laufsteuerung
mit Kapsel-Collider, Schwerkraft, Bodenkontakt und Sprung. Laufen verwendet
5.4 m/s, Sprint 8.2 m/s; der Debug-Flug bleibt in Creative verfuegbar.
Optionaler Einblock-Stufenaufstieg, Schwimmen mit Auftrieb und eine kollisionsbegrenzte
Third-Person-Kamera sind implementiert. Im Wasser Jump halten zum Auftauchen,
Fly Down (standardmaessig Shift) zum Tauchen. Vorwaerts folgt beim Schwimmen
auch der Blickneigung. Jump an der Oberflaeche ermoeglicht den Uferausstieg.
Beide Aktionen folgen den gespeicherten Controls. Die Kameraposition wird zwischen
Physikticks interpoliert; lokale Netzwerkkorrekturen werden nur visuell geglaettet.
**Options > Controls > Auto-Climb** ist standardmaessig ausgeschaltet.
Apply aktiviert und speichert `auto_climb` in `configs/controls.toml`; Reset
schaltet es im Entwurf aus. Der gemeinsame Client-/Server-Controller uebernimmt
diese Wahl pro Eingabepaket, auch bei Vorhersage und Replay. Ein bewusster
Schwimmsprung zum Ufer funktioniert unabhaengig davon. Client und Server
muessen mit demselben Protokollstand (aktuell 65) gemeinsam aktualisiert werden.
`mouse_sensitivity`, `fly_speed` (jetzt 20 m/s), `fly_boost_multiplier` und
`fly_precision_multiplier` sind in client.toml konfigurierbar. Beschleunigung
wird mit einer kurzen exponentiellen Reaktion zeitlich exakt integriert.
Diagonale Bewegung ist nicht schneller; Loslassen stoppt ohne Nachgleiten.
Bei Fokusverlust wird die Maus freigegeben und die Bewegung ausgesetzt.

### Einstellungen

Das ESC-Menue bleibt auch ohne Debug-Overlay verfuegbar. Es bietet vertikales
Sichtfeld 45..110 Grad, Sichtweite 2..32 Chunk-Spalten (32..512 m Radius),
Mausempfindlichkeit, Lauf-/Flugmodus, VSync, Debug-Anzeige, Wasserqualitaet,
Szenenreflexionen und Wellenstaerke. Der bisherige FOV-Default 45 Grad bleibt
erhalten; ein neues visuelles Standardprofil wird dadurch nicht festgelegt.
Kamera, Fenster-Praesentationsmodus und Rendering-/Streaming-Konfiguration
folgen der Live-Vorschau. Bei groesserer Sichtweite muessen neue Sections und
Render-Batches weiterhin asynchron geladen werden.

`Uebernehmen` setzt einen neuen Ruecksetzpunkt fuer diese Sitzung, `Fortsetzen`
schliesst das Menue und behaelt die Vorschau. ESC oder das Schliessen-Symbol
stellen den Stand beim Oeffnen bzw. letzten Uebernehmen/Speichern wieder her.
`Standardwerte` setzt nur die angebotenen Client-Optionen zurueck, keine Seeds,
Weltparameter oder Worker-Budgets. `Speichern` schreibt die validierte
Client-Konfiguration nach `TERRA_CONFIG_DIR/client.toml` bzw. `configs/client.toml`:
temporaere Datei im selben Ordner, Synchronisierung und atomarer Austausch.
Fehler bleiben im Menue sichtbar und beenden das Spiel nicht. Der gewaehlte
Lauf-/Flugmodus gilt nur fuer die Sitzung und wird nicht mitgespeichert.

Unter Umgebung stehen die Wetterqualitaet sowie, bei einem beim
Oeffnen anvisierten Grassblock, lokale Bodenbefehle zur Verfuegung. Dies sind
Darstellungs-/Session-Werkzeuge. Seit Phase 72 bestimmt der Server das Wetter;
die Grafikoption beeinflusst nur dessen Darstellung. Gezielte Schneedecke und
autoritative Feuer-Ausbreitung bleiben erhalten.
Bodenzustandsbefehle veraendern das Weltjournal unmittelbar und werden nicht
durch den Abbruch einer Client-Einstellung zurueckgenommen. Im geoeffneten
Menue ist die Maus frei; Blick-, Bewegungs-, Abbau- und Platziereingaben sind
gesperrt. Die Welt einschliesslich Schwerkraft laeuft weiter.

## Chunk- und Abbausystem

`SectionPos` verwendet i64. X/Z identifizieren die 16 x 16-Spalte,
Y die vertikale Section. Negative Koordinaten verwenden euklidische Division:
Block `[-1,-1,-1]` liegt in Section `[-1,-1,-1]`, lokal `[15,15,15]`.
Jeder Block besitzt genau einen Eigentuemer. Indexierung ist X, dann Y, dann Z.
Materialspeicher, Zugriff, Koordinaten-Overflow und Revisionen sind geprueft.

Der geladene Bereich folgt der Kamera. `view_distance` ist der horizontale
Radius in Spalten, `vertical_view_distance` der vertikale Radius in Sections.
Default: 12 horizontal und 3 vertikal, also 4375 Sections im kamera-zentrierten Basisbereich.
Zusaetzlich werden die Oberflaechen-/Kronenbereiche sichtbarer Spalten nachgeladen.
Die Hoehenbereiche stammen aus den ohnehin auf Workern berechneten Oberflaechenspalten;
es gibt keine blockierende Chunk-Generierung im Renderthread. Sie werden pro Spalte
zwischengespeichert und bei Bewegung/Entladen erneut ins Ladeinteresse aufgenommen.
Damit bleiben tiefe Taeler auch unter einer hohen Flugkamera sichtbar. Der feste
Kuesten-Smoke-Test benoetigt 4885 Sections statt 4375. Die Gesamtzahl ist terrainabhaengig.
Der vertikale Radius wurde fuer die hoeheren Baumkronen um eine Section erweitert.
Das sind 192 statt zuvor 64 Meter horizontaler Sichtweite. Der Nebel beginnt
bei etwa 125 Metern und ist bei 179 Metern vollstaendig. Die Kamera-Clipweite
folgt der Sichtweite. Die hoehere Distanz braucht mehr Ladezeit und Speicher;
Worker-/Upload-Budgets bleiben begrenzt. `view_distance` erlaubt 0..=16.
Vertikale Koordinaten sind nicht auf eine feste Minecraft-Welthoehe begrenzt;
nur die Sections im aktuellen Sicht-/Simulationsbereich werden gehalten.

`max_chunk_jobs` begrenzt parallele Worker (Default 4). Ein Job bearbeitet bis zu
16 Sections derselben Spalte. Hoehen, Bodenschichten und Baumkandidaten werden
einmal vorbereitet und als unveraenderliche Spaltendaten wiederverwendet;
Edits werden nicht darin gecacht. Bei Radius 12 bleiben maximal 625 Spalten im
Cache; Entladen entfernt auch ihre Vorlagen. Leere Sections oberhalb der
Oberflaeche werden ohne teure Dichte-/Baumrasterisierung fertiggestellt.
Nahe Sections werden zuerst angefordert. Bewegung ueber Section-Grenzen aktualisiert das Interesse,
entfernt entfernte Daten und bricht nicht mehr benoetigte Jobs ab.
`max_mesh_jobs = 16` begrenzt die separaten Terrain-Meshworker;
`mesh_uploads_per_frame = 12` begrenzt Terrain-GPU-Uploads pro Frame.
Entladene Mesh-Assets werden freigegeben.

Fertige Terrain-, Prop- und Wasser-Meshes verwenden
`RenderAssetUsages::RENDER_WORLD`. Bevy verwirft ihre CPU-Vertex- und Indexdaten
nach dem GPU-Upload; Mesh-Handles und `final_aabb` fuer das Culling bleiben
erhalten. Die logischen Voxeldaten, Generatorcaches und wiederverwendbaren
CPU-Geometrievorlagen bleiben verfuegbar. Diese Aenderung betrifft die fertigen
Render-Meshes, nicht die Datenbasis fuer Abbau oder erneutes Meshing.

Block-Generierung und Greedy-Meshing laufen im `AsyncComputeTaskPool`.
Meshing verwendet eine schreibgeschuetzte 18^3-Umgebung: 16^3 eigene Bloecke
plus je einen Nachbarblock als Rand. Verdeckte Flaechen werden entfernt,
benachbarte Flaechen desselben Materials zusammengefasst. Ein Mesh-Quad kann
mehrere Bloecke darstellen; der Raycast arbeitet trotzdem auf einzelnen Zellen.
Die sichtbaren Flaechen werden in einem zweiten Worker-Schritt gemeinsam
zusammengesetzt. Alle 19 normalen Materialien verwenden denselben
prozeduralen Cube-/Face-Generator. Im Nahbereich bleiben abgerundete Aussenkanten;
Nachbar-Padding zieht verbundene Bevelbaender auf die exakte Zellgrenze.
Beruehrt ein Vertex eine ausgeblendete Nachbarflaeche, werden alle seine
Randkoordinaten auf die kanonischen Zellgrenzen gesetzt, nicht nur die zur
Nachbarflaeche senkrechte Achse. Freie Aussenkanten behalten ihre Rundung.
Mittlere und entfernte Cube-Geometrie nutzt die vorhandenen Greedy-Rechtecke.
Dreiecke werden pro Section und Material gebuendelt, ohne Entity pro Block.
Veraltete Mesh-Epochen werden verworfen, alte GPU-Meshes erst beim Ersatz entfernt.
Degenerierte Bevel-Dreiecke werden nicht hochgeladen. Logische Bloecke bleiben
einen Meter gross; kein LOD aendert die Material-ID oder Voxelbelegung.

### LOD und Materialfehler

Die Grass-zu-Dirt-Ursache lag im alten Fernpfad: Er waehlte fuer eine ganze
Flaeche das Material ihres ersten GLB-Primitives. Eine Grass-Flaeche kann mehrere
Primitives fuer Erde und Gras enthalten; deren Reihenfolge bezeichnet keine
Oberflaechenart. Zusaetzlich verschwand der gruen gefaerbte Seitenrand.
Cube-Definitionen binden jetzt fuer alle sechs Seiten explizite Materialien;
`side_cap` behaelt den Gras-/Schneerand auch auf groben Seitenflaechen.
Custom-Modelle behalten ihre vollstaendige Mehrmaterial-Geometrie in jeder Stufe.

| Stufe | Terrain | Terraindarstellung | Props |
| --- | --- | --- | --- |
| Near / LOD 0 | 0..144 m | Cube-Kanten, fein gegliederte Laubgruppen | 0..80 m: feine Halme, Blueten, Farnblaetter |
| Middle / LOD 1 | 144..240 m | Greedy-Flaechen, vereinfachtes Micro-Laub | 80..144 m: vereinfachte Silhouetten aller Arten |
| Far / LOD 2 | ab 240 m | Greedy-Flaechen mit gleichen Materialien | ab 144 m: reduzierte Geometrie aller Arten |

Gemessen wird horizontal zum naechsten Punkt des Section-Grundrisses, nicht zur
Mitte. `lod_near_distance`, `lod_far_distance` und `lod_hysteresis` sind validierte
Clientoptionen. Seit Phase 34 werden gespeicherte Terrainwerte einschliesslich
Hysterese mit 1.5 multipliziert; bestehende relative Einstellungen bleiben
erhalten. Aus den Basiswerten 96/160/12 entstehen 144/240/18 m: Near bleibt beim
Entfernen bis 162 m und kehrt unter 126 m zurueck, fuer Far gelten 258/222 m.
Die rein visuellen Blockreliefs wechseln ebenfalls 50 Prozent weiter draussen:
48 m initial, 54/42 m mit Hysterese. Prop-Distanzen werden nicht mitmultipliziert.
Terrain und Props behalten jeweils ihre eigene vorherige Stufe. Jobs und fertige
Uploads priorisieren nahe Sections. Der Geometriecache vermeidet erneute
Voxelgenerierung bei LOD-Wechseln. Wasser besitzt ein separates, unveraendertes
Gitter. Drei ueberlappende Sonnen-Schattenkaskaden reichen bis maximal 128 m;
entfernte Terrain-/Prop-Batches werfen keine Schatten.

`prop_lod_near_distance` und `prop_lod_far_distance` entkoppeln die Prop-Stufen
vom Terrain; dieselbe Hysterese verhindert Flattern. Auch Gras, Blumen, Farne
und Pilze behalten im Far-LOD eine reduzierte Silhouette. Prop-Batches verwenden
`VisibilityRange::abrupt(0.0, view_distance * 16.0)`, standardmaessig bis 192 m.
Diese harte Ferngrenze liegt hinter dem bei rund 179 m vollstaendigen Nebel;
es gibt weder einen Distanz-Dither noch eine Alpha-Ausblendung.

Terrain- und Prop-Batches wechseln atomar, sobald ein erfolgreiches Worker-
Ergebnis mit aktuellen Epochen und passender LOD-Stufe vorliegt. Ausstehende
oder veraltete Jobs lassen das bisherige Mesh bestehen. Entfernen und Ersetzen
der Batch-Entities erfolgt in demselben Deferred-Command-Commit, ohne zeitliche
Ueberlappung zweier Meshstufen. `LodSwapStatus.completed` zaehlt diese Wechsel.
Das fruehere Dithering wurde wegen inkonsistenter Tiefen-/Schattenmaskierung im
verwendeten Bevy-0.19-Depth-only-Prepass entfernt; es gibt dafuer keine eigene
Pipeline oder zusaetzlichen Render-Targets.

Bewusster Kompromiss: Der geometrische Stufenwechsel kann weiterhin abrupt
sichtbar sein. Stabile Materialien, groessere LOD-Distanzen und Hysterese bleiben
erhalten, waehrend das Dither-bedingte Flackern und seine Tiefenloecher nicht
mehr durch diesen Renderpfad entstehen. Echtes GPU-Instancing bleibt offen.

### Block-Assets und Texturen

`render_kind = "cube"` benoetigt keine GLB-Datei. JSON-Schema 1 und IDs 0..10
bleiben erhalten, neue Materialien verwenden 11..26; fehlendes `render_kind` interpretiert historische Definitionen
weiterhin als `custom_mesh`. Gleiche Textur-/Farb-/Rauheitsparameter teilen sich
einen `StandardMaterial`-Handle, gleiche PNG-Pfade/Texturmuster einen Image-Handle. Die 26
Cube-Typen verwenden zusammen 28 Basismaterialien; Props teilen eine kleine
zusaetzliche Palette. Das reduziert Assetduplikate, ist allein kein FPS-Nachweis.

`textures/grass-neutral.png` und `textures/canopy-stylized.png` sind die aktiven,
mit der integrierten Bildgenerierung (`image_gen`) erzeugten Bitmap-Texturen.
Grass verwendet eine neutralere Basis fuer biomeabhaengige Farbe;
`grass-stylized.png` bleibt ebenso wie alte PNGs als Quellmaterial erhalten.
Die unbenutzten Standard-GLBs wurden entfernt.
Gemeinsame Laubtextur und
getrennte Materialfarben bewahren gruene, orange-herbstliche und helle
Birkenkronen. Es werden keine Block-IDs ersetzt.

PNG-Texturen bleiben einzeln wiederholbar. Greedy-UVs verwenden lokale
Zellkoordinaten, damit eine Textur nicht ueber das gesamte Rechteck gestreckt wird.
Auch abgeflachte Cube-Kanten projizieren UVs aus ihren endgueltigen Positionen.
Die vier PNGs erhalten auf Workern vollstaendige Mipmap-Ketten (10 bzw. 11
Stufen), in linearem Farbraum mit vormultipliziertem Alpha gefiltert. Erst danach
werden sie unter denselben Image-Handles fuer die GPU freigegeben; abhaengige
Materialien werden explizit neu gebunden. Das vermeidet veraltete Single-Mip-
Texturansichten. Trilineare Filterung und Anisotropie 4 reduzieren Fernflimmern.
Ein Atlas benoetigte Padding, mipfeste Raender und besondere Wiederholungslogik;
ein Texture Array benoetigte Layerattribute, Bildaufbereitung und eine eigene
PBR-Shadererweiterung. Ohne nachgewiesenen Engpass bleibt die bestehende
StandardMaterial-Pipeline erhalten. Es gibt kein Atlas-Bleeding, aber weiterhin
Draws je sichtbarem Section-Material, keinen gemeinsamen Draw fuer alle Texturen.

Sand verwendet feine Koerner statt der Steintextur. Sandstein (11), glatter Stein
(12), Mossy Stone (13), fester Wasserblock (14), gerahmtes Glas (15), Oak-/Birch-/
Spruce-Planks (16..18) und Lehm (19) ergaenzen die Palette. Die JSON-Option
`texture_pattern` erzeugt sieben deterministische, gemeinsam genutzte 64er-
Texturen mit sieben Mipstufen. Wuesten erhalten Sandsteinschichten, nasse
Sandboeden unregelmaessige Lehmvorkommen und geeignete feuchte Hoehleneingaenge
vereinzelt Mossy Stone. Glatter Stein, Glas, Planks und fester Wasserblock werden
nicht generiert. Erste Herstellungsrezepte sind seit Phase 45 integriert.
Die Materialbelegung alter uneditierter Regionen kann sich dadurch aendern;
Seed, Generatorversion 5 und gespeicherte Edits bleiben erhalten. Die spaetere
Hoehlen-/Biomerweiterung aendert auch Terrainformen, siehe folgenden Abschnitt.

Pilzstamm (20) besitzt eine cremeweisse, braeunlich faserige Textur. Pilzkappen
in Lila, Rot, Gelb, Gruen, Blau und Weiss (21..26) teilen Form und helle
Lamellenunterseite, besitzen aber eigene gesprenkelte Oberseiten. Acht weitere
64er-Muster nutzen dieselbe Registry und Mip-Pipeline. Alle sieben neuen
Bloecke sind normale solide Cubes: abbaubar, platzierbar, speicherbar und als
echtes GPU-Blockbild im Inventar sichtbar. Keine zusaetzlichen GLB-Modelle.

Glas besitzt pro sichtbarer Flaeche 10 Prozent Opazitaet und einen 3.5 cm breiten
Holzrahmen nur am Rand zusammenhaengender Glasfelder. Nachbarpadding entfernt
innere Flaechen und Rahmen auch ueber Section-/LOD-Grenzen. Mehrere ueberlagerte
Scheiben verdichten optisch; Bevy sortiert weiterhin transparente Section-Batches,
nicht jedes Dreieck. Der feste Wasserblock kollidiert wie ein normaler Cube,
verwendet aber das vorhandene Wasser-Material mit animierten Normalen und
begrenzter optischer Dicke. Er ist weder Fluidquelle noch Schwimm-/Nebel-/Audio-
Wasser und funktioniert auch bei abgeschaltetem natuerlichem Wasser.
Glaspane und Rahmen werfen keinen Schatten; beide tatsaechlichen Batchtypen
tragen `NotShadowCaster`.

Der zusaetzliche `Fluid Water Block` (27, `teramic::fluid_water`) ist dagegen
eine echte Quelle im vorhandenen Zellwassersystem. Sein anwaehlbarer Anker ist
nichtsolid und erzeugt keine Terrain-Cubeflaechen. Platzieren setzt einen vollen
Source-Fuellstand im bestehenden Journal; Nachbarwasser nutzt abnehmende Flusspegel.
Entfernen stoppt den Nachschub, ohne ausgelaufenes Wasser sofort zu loeschen.
Ein anderer Block wird direkt in die Quellenzelle gesetzt und verdraengt sie,
statt auf ihr zu stapeln. Gewoehnliches Zell-/Flusswasser wird beim Bauen gegen
Untergrund in seiner Luftzelle verdraengt. Clientvorschau und Server verwenden
denselben Zielvertrag; feste Bloecke bleiben gegen direktes Ueberschreiben
geschuetzt. Solid Water (14) ist weiterhin ein normaler fester Bauuntergrund.
Inventarbild und Drops verwenden die geteilte animierte Wasservorlage, ohne GLB.
Die Inventarvalidierung unterscheidet deshalb registrierte Items von solider
Kollision: Auch der nichtsolide Wasserquellenblock ist sammel-/speicherbar.

Grass-Farbe wird im linearen Farbraum aus den fuenf vorhandenen Biomgewichten
gemischt. Vier Klimasamples an gemeinsamen Section-Ecken ergeben kontinuierliche
Vertexfarben, auch an negativen Koordinaten. Oberseite und seitliches Grassband
nutzen dieselbe Zuordnung unabhaengig vom Terrain-LOD. Gruene Prop-Blaetter und
Stiele uebernehmen die relative Biomtoenung; Blueten behalten ihre Grundfarben.
Terrain und Props verwenden dasselbe Section-Farbfeld, ohne erneute
Klimaabfrage fuer jede Pflanze. Interpolation innerhalb grober Dreiecke bleibt
eine Naeherung an die feinere Nahgeometrie.

Globale Regen-/Schnee-Darstellung aktualisiert die gemeinsam genutzten
Grass-/Prop-Materialien, ohne die Welt neu zu meshen. Lokale Feuchte und
Verkohlung liegen dagegen im bestehenden `EditJournal` und ueberleben
Section-Unload/Reload innerhalb der Sitzung. Eine eigene `appearance_epoch`
invalidiert nur betroffene Terrain-/Prop-Batches; Worker nutzen die vorhandene
Geometrie wieder. Nur betroffene Greedy-Flaechen werden nach Bodenzustand
unterteilt und gleichartige Nachbarzellen wieder zusammengefasst. Eine einzelne
geaenderte Zelle auf einer 16x16-Flaeche benoetigt hoechstens fuenf Rechtecke.
Wettereinstellungen erzeugen keine
Partikel, Schneedecke, Feuer-Ausbreitung oder neue Spielmaterial-IDs.

`custom_mesh` unterstuetzt zwei kompatible Layouts. `model_layout = "face_groups"`
ist der bestehende Standard mit sechs benannten Flaechengruppen und optionalen
Details. `model_layout = "scene"` importiert die vollstaendige statische
GLB-/GLTF-Standardszene, einschliesslich Hierarchie, Instanzen, Materialien,
Translation, Rotation und nichtuniformer Skalierung. Normalen werden mit der
invers-transponierten Matrix transformiert, Spiegelungen korrigieren das Winding.
Die transformierte Szene muss in [0,1] passen und bleibt eine logische Blockzelle.
Sie verdeckt keine Nachbarflaechen; ihr vollstaendiges Modell wird einmal pro
belegter Zelle in bestehende Section-Batches eingefuegt und bleibt in allen LODs
erhalten. Animationen, Skinning, Morph Targets und bewegliche Maschinen bleiben offen.
Der statische Import weist solche Assets ausdruecklich zurueck, statt eine
eingefrorene Animation als Unterstuetzung auszugeben. Die gelieferten Cube-/
Spezialassets enthalten keinen Bedarf fuer einen zweiten Live-Szenenrenderer;
dessen Integrationspunkte sind Szenenimport und Custom-Mesh-Snapshots, nicht
die gemeinsame Cube-Pipeline. Es wurde kein ungetestetes Animationssystem hinzugefuegt.
Der genaue kompatible Vertrag steht in
[assets/terra/blocks/README.md](assets/terra/blocks/README.md).

Ein autoritativ bestaetigter Abbau veraendert den Block-Datensatz und das Edit-Journal.
Creative schickt den Edit ohne vorgeschaltete Haltezeit, Survival wartet auf den
serverseitigen Abbaufortschritt. Bei einem
Randblock werden auch betroffene direkte Nachbar-Sections neu gemesht.
Gemeinsame X/Z-Ecken invalidieren zusaetzlich die horizontale Diagonal-Section,
weil Wasser-Uferdaten vier benachbarte Zellen pro Vertex verwenden.
Block-Revision und Mesh-Epoch sind getrennt. Ein Job mit veralteter Epoch
wird verworfen, sodass er neuere Edits nicht ueberschreiben kann.

Das Journal ueberlebt **Entladen, erneutes Laden und gespeicherte Sitzungen**.
Neue Sections und Meshing-Raender wenden diese Edits erneut an. Integrated und
Dedicated Server verwenden denselben transaktionalen Region-/Spielerspeicher;
bewusst mit `--no-save` gestartete Dedicated Server speichern nicht auf Disk.

### Abbau, Bauen und Ressourcen

Survival-Abbau verwendet `MiningIntent` und `MiningProgress` statt direkt
ausgefuehrter Loeschkommandos. Der Server prueft jeden Tick Ziel, Sichtlinie,
Reichweite, Revision und Baurechte erneut. Loslassen, Zielwechsel, Menue,
ungueltiges Ziel oder ein ausbleibender Heartbeat brechen den Vorgang ab.
Die aktuellen Haltezeiten sind 1.25 s fuer Steinvarianten/verwittertes Holz,
0.9 s fuer Schnee, 0.8 s fuer Planks, 0.7 s fuer Lehm, 0.4 s fuer Glas/festes
Wasser, 0.3 s fuer Laub, 0.55 s fuer die anderen normalen Bloecke und 0.25 s
fuer Props. Creative hat keine solche Haltezeit; Loeschungen bleiben trotzdem
servervalidiert und werden erst nach Bestaetigung im Client wirksam.

Die Spielerhand und das First-Person-Viewmodel sind entfernt. Beim Survival-Abbau
normaler Cubes ersetzt ein begrenzter Worker voruebergehend genau den betroffenen
Section-Batch: Der Zielblock wackelt, einzelne Bruchstuecke loesen sich mit
wachsendem Fortschritt. Kern und Rand sowie zuvor verdeckte Nachbarflaechen
verhindern Durchblicke. Abbruch stellt das unveraenderte Mesh wieder her; nach
Serverbestaetigung bleibt die Ersatzsection bis zum regulaeren Meshupload aktiv,
damit der alte Block nicht kurz erneut erscheint. Maximal ein Job und eine
Ersatzsection sind gleichzeitig aktiv, keine synchronen Gesamtwelt-Rebuilds.
Seit Phase 49 koennen auch importierte statische Modelle ihre wirklichen
Oberflaechenstuecke verlieren; Zwischenraeume bleiben offen und unveraenderte
Modelle derselben Section erhalten. Eine verbundene Doppeltruhe zerlegt nur
die ausgewaehlte Haelfte. Das ist eine kosmetische Darstellung; die logische
Kollision aendert sich erst beim echten Edit.
Creative entfernt weiterhin sofort ohne Haltezeit.
Die fruehere rote/gruene Platzierungsbox ist durch die orange Modellkontur fuer
ausrichtbare Bauten ersetzt. Normale Vollbloecke bekommen auch mit F1+B keine
Box. Menues und Texteingaben sperren diese Debug-Tasten.
Die Kombination aktiviert nicht gleichzeitig eine auf B gebundene Spielaktion.
Debug-Overlay, Platzierungsbox, Chat und Command sind eigene Controls-Aktionen.
Neben einzelnen Tasten und Maustasten sind Zwei-Tasten-Kombinationen moeglich;
aktive Kombinationen unterdruecken die gleichzeitig belegte Einzelaktion.
Alte Controls-Dateien erhalten freie Tasten, ohne vorhandene Belegungen zu
ueberschreiben. Konflikte, auch umgekehrt eingegebene Kombinationen, werden
erkannt. Escape bricht Rebinding ab und schliesst nicht zugleich das Menue.
Normaler Betrieb protokolliert auf Info; wiederkehrendes Streaming-Ready liegt
auf Debug. `RUST_LOG=info,terra_world=debug` aktiviert die Diagnose explizit.

Inventar und Material-HUD zeigen echte Textur-/Mesh-Ansichten. Ein isolierter
Offscreen-Kameralauf rendert dieselben Blockvorlagen, inklusive Glas, Laub und
Wasser, einmal in 128er-Images. GPU-Ruecklesung verhindert leere Vorschaubilder;
anschliessend werden Kamera, Licht und Szenen wieder entfernt. TiltUI erzeugt
weiterhin alle Elemente: Ein kleiner Image-Adapter bindet die geteilten Bevy-
Image-Handles an TiltUIs generierte img-Nodes. Keine Symbole als Blockersatz,
kein dauerhaft laufender Inventar-Renderpass.

Ein erfolgreicher Survival-Abbau erzeugt einen Ressourcen-Drop. Der Server sammelt
ihn innerhalb von 2 m mit freier Sichtlinie ein, sofern im UUID-gebundenen Inventar
Platz ist. Ein Inventar hat 27 stabile Stapelplaetze mit standardmaessig maximal
128 Einheiten pro Stapel. Leere Slots werden nicht mehr zusammengezogen.
Survival-Platzierung verbraucht genau ein passendes Blockexemplar erst nach einem
erfolgreichen Edit; Konflikte und ungueltige Positionen verbrauchen nichts.
Creative-Abbau erzeugt keine Ressourcen und Creative-Platzierung verbraucht keine.
Es existiert weder ein zweites Inventar-Save noch ein zweites Edit-Journal:
`GameplayState` mit Inventaren, Drop-IDs und Ressourcenpositionen liegt im
bestehenden Journal und wird gemeinsam gespeichert. Alte Saves ohne das neue
`gameplay`-Feld laden kompatibel mit leerem Gameplay-Zustand (`serde(default)`).

Es sind maximal 2048 lose Ressourcen in einer Welt erlaubt. Ist das Limit
erreicht, verweigert Survival weiteren ressourcenerzeugenden Abbau, statt Drops
still zu verlieren. Jeder Client erhaelt hoechstens 256 nahe Drops innerhalb
von 48 m. Echte kleine 3D-Block-/Propmodelle verwenden geteilte Mesh- und
Material-Handles. Rotation, Winkelgeschwindigkeit und Bodenkontakt werden
serverseitig simuliert und gespeichert: geworfene Items taumeln, prallen leicht
ab und bleiben auf einer Flaeche liegen, ohne kosmetisches Dauerschweben.
Begrenzte Substeps und rotationsabhaengige AABB-Terrainkollisionen halten den
Headless-Pfad klein; es ist keine beliebige Rigidbody-Stapelsimulation.
Entfernter Untergrund reaktiviert das Fallen. Ungeladene Bereiche pausieren
Bewegung; nach 300
Sekunden Serverzeit verfallen lose Ressourcen. Volle Inventare lassen die
Ressource bis dahin in der Welt. Es werden keine Avian-Rigidbodies pro Drop
und keine Renderabhaengigkeiten im Server benoetigt.

Das TiltUI-Inventar besitzt Creative-Katalog mit Suche und Kategorien,
18 Survival-Rucksackplaetze, neun Hotbarplaetze und einen Detailsbereich. Das
sind 27 verschiedene Speicherplaetze, keine doppelt angezeigten Hotbarslots.
Slots bleiben je Layoutstufe feste, leicht abgerundete Quadrate mit 54, 48 oder
42 Pixeln; Listen scrollen, statt die Kaesten zu strecken. Hover zeigt
Name, Namespaced-ID wie `teramic::stone` und Beschreibung rechts unten neben dem
Cursor mit zehn UI-Pixeln Abstand und Ausweichplatz an Fensterrandern. Mengen stehen
oben rechts in belegten Spieler-/Hotbar-/Cursorslots. Der unendliche Creative-
Katalog blendet Mengen komplett aus, erzeugt beim Aufnehmen aber weiterhin
echte Stapel bis 128. Block- und Propbilder entstehen aus den
echten gemeinsamen Meshes; keine Ersatzsymbole. Blockbeschreibungen stehen im
jeweiligen `assets/terra/blocks/*.json`-Feld `description`, Pflanzenbeschreibungen
in `assets/terra/props/items.json`. Der Cursorstapel gehoert zum gespeicherten
autoritativen Inventar, nicht nur zum lokalen UI-Zustand.

Der gemeinsame TMI-Itembrowser besitzt hervorgehobene Bildtabs mit
80 UI-Pixel grossen Quadraten, seit Phase 88 doppelt so gross wie zuvor.
Hover-Titel folgen wie alle Tooltips dem Cursor rechts unten; Tabklicks werfen
keinen gehaltenen Cursorstapel ab. Einzelne JSONs unter
`assets/terra/inventory/categories/` definieren ID, Name, Itemicon und Sortierung.
Die sechs Standardkategorien sind Building, Decoration, Materials, Animal Materials,
Food und Tools. Block-/Prop-JSONs ordnen Items ueber `creative_category` zu.
Namespaced Custom-Kategorien bleiben moeglich, ohne zweiten Blockkatalog.
Die Suche filtert innerhalb der ausgewaehlten Kategorie.
In Creative oeffnet **R ueber einem TMI-Item** die Rezeptvorschau, ohne Items zu
erzeugen oder das Craftingraster zu fuellen; beim Tippen in der Suche ist dieses
Kuerzel deaktiviert. Der Recipe-Plusbutton bleibt die getrennte Fuellaktion.

Linksklick nimmt, legt, vereinigt oder tauscht einen Stapel; Rechtsklick nimmt
die Haelfte oder legt eine Einheit ab. Ziehen mit gehaltenem Cursor verteilt
gleichmaessig mit Links und einzeln mit Rechts. Mit leerem Cursor begonnener
Linksklick sammelt beim Ziehen passende Stacks; Shift-Ziehen verschiebt besuchte
Stacks schnell zwischen Rucksack und Hotbar. Doppelklick sammelt gleichartige
Items, Ctrl+Mausrad transferiert einzelne Einheiten; das normale Mausrad scrollt
den Bereich unter dem Cursor. Creative-Katalogklicks geben genau
eine Einheit auf den Cursor, Shift-Klick befuellt den Bestand direkt mit bis zu 128.
Auch die Tastaturauswahl gibt ohne Shift nur eine Einheit; Middle-Pick bleibt
ein getrenntes Creative-Werkzeug. Ein Klick
ausserhalb des Inventarpanels wirft den Cursorstapel ab, rechts nur eine Einheit.
Beim Schliessen wird der Cursor nach Moeglichkeit zurueckgelegt; volle Slots
vernichten ihn nicht. Eine begrenzte FIFO sendet erst nach einer expliziten
Serverantwort weiter. Revisionen und gespeicherte Cursor-Intention verhindern
doppelte Anwendung oder versehentliches Aufnehmen nach Leeren des Cursors.
Platzierung verbraucht den ausgewaehlten Stack, nicht einen beliebigen gleichen.

Der Detailsbereich zeigt die Namespaced-ID ohne Zeilenumbruch. Passt die vom
Textsystem gemessene vollstaendige Kennung nicht, entfaellt nur der Namespace;
der Registryname behaelt seine Unterstriche. Hover zeigt `von teramic` oder den
tatsaechlichen Namespace eines fremden Inhalts. Kennungen mit vorhandenem
Namespace bekommen kein zweites `teramic::` vorangestellt.

### Monitor, Nebel Und Streaming-Uebergaenge

Die Options-Seite nutzt physische Monitorabmessungen und verfuegbare Videomodi,
nicht die Groesse des aktuellen Fensters. Native 3840x2160-, Ultrawide- und
weitere passende Fensteraufloesungen werden angeboten; Duplikate entfallen.
Borderless verwendet weiterhin die Desktopaufloesung. Exklusives Fullscreen
verlangt einen real gemeldeten Videomodus; der 15-Sekunden-Rollback bleibt erhalten.
TiltUI 0.1 benoetigt fuer dynamische Optionen einen echten Gruppenknoten im
ChoiceBox-Popup und eine Synchronisation gebundener Optionsbeschriftungen.

`configs/graphics.toml` besitzt `fog_distance` (0 aus, sonst bis 1.5 mal bisherige
Nebeldistanz; Default 1.0) und `chunk_animation`
(`none`, `fade`, `fade_slow`, `bounce`, `rise`, `slide`).
Entsprechende Client-Defaults stehen auch in `client.toml` zur Verfuegung.
Unterwasserabsorption bleibt unabhaengig von ausgeschaltetem Oberflaechennebel.
`none` erhaelt das bisherige sofortige Ein-/Ausblenden. Optionale Fades dauern
weiterhin 300 beziehungsweise 750 ms; Bounce eine Sekunde, Rise/Glide 900 ms. Sie wirken
auf neu geladene/entladene Terrain-, Pflanzen- und Wasserbatches. Neue Wurzeln
werden unsichtbar eingereiht. Sichtbare Wurzeln starten ihre Animation mit
tatsaechlichen Mesh-Grenzen im Kamerafrustum. Fertige Wurzeln ausserhalb des
Blickfelds werden unabhaengig davon im Hintergrund freigegeben: hoechstens 64
je 100-ms-Queue-Aktualisierung, ohne Fade-Materialien oder Animationsplaetze.
Fehlende Mesh-Grenzen oder noch nicht propagierte Welttransformationen bleiben
in der Vorbereitung. Beim Umdrehen wartet so kein bereits fertig vorbereiteter
Bereich erst auf seine Animation. Jeder sichtbare Materialbatch startet seine Fade-Uhr erst mit
seinem eigenen erfolgreichen Farb-Draw. Bei frisch generierten Chunks kann
deshalb kein schneller Batch den Fade noch hochladender Geschwister verbrauchen:
fertige Teile blenden bereits ein, wartende bleiben bei Alpha null. Materialteile
ausserhalb des Bildes halten abgeschlossene Animationsplaetze nicht besetzt.
Leere Prop-/Wassercaches zaehlen noch nicht als erster sichtbarer Upload.
Unveraenderte Alphawerte erzeugen keine erneuten Materialuploads. Unter 30 FPS wird die
Animationsuhr begrenzt, damit ein langer Frame nicht den gesamten Fade verbraucht.
Reine LOD-Ersetzungen warten einen laufenden Eingang ab; echte Edits bleiben atomar
und unmittelbar. Bounce/Rise/Glide bewegen nur die Darstellung, keine Kollisionszellen.
Materialvarianten existieren nur waehrend des Fades; danach werden die geteilten
Originalhandles wiederverwendet. Transparente Zwischenstaende werfen keine
falschen Vollschatten. Hoestens 96 Batchwurzeln animieren gleichzeitig, maximal
acht beginnen pro Frame. Weitere Wurzeln bleiben verborgen in der priorisierten
Warteschlange, statt ploetzlich voll sichtbar zu werden. Nur animierende Meshes
setzen automatisches Batching fuer zuordenbare Draw-Rueckmeldung kurz aus;
danach gilt wieder das urspruengliche Batching. Meshes eines
Fade-outs werden erst nach dessen Abschluss entfernt. Dies ist kein neuer
Dither- oder Tiefenpass und keine Entity pro Voxel/Pflanze.
Noch wartende schattenfaehige Batches bleiben auf dem
reservierten Sonnen-Schattenlayer 62 sichtbar, nicht auf dem Kameralayer.
So fehlen keine dauerhaften Schatten hinter der Kamera. Es entstehen dafuer
keine doppelten Mesh-Entities; urspruengliche Layer/Sichtbarkeit werden beim
Animationsstart, der Hintergrundfreigabe oder Abschalten wiederhergestellt.

Eine kamerazentrierte, tiefengepruefte Haze-Huelle verbindet den endlichen
Streamingrand mit dem atmosphaerischen Himmel. Ihre lineare Farbe stammt direkt
vom aktuellen DistanceFog, einschliesslich Unterwasserwechsel; ein weicher
Winkelverlauf ersetzt den harten planetaren Bodenstreifen. Terrain und Wasser
liegen davor. Keine zusaetzlichen Weltzellen, kein Schatten und kein Tiefenschreiben.
Die kamerazentrierte Huelle wird explizit vor der anderen transparenten Geometrie
gezeichnet; ihr Mittelpunkt darf sie nicht nach vorne sortieren und einblendende
Chunks verdecken. Der reine Sortier-Bias veraendert weder Rastertiefe noch
Tiefentest und fuegt keinen Renderdurchlauf hinzu.
Der Oberflaechennebel endet auch bei 150 Prozent vor dem endlichen Streamingrand;
bei Nebel aus bleibt auch diese Huelle aus.

Grasboden wird beim Ueberbauen mit einem festen Block autoritativ zu Dirt.
Beide Edits laufen durch das vorhandene Journal, einschliesslich negativer
Section-Grenzen, Replikation und Regionsspeicherung. Wiederabbau begruent den
Boden nicht sofort neu. Wasser zeigt keine Auswahlumrandung; das gezielte
Ersetzen von Quellen und die bestehenden Kollisionsregeln bleiben erhalten.

Das persoenliche 2x2-Crafting verweist auf echte Inventarslots: ein Holzblock
ergibt vier zugeordnete Planks, vier Sand ergeben einen Sandstein. Verbrauch
und Einfuegen sind atomar; bei Platzmangel bleibt das Inventar unveraendert.
Creative-Befuellung ist serverseitig nur im Creative-Modus erlaubt.
Platzierte Pflanzen liegen in `SectionDelta.placed_props` des bestehenden
Journals, werden repliziert und bleiben nach Reload erhalten. Alte Saves
erhalten fehlende neue Felder ueber Serde-Defaults; keine zweite Speicherung.

Der Blatt-Waldlaeufer nutzt sechs gemeinsame, vertexgefaerbte Koerperteilmeshes
mit Kapuze, Gesicht/Haar, Blattkleidung, Lederriemen und Schnalle. Laufbewegung
animiert diese Teile, nicht einzelne Detailvoxel. Kleinere Augen und gemeinsame
Lidmeshes blinzeln in variierenden Abstaenden. Die Inventarvorschau rendert
dieselben Assets einmal offen und geschlossen auf einer getrennten RenderLayer
mit GPU-Readback. Danach werden Vorschaukamera und Szene entfernt; das Portrait
wechselt nur die beiden geteilten Images, statt staendig neu zu rendern.
Ein Ausruestungssystem und nicht vorhandene Items werden nicht vorgetaeuscht.
Das Survival-HUD fuer Hunger/Gesundheit/Ausdauer und Atemluft ist seit Phase 43
unabhaengig vom Inventar und Debug-Overlay unten links vorhanden.
Der Hals ist um 65 Prozent auf 0.049 m verkuerzt, mit entsprechend tieferem Kopf;
der logische Physikkoerper wird dadurch nicht veraendert.
Nur das gezielt abgebaute Objekt liefert Ressourcen. Pflanzen, die beim Entfernen
ihres Bodens automatisch ungueltig werden, werden weiterhin ohne zusaetzliche
Ernte entfernt. Andere Spieler sehen bestaetigte Weltveraenderungen und Drops;
Bruchfortschritt und Splitter sind derzeit lokale Darstellungen.

## Weltgenerierung und Referenzstil

Generatorversion **5** verwendet gebogene,
verjuengte Staemme, Wurzelansaetze, verzweigte Aeste und mehrere versetzte
ellipsoidale Kronengruppen. Gruene, herbstliche und hellgruene Birkenvarianten haben
unterschiedliche Hoehen (etwa 9 bis 22 Meter Stammhoehe), Farben und Kronenformen.
Breite, aufrechte und gestaffelte Formen variieren Aeste, Kronen und Wuchsrichtungen.
Alle Teile bleiben normale, einzeln abbaubare Voxelzellen, kein starres Gesamt-GLB.
Die aktualisierte world.toml verwendet Version 5; Versionen 1 bis 4 werden
explizit abgelehnt. Es gibt noch keine Migration alter Generatorwelten.
Huegel, breite gewundene Taeler, groessere Erhebungen und unregelmaessige Terrassen
stammen aus der deterministischen Dichtefunktion. Gras belegt exakt die oberste
Voxelzelle; darunter liegen mindestens drei Dirt-Zellen, anschliessend Stein.
Erst lokales Relief von mindestens drei Metern duennt die Bodendecke aus.
Zusammenhaengende tiefe Erddecken erhalten auch vollstaendig gruene Berghaenge.
Oberflaechenspalten und Baumkandidaten werden pro X/Z-Spalte zwischengespeichert.

Ocean, Plains, Forest, Desert, Highland und Mushroom Park sind **weich gemischte Terrainprofile**,
keine diskreten Biome-Rechtecke. Kontinente, Temperatur, Feuchtigkeit und
Hochlandanteil stammen aus globalen Noise-Feldern. Normierte Gewichte blenden
Hoehenprofile, Bodentiefe, Terrassenbreite,
Waldanteil und Baumartenwahrscheinlichkeiten. Es gibt keine Auswahl eines dominanten
Biome-Generators und keine Abhaengigkeit von Chunk-Grenzen. Sand-/Schneebaender
haben ebenfalls gemischte Randbereiche; einzelne Voxel behalten ganzzahlige Material-IDs.
Ozeanprofile senken den Boden grossflaechig unter den Meeresspiegel. Wuesten haben
weichere Duenen, tiefere Sandbaender und wenig Bewuchs. Plains und Forest
unterscheiden sich in Wald-/Bodenbewuchs, ohne harte Hoehenspruenge am Biomwechsel.
Das ist eine erste Biome-Sammlung, noch keine vollstaendige Oekologiesimulation.

Highland ist hoehengebunden: Unter Y80 ist sein sichtbares Gewicht exakt null;
zwischen Y80 und Y108 wird es weich eingeblendet. Das zugrunde liegende Klimafeld
hebt tatsaechliche zusammenhaengende Bergmassive an, statt niedrige Huegel nur
umzubenennen. Rohklima und hoehengebundene Biomausgabe sind getrennte Schritte,
damit Hoehe und Biom sich nicht rekursiv abfragen. `snow_height` hat fuer neue
Konfigurationen den Default 112; explizite Werte bestehender Welten bleiben
gueltig. Snow (ID 5) verwendet Schneematerial auf allen sechs Cube-Flaechen,
ohne seitlichen Stein und ohne Snow-Cap. Kosmetischer Schnee aus dem
Oberflaechenjournal bleibt davon getrennt.

`world.toml`: `tree_density = 0.8` senkt die erwartete Baumzahl um 20% gegenueber
derselben Version-5-Welt mit `1.0`. Beibehaltene Baeume behalten ihre Position und Form.
Neue Waldgruppen, Lichtungen und Hangregeln veraendern die lokale Verteilung zusaetzlich;
gegenueber frueheren Generatorversionen ist die Anzahl nicht ueberall exakt 20% kleiner.
`biome_scale = 2048.0` und `ocean_scale = 4096.0` steuern die groesseren Klima- und
Kontinentbereiche in Metern (keine feste Grenzbreite). `biomes = false` verwendet
ein gleichfoermiges Waldprofil fuer gezielte Debug-/Generatortests.
`terrace_height = 4.0` und `terrace_strength = 0.72` steuern die Terrain-Shelves.
Staerke 0 schaltet Terrassierung aus; Dichte 0 schaltet Baeume aus.

### Hoehlen, Oasen Und Pilzpark

Seltene Hoehleneingaenge folgen dem Hang. Ein etwa 48 m langer, unregelmaessig
gewoelbter Abstieg verbindet die Oberflaeche mit dem vorhandenen 3D-Hoehlenfeld.
Die begehbare Sohle bleibt gestuetzt; Seitengaenge und endseitige Kammern
variieren in Breite und Hoehe. Dachstreifen werden nur beibehalten, wenn ihre
Voxellagen eine flaechenverbundene Verbindung zum Terrain besitzen. Zu duenne
oder ungestuetzte Ueberhaenge werden geoeffnet. Flussgalerien und Oasen sind von
diesen trockenen Eingaengen ausgenommen. Die deterministischen Beschreibungen
liegen in einem begrenzten Cache, waehrend die Geometrie weiter im bestehenden
Section-Mesher entsteht. Nahe unterirdischen Spielern werden hoechstens 18
zusaetzliche Sections unterhalb des bisherigen Fensters priorisiert geladen;
der Hauptthread verwendet bereits vorbereitete Hoehen statt neuer Noise-Abfragen.

Das unterirdische Dichtefeld mischt schmale Gaenge, hohe Spalten und seltene
ellipsoidale Grosskammern bis etwa 86 x 70 x 52 Meter. Ein pro X/Z-Spalte
vorbereiteter Hoehlenparameter nutzt die vorhandenen zwei 3D-Noise-Samples
fuer raue Waende; kein zweiter Hoehlenmesher. Geschuetzte Dach- und Bodenbaender
halten explizite Eingaenge und Flusssohlen zusammen. Fuer die Grosskammeransicht
gibt es `TERRA_BIOME_SCENE=giant-cave --smoke-test-biomes`.

Die Wueste besitzt Sand ueber einer unregelmaessigen Sandsteinschicht und
darunter Stein. Tote, verzweigte Buesche ersetzen dort gruenen Unterwuchs.
Seltene Oasen erhalten unregelmaessige Becken mit lokalem Pegel oberhalb des
Meeres: der niedrigste umgebende Boden begrenzt den Pegel. Die gesamte
Beckenflaeche wird auf tragendes Terrain und Abstand zu Fluessen geprueft.
Oasen verwenden dieselben Wasser-Sections, Fluidquellen, Physik- und Audioabfragen
wie die bisherige Welt. Es gibt keine zweite Seen- oder Wasser-Entity-Pipeline.
Trockene Grasvarianten wachsen nur an geeigneten Oasenraendern; dort stehen
Palmen mit gebogenem Stamm und acht zusammenhaengenden, geschwungenen Wedeln.
Wuesten-Flusskandidaten werden als ganze Laeufe auf etwa ein Siebtel reduziert,
nicht mitten im Wasser an einer Biomgrenze abgeschnitten.

Mushroom Park verwendet ein zusaetzliches weiches, seedabhaengiges Klimafeld.
Es nimmt Anteile von geeignetem feuchtem Land, nicht vom Ozean oder der Wueste.
Grosse Pilze haben 6..14 m hohe Staemme und unregelmaessig gewoelbte Kappen;
sechs Farbgruppen nutzen die IDs 20..26. Kleinere Pilze bilden passenden
Unterwuchs. Alle Strukturen verwenden die bestehende Baum-Platzierung und
Section-Stempelung, keine individuellen Modelle. Waldgruppen besitzen
unterscheidbare Arten sowie Fichten mit gestaffelten Nadelkronen in kaelteren
Lagen; die bisherigen Gruen-/Herbst-/Birkenvarianten bleiben erhalten.
Fichten sind jetzt 21..33,6 Meter hoch (40 Prozent groesser), besitzen genau
einen Stammvoxel je Hoehe und eine ueberlappende Krone, die den Holzabschluss
umschliesst. Die sechs Pilzfarben haben unterschiedliche gelappte Kappenprofile,
Proportionen und seit Phase 31 ein bis drei Kappen. Seitliche Koepfe sitzen an
ausladenden, geknickten und zum Kopf ansteigenden Armen. Verbindungen und
Suchreichweiten bleiben voxelgenau geprueft; keine schwebenden Seitenkoepfe.
Stielrippen, erhabene
helle Flecken und Lamellen entstehen nur im Near-Section-Mesh, additiv auf
geschlossenen Cube-Flaechen. Pro betroffener freier Flaeche kommen hoechstens
72 Vertices hinzu (zwei feine Rippen oder zwei bis drei unterschiedlich grosse
Flecken); keine weiteren ECS-Entities oder Materialhandles. Middle/Far enthalten
diese zusaetzlichen Mikrodetails nicht. Bestehende uneditierte v5-Pilzstrukturen
erhalten beim Regenerieren die neue Form; Edits und Material-IDs bleiben erhalten.
Plains erhalten raeumlich gruppierte Blumenfarben, Waelder mehr Farn-Unterwuchs.
Tiefe Ozeanboeden besitzen zusaetzliche schwache Mulden/Rippen; Kuesten bleiben
kontinuierlich, Sand- und Lehmablagerungen nutzen die vorhandenen Regeln.

Diese Erweiterung veraendert auch uneditierte v5-Terrainformen und Baumverteilung.
Seeds, Welt-IDs, Block-IDs 0..19, Saveformat 1 und gespeicherte Edits bleiben
erhalten. Da Prop-IDs auch die Bodenhoehe enthalten, kann eine entfernte alte
Pflanze bei geaenderter Hoehe durch einen neuen Kandidaten ersetzt werden; das
alte Entfernungsjournal wird nicht geloescht. Neue Landschaften deshalb
vorzugsweise in einer neuen Welt erkunden;
es gibt keine Behauptung bitidentischer alter Terrainregeneration. Netzwerk-
protokoll 14 verhindert einen Beitritt inkompatibler Clients.

### Chat Und Biomsuche

T oeffnet standardmaessig den TiltUI-Chat, die Slash-Taste oeffnet ihn mit `/`.
Beide Belegungen sind unter Controls aenderbar. Enter sendet, Escape schliesst;
Pfeil hoch/runter waehlt aus den letzten 32 eigenen Eingaben. Tab ergaenzt die
unterstuetzten Befehle/Biomnamen, Shift+Tab wechselt rueckwaerts.
Solange das Eingabefeld offen ist, bleiben Bewegung, Abbau, Platzierung,
Inventar- und Pause-Kuerzel gesperrt. Escape und Schliessklick werden erst nach
dem Loslassen wieder ans Gameplay freigegeben. Fokus und Scrollposition werden
an den von TiltUI erzeugten Controls gesetzt, keine zweite UI-Pipeline.

Nachrichten laufen ueber den bestehenden zuverlaessigen ControlChannel des
Integrated-/Dedicated-Servers. 256 Zeichen, 64 Verlaufseintraege und Rate-Limits
begrenzen Aufwand. Nutzereingaben werden als Text gebunden, nicht als HTML.
`/help` und `/locate biome desert` sind echte Serverkommandos. Unterstuetzt:
`ocean`, `plains`, `forest`, `desert`, `highland`, `mushroom_park`.
Die asynchrone deterministische Rastersuche reicht bis 6144 Meter, mit hoechstens
vier gleichzeitigen Suchen serverweit und einer je Spieler. Sie findet ein
passendes Biom, garantiert aber nicht dessen mathematisch naechsten Punkt.

Ergebnisse zeigen Biomnamen und Koordinaten in Rosa/Lila, den Teleport-Link in
Gelb. Nur der Server stellt spielergebundene, einmalige Ziel-Tokens aus.
Teleportieren benoetigt Creative-/Umgebungsberechtigung und eine sichere trockene
Landestelle. Ozean-Ergebnisse ohne solche Stelle erhalten keinen Link.
Vor und nach dem Laden prueft der Server Terrain plus EditJournal erneut;
der Client kann keine beliebigen Zielkoordinaten einschleusen. Eine neue
Bewegungsgeneration verwirft alte Inputs, und Prediction pausiert bis zur
geladenen Zielkollision. Die Standard-HUD-Meldungen `Connected` und
`Welt wird geladen` entfallen; reale Verbindungs-/Aktionsfehler und der
eigenstaendige Ladebildschirm bleiben erhalten.

**Bewusste Grenzen:** Beliebige unterirdische Aquifere unter Oberflaechenwasser
sind noch nicht implementiert. `WaterColumn` beschreibt einen vertikalen
Wasserbereich pro X/Z; mehrere gestapelte Bereiche erfordern eine gemeinsame
Erweiterung von Fluidquellen, Meshes, Physik, Audio und Replikation. Bestehende
zusammenhaengende Flussgalerien und jetzt isolierte Oasen funktionieren damit.
Faellen ganzer Baeume und logischer Laubzerfall sind getrennt entworfen, aber
noch nicht aktiv: natuerliche Baumanker sollen stabile IDs aus Seed/Position
erhalten; der Server prueft Stammzugehoerigkeit und schreibt begrenzte
Abbaubatches in das vorhandene Journal. Eine separate budgetierte
Nachbarschaftssuche soll verbleibende natuerliche Blaetter pruefen, ohne
Spielerbauten oder kosmetischen Blattfall umzudeuten. Drops und Aktivierung
bleiben Gameplay-Entscheidungen, nicht automatische Nebenwirkungen dieses Patches.

Referenzregionen mit Seed 42042: Kueste bei X/Z=1152/-128, Wald bei -512/768,
Plains bei -256/1536, Wuestenkern bei 2304/-8192 und Ozean bei 1408/0.
Reproduzierbare Kennwerte ohne
Bevy/GPU: `cargo run --locked -p terra-world --no-default-features --example terrain_regions`.

Material-IDs: Air=0, Grass=1, Dirt=2, Stone=3, Sand=4, Snow=5, Wood=6, Leaves=7,
AutumnLeaves=8, SilverLeaves=9, WeatheredWood=10. Alte IDs bleiben unveraendert.
Die historischen Namen SilverLeaves/WeatheredWood bezeichnen jetzt hellgruenes
Birkenlaub und warme Holzrinde, keine Steinbaeume.
Baeume und Terrain werden aus globalen Koordinaten berechnet und bleiben ueber
Section-Grenzen und bei anderer Generierungsreihenfolge konsistent.
Baumkandidaten werden in globalen 16er-Regionen gehasht; nur ausreichend flache
Standorte oberhalb der Sandzone und unterhalb der Schneegrenze werden verwendet.
Wurzelendpunkte folgen dem lokalen Boden. Eine konservative Suchreichweite von
16 Zellen findet auch Kronen mit Ankern in entfernten Nachbar-Sections.
Holzsegmente und Kronengruppen werden nur in ihren Schnittbereichen rasterisiert.
Terrain hat Vorrang, danach alle Holzsegmente, danach Laub und zuletzt Edits.
Dadurch koennen Kronen anderer Baeume weder Aeste noch gespeicherte Edits verdecken.
Erst ganzzahlige Weltkoordinaten subtrahieren, dann lokale f32-Geometrie auswerten:
die Baumformen behalten ihre Praezision auch in weit entfernten Regionen.

Die fruehere 32-Zellen-/33-Sample-Struktur bleibt separat unter `density/`
erhalten. `CHUNK_SIZE`, `ChunkPos` und `global_to_chunk` gehoeren ausschliesslich
zu dieser Dichte-API; die aktive Blockwelt nutzt `SECTION_SIZE`,
`SectionPos` und `global_to_section`. Sie werden nicht miteinander vermischt.
Das Dichtefeld ist nach Noise/CSG keine exakte euklidische Distanzfunktion.

Die bemalten Einzeltexturen und ihre Herkunft bleiben erhalten.
`canopy.png` zeigt botanisches Laub; Rinde verwendet `wood.png`, keine Steintextur.
Die Herkunft steht in `assets/terra/blocks/textures/prompt.txt` und
`canopy-prompt.txt`. Die logischen gruene/herbstliche/Birken-Kronen,
Astverlaeufe und Voxelzellen der Version 5 bleiben unveraendert. Sichtbares Laub
erhaelt im Near-LOD nichtuniforme 6x6x6-Micro-Geometrie, im Middle-LOD eine
4x4x4-Vereinfachung. Versetzte, verschieden grosse Laubgruppen formen die
Oberflaeche innerhalb der vorhandenen Zelle; ein 0.045-m-Rahmen sichert Anschluesse.
Gemeinsam gecachte Vorlagen beruecksichtigen Nachbarbelegung, entfernen verdeckte
Micro-Faces mit dem vorhandenen Greedy-Mesher und variieren Vertexfarben
flaechengewichtet. Die Details werden in Section-Materialbatches komponiert,
ohne Micro-Entities oder abbaubare Einzelblaetter. Das verfeinert die Darstellung,
ist aber kein Umbau der logischen Baum-Makrostruktur oder ihrer Astverteilung.
Seed, Welt-ID und Generatorversion 5 bleiben erhalten.

Phase 20 ergaenzt eine gemeinsame Alpha-Maske der vorhandenen Laubtextur fuer
kleine echte Sichtoeffnungen, ohne die Baumzellen oder Picking-Grenzen zu aendern.
Nur die drei Blattmaterialien verwenden `AlphaMode::Mask`, kein transparentes
Sortierverfahren fuer das gesamte Terrain. Ein gemeinsamer Vertex-Windshader
bewegt diese Batches auch im Tiefen-/Schattenpfad; erweiterte Bounds halten die
kleine Auslenkung im Culling. Bis zu 48 fallende Blaetter in 18 m Umgebung werden
in einem einzigen wiederverwendeten Mesh bei 30 Hz aktualisiert. Sie verschwinden
an festem Gelaende, ungeladenen Bereichen oder nach zwoelf Sekunden und sind rein
visuelle Partikel, keine Ressourcen-Drops oder autoritativer Baumverschleiss.
Die geschlossenen Kronen verwenden Backface-Culling, die flachen fallenden
Blaetter bleiben doppelseitig. Sichtbare Flaechen hinter Laub werden nicht mehr
weggecullt; ein Kontaktflag trennt diese Sichtbarkeit vom freien Nachbarraum fuer
geschlossene Bevel-/Micro-Anschluesse. Der Face-Datensatz bleibt 48 Byte gross.
Native Vulkan-Aufnahmen und alle bisherigen Seam-Tests sind geprueft. Im festen
Waldbenchmark kosten Masken/Wind/korrekte verdeckte Flaechen dennoch etwa 8%
mittlere FPS; Messdaten und Grenzen stehen im Phase-20-Bericht.

Die Generation ist innerhalb +/-2^40 globaler Sample-Koordinaten begrenzt, um
f64-Praezision zu erhalten. Sehr grosse Renderkoordinaten brauchen spaeter
Origin-Rebasing fuer Bevys f32-Transforms. Plattformuebergreifende Bitidentitaet
ist noch nicht zugesichert.

## Props und Micro-Voxels

### Aktueller Bewuchs und Oberflaechen

Gras besitzt breite, stumpfe, geknickte Quaderhalme statt Spitzen. Quadratische
Querschnitte und sechs kleine Blattgruppen am Boden ersetzen flache Halmbaender.
Drei
Silhouetten kombinieren kurze Polster, mittlere Bueschel und breite Faechern
mit kleinen bodennahen Blattpixeln. Einzelne Varianten tragen winzige weisse
Blueten; eine schwebende Dirtplatte wird nicht erzeugt. Near verwendet je nach
Form unterschiedliche Vertexzahlen, mit optionalen Blueten hoechstens 536. Middle/Far
bleiben bei 160/80 Vertices; dieselben Batches, Windanimation, Zustands- und
Biomfarben sowie die zentrale 1.3-fache Darstellung bleiben erhalten.
Buesche besitzen neun duenne, versetzte
Blattschichten entlang dreier feiner Zweige; sie behalten die bisherigen
312/144/96 Vertices in Near/Middle/Far, ohne neue Micro-Entities.
Blumen besitzen zwei schlanke Stiele,
versetzte Seitenblaetter und drei unterschiedliche Bluetenformen: kelchfoermig,
sechsstrahlig oder gestuft mit vier Petalen. Erhoehte Zentren, kleine Kelchblaetter
und einzelne helle Spitzen ergaenzen die Near-Details bei hoechstens 480 Vertices;
Middle/Far bleiben bei 144/48 Vertices. Rote, gelbe, helle und blaue Blueten ergaenzen die erhaltenen
rosa/violetten Farben. Alle Pflanzen bleiben einzelne logisch anwaehlbare Props,
gerendert im gemeinsamen Section-Batch und mit bestehender Biom-/Zustandstoenung.

Terrain-Edits erzeugen keine neuen Pflanzen unter entfernten Baumteilen: Bei
vorhandenen Edits wird die urspruengliche Boden-/Belegungsinformation vor deren
Anwendung festgehalten und dient als Kandidatenfilter. Durch gesetzte Bloecke
oder geaenderten Untergrund zerstoerte, geladene Pflanzen werden zudem im
bestehenden Prop-Journal entfernt. Spaeteres Abbauen und Neuladen laesst sie
nicht wieder erscheinen. Ein wiederhergestellter urspruenglicher Terrainblock
bleibt eine logische Zelle; es gibt keinen neuen Wachstumsautomaten.

Normale Cube-Bloecke behalten eine geschlossene Huelle innerhalb ihrer logischen
Zelle. Das zuvor aufgesetzte Relief wurde durch ausschliesslich nach innen
geschnittene Mikrostruktur ersetzt: feine Rasenschollen, Erd-/Steinstuecke,
flachere Sand-/Schneeflaechen und laengs gerichtete Holzrillen. Das optionale
JSON-Feld `surface` waehlt `turf`, `earth`, `rock`, `sand`, `snow` oder `wood`;
ohne Feld bleibt der bisherige Cube-Generator aktiv. Custom-Mesh-Definitionen
duerfen dieses Cube-Feld nicht setzen und behalten ihre eigene Geometrie.

Das Near-LOD verwendet vier deterministische Varianten aus gruppierten kleinen
Teilflaechen. Gleich hohe/gleich getoente Stuecke werden greedy zusammengefasst;
geschlossene senkrechte Waende verbinden ihre Tiefen. Rasenoberseiten sind
hoechstens 1.8 cm, Erde/Rasenseiten 3.5 cm, Stein 4.5 cm, Holz 2.5 cm und
Sand/Schnee 1.2 cm vertieft. Nichts ragt ueber die logische Zelle hinaus.
Auessere Face-Raender, Kantenfasen, Unterseiten und duenne Gras-Seitenkappen
bleiben unveraendert. Middle/Far behalten das bisherige Greedy-Meshing.
Templates werden auf Workern nach Face/Variante/Nachbarmaske gecacht und weiter
in vorhandenen Section-Materialbatches gerendert, ohne weitere Draw-Entities,
GLBs oder Shader-Pipeline. Texturen/Material-Handles und Biom-/Wettertoenung
bleiben geteilt; kleine Facettentoene werden als Vertexfarben multipliziert.
Auch zusammenhaengende Fugenwaende werden greedy verbunden. Die gecachten
Vorlagen verwenden gemeinsame Indizes fuer exakt gleiche Positionen, Normalen
und UVs; harte Kanten und Texturgrenzen werden dabei nicht verschmolzen.
Dies reduziert Geometrie-/Uploadkosten ohne geringere Detaildichte, Sichtweite,
LOD-Reichweite oder Schattenqualitaet. Aktuelle isolierte Messungen:
[surface-performance-2026-10-14](tools/benchmarks/results/surface-performance-2026-10-14/README.md).

Eine konkrete Lochursache war das Verwerfen eines ganzen Quads, wenn nur eines
seiner Dreiecke nach der Nachbaranpassung degeneriert war. Jetzt werden beide
Dreiecke einzeln auf Flaeche und Winding geprueft, gueltige Haelften bleiben
erhalten. Regressionstests vergleichen detaillierte und urspruengliche Raender
fuer alle 63 Masken normaler Materialien, einschliesslich streifender Randpunkte,
negativer Koordinaten und gemischter Section-/LOD-Ecken. Alle Stilvarianten
werden zudem auf Zellgrenzen, gueltige Indizes und Normalen getestet.

`--smoke-test-surface` erstellt ausschliesslich fuer diesen Prozess eine kleine
Holz-/Gras-/Stein-Testszene ueber negativen X/Z- und vertikalen Section-Grenzen,
wartet auf aktuelle Mesh-Epochen und nimmt sie vor/nach einem Blockabbau auf.
Der laufende Client muss fuer Codeaenderungen neu gestartet werden.
Aktueller [Testbericht mit Aufnahmen](tools/benchmarks/results/surface-detail-2026-10-14/README.md).

Die Wasserfarbe ist staerker tuerkis. Hoehere Absorption und ein sanfter
optischer Grundanteil verbessern die Lesbarkeit flacher Wasserflaechen;
tiefenabhaengige Transmission, sichtbarer Unterwasserboden und die vorhandenen
Qualitaets-/Reflexionsoptionen bleiben verfuegbar.

Das kantige HUD zeigt FPS, App-CPU, App-RAM, Chunk, XYZ und das tatsaechliche
`WorldTarget`, einschliesslich Pflanzen. Die zum Bauen ausgewaehlte Palette
wird dafuer nicht mehr angezeigt. CPU ist der Prozessanteil an der verfuegbaren
logischen CPU-Kapazitaet (100% = alle verfuegbaren Kerne), RAM ist RSS in MiB.
Linux verwendet `/proc/self/stat`, `/proc/self/status` und einmalig `getconf
CLK_TCK`; Abtastung 1 Hz, Textaktualisierung 4 Hz. Ohne Plattformbackend stehen
CPU/RAM auf `--`; es werden keine geschaetzten Werte als Messungen ausgegeben.

`--smoke-test-botanical` rahmt echte generierte Gras-/Blumen-/Busch-Props und Boden
nah. Der erweiterte Ablauf prueft Ziel-ID, Zielanzeige, Prozessmetriken und
Wind-Batches; fuer Gras und Busch sind jeweils zwei Aufnahmen bei feststehender
Kamera im Abstand von einer Sekunde vorgesehen. Die bestehenden Bildnamen
`grass`, `flowers`, `terrain` und `flowers-detail` bleiben erhalten; neu sind
`grass-wind`, `bush` und `bush-wind`. Fruehere Artefakte und Messungen:
[voxel-detail-2026-10-14](tools/benchmarks/results/voxel-detail-2026-10-14/README.md).

### Generierung und Batching

Der vorhandene Bewuchs in `generation/vegetation/ground.rs` verwendet weiter
biomeabhaengige globale Masken. Zusaetzliche kontinuierliche Masken gewichten
Lichtungen, Blumenwiesen, Buschgruppen und Grasbueschel getrennt. Die Erweiterung
erzeugt daraus logische `WorldProp`-Datensaetze waehrend der
asynchronen Section-Generierung, auf Client und Headless-Server identisch.
Es existiert kein zweiter konkurrierender Pflanzen-Generator. Die bisherigen
Cube-GLB-Dekorationen werden nicht zusaetzlich gerendert.

`WorldProp` enthaelt stabile `PropId(u64)`, `PropKind`, Weltposition, Y-Rotation,
Skalierung, Variante, Eigentuemer-Section und `removed`. Der Hash verwendet den
vollen Seed, globale Koordinaten des tragenden Blocks und einen festen Salt;
Ladereihenfolge und LOD aendern die ID nicht. Pro Bodenposition entsteht hoechstens
ein Prop. Freier Raum und der bearbeitete Untergrund werden geprueft, sodass
Baeume/gesetzte Bloecke Kandidaten blockieren und fehlender Boden keinen Bewuchs
traegt. Der Positionsversatz nutzt den freien Platz innerhalb der traegenden
Zelle und beruecksichtigt die groessenabhaengige Prop-Begrenzung. Dadurch bleibt
mehr Variation moeglich, ohne benachbarte Props ineinanderzuschieben.
Die Verteilung bleibt an der generierten Oberflaeche orientiert; neue
Spielerbauten erzeugen keine beliebigen neuen Pflanzenebenen.

Arten sind Gras, Blumen, Buesche, Farne, Pilze und `SmallPlant` fuer Schilf bzw.
kleine Steinvarianten. Waldmasken gewichten Farne/Pilze, Plains Blumen; trockene
Ufer erhalten Schilf, Sand-/Steinflaechen wenige Steine auch im flachen Wasser.
Die bereits vorbereiteten Baumkandidaten gewichten Unterwuchs unter Kronen:
mehr Farne, weniger Blumen und grosse Buesche nahe am Stamm. Das ist eine
deterministische Naeherung der Beschattung, keine Lichtsimulation.
Hoehen, Rotation und Farben variieren deterministisch. Die visuellen Vorlagen
verwenden schlanke Halme, Stiele mit mehreren Bluetenformen, gefiederte Farne,
unregelmaessige Buschgruppen und gestufte Pilzkappen. Kleinste Elemente messen
nur wenige Zentimeter; sie sind keine eigenen logischen Voxelzellen.

`terra-render/src/props/` cached Geometrie pro Art, Variante und LOD und
komponiert auf Workern genau einen Meshbatch je belegter Prop-Section.
Die Palette wird einmalig in lineare Vertexfarben gebacken; alle Prop-Batches
teilen ein weisses `ExtendedMaterial<StandardMaterial, PropExtension>` mit dem
Vertexshader `terra_props.wgsl`. Das zuvor ungenutzte UV0 traegt zwei Werte:
quadrierte wurzelrelative Hoehe und skalierte Biegsamkeit. Daher bleibt die
Vertexgroesse unveraendert; Wind benoetigt weder CPU-Meshupdates noch eine
Entity pro Pflanze. Wurzeln bleiben fest, waehrend Gras, Buesche, Blumen, Farne
und Schilf sanft seitlich schwingen. Pilze und kleine Steine bleiben statisch.
Near verwendet volle, Middle halbe Windstaerke; Far behaelt die Pflanzengeometrie,
ueberspringt aber die kaum sichtbare Windberechnung. Forward-, Schatten-,
Prepass- und Bewegungsvektoren verwenden dieselbe Verformung, mit konservativ
erweiterten Batch-AABBs. Die vorhandenen logischen Picking-Grenzen bleiben erhalten.
Die drei Laubfarben der Prop-Palette und
die Stielfarbe wurden in linearem Farbraum um 20 bis 25 Prozent abgedunkelt,
damit beleuchtete Pflanzen weniger ausbleichen; Bluetenfarben bleiben getrennt.
Bis zu vier zusaetzliche Jobs und vier
Uploads pro Frame werden jeweils durch das Minimum aus 4 und dem konfigurierten
Mesh-/Uploadbudget begrenzt. Im stabilen Zustand werden keine Sections sortiert;
nur veraltete Batches kommen in die Distanzpriorisierung.
Ein Prop ist ein Datensatz, kein eigener Draw Call; auch Micro-Quader besitzen
keine ECS-Entity. Die Vorlage wird gemeinsam verwendet, die transformierten
Vertices liegen jedoch in Section-Meshes: Das ist CPU-Batching, kein GPU-Instancing.

`WorldTarget` vereint Block- und Prop-Treffer. Der Block-Ray verhindert Auswahl
durch Terrain. Prop-Begrenzungsboxen dienen nur als schnelle Vorauswahl in
Sections entlang des kurzen Strahls. Die eigentlichen Treffer verwenden exakt
dieselben Dreiecke wie die Near-Vorlagen des Renderers, einschliesslich Rotation,
visueller Skalierung und der aktuellen Windverformung. Auch der Auswahlumriss
folgt den einzelnen Pflanzenflaechen statt einer Box. Die autoritative Pruefung
nutzt nur den engen moeglichen Windbereich jedes Dreiecks, damit unterschiedliche
Client-/Server-Uhren keine falschen Abweisungen erzeugen. Normale Terrainbloecke
behalten ihre vollstaendige logische Meterzelle fuer Picking und Kollision;
kosmetische Bevels/Reliefs aendern nicht das Bau-/Kollisionsraster.
Creative sendet direkt einen
Entfernungswunsch, Survival verwendet die gemeinsame serverseitige Haltezeit;
ein bestaetigter Edit muendet in die vorhandene `PropRemoveRequest`-Commit-Pipeline.
Das bestehende `EditJournal` speichert den entfernten Prop
und erhoeht eine separate `prop_epoch`; nur dessen Section-Prop-Batch wird
erneuert, ohne Terrain- oder Wasserneugenerierung. Alte Worker-Ergebnisse werden
an Terrain- und Prop-Epochen geprueft. Reload wendet dieselben Tombstones erneut
an. Das gemeinsame Save-System erhaelt diese Entfernungen ueber Programmneustarts.
Survival erzeugt dabei eine Ressource des jeweiligen Prop-Typs, Creative nicht.
Ein Abbau trifft den ganzen Prop, nicht einen einzelnen dekorativen Halm.

## Wasser und Ufer

Wasser ist **kein Terrainblock und keine Material-ID**. Sein Zellfeld bleibt
von soliden Bloecken getrennt. `world.water = true` aktiviert
separate zusammenhaengende Oberflaechen bei `sea_level` in tiefliegenden Regionen.
Die Gelaende-/Biomgenerierung verwendet Version 5; der separate Wasservertrag ist
`WATER_SURFACE_VERSION = 1`. Engine-/Dependency-Versionen bleiben unveraendert;
das vorhandene Bevy-Audio-Feature ist ausschliesslich im Client aktiviert.

Ohne Fluesse erzeugen Worker fuer die Section des Wasserpegels ein flaches 17x17-Gitter,
mit Dreiecken ausschliesslich ueber geeigneten leeren Zellen. Nachbar-Padding
und Edit-Snapshots halten Hoehe, Tiefen- und Uferwerte an negativen und positiven
Chunk-Grenzen identisch. Ein gemeinsames Material zeichnet alle Wasser-Tiles;
es gibt keine Wasser-Entity pro Voxel. Uploads sind auf vier Wasser-Tiles pro
Frame begrenzt, alte Epochen werden verworfen, entladene Meshes entfernt.
Flusswelten verwenden denselben Uploadpfad mit gemeinsam berechneten Eckhoehen,
Ufertiefen und Section-geschnittenen Wasserfaellen. Kleine Gefaelle bleiben glatt,
Faelle ab 5 m erhalten ein leicht unregelmaessiges, gekruemmtes Profil in einem
weltweit ausgerichteten 1-m-Raster. Vertex-Metadaten steuern Kamm-/Aufprallschaum
auf bestehenden Flaechen, ohne zusaetzliche Overlay-Entities.

`assets/shaders/terra_water.wgsl` nutzt Bevys echte `ExtendedMaterial`-API,
DepthPrepass und einen gemeinsamen Screen-Space-Transmission-Schritt. Animierte
Normalwellen, farbabhaengige Beer-Lambert-Tiefenabsorption, Grundlichtmuster und
Uferschaum werden mit der bereits vorhandenen Transmission kombiniert.
Die Reflexion wurde zuvor nochmals mit Fresnel gewichtet, obwohl Bevy PBR diese
Gewichtung schon enthaelt. Jetzt wird die Transmission einmal abgeschwaecht und
die native PBR-Reflexion mit tiefenabhaengiger Farbstreuung addiert.
Flache Bereiche bleiben durchsichtiger; Schaum ist auf flaches Wasser begrenzt,
Wellen werden am Ufer ruhiger, Brechung beruecksichtigt das Seitenverhaeltnis und
weist unpassende Vordergrund-/Himmelstiefe zurueck. Die geometrische Wasserhoehe
bleibt ruhig und nahtlos.

Medium und High ergaenzen echte Screen-Space-Reflexionen sichtbarer Szenengeometrie.
Der Wasser-Shader verwendet Bevys vorhandenen `bevy_pbr::raymarch` mit DepthPrepass
und der bestehenden Transmission-Farbtextur. Ein Treffer ersetzt nur den
entsprechenden Umgebungs-Specularanteil; Sonne, Fresnel und Transmission bleiben
erhalten. Unsichere Treffer und Bildraender blenden zur Atmosphaerenreflexion aus.
Bevys Kamera-SSR benoetigt Deferred Rendering und liegt vor dem transmissiven
Wasserpass; deshalb wird dessen Raymarch-Bibliothek direkt im Wasserfragment
verwendet, ohne neue Renderpipeline oder zusaetzlichen Reflexionspass.
Nicht sichtbare oder verdeckte Objekte fehlen in SSR. Planarreflexionen und
temporale Reflexionsfilterung sind nicht implementiert.

`water_quality = "low" | "medium" | "high"` waehlt eine, zwei oder drei
Normalwellen. Low deaktiviert Brechungsversatz und Szenenreflexionen; Medium/High
verwenden 16/28 Raymarch-Schritte und 3/5 Verfeinerungen bis maximal 80 m.
Nur High berechnet die subtilen Grundlichtmuster. `water_wave_strength = 0.65`
ist zwischen 0 und 2 einstellbar. Unter Wasser wird SSR nicht ausgefuehrt.
`water_reflections = false` deaktiviert SSR auch bei Medium/High, ohne
Wellen, Brechung oder Transparenz umzustellen. So ist ein Reflexionsvergleich
mit identischer Wasserqualitaet moeglich; die Atmosphaerenreflexion bleibt.
Konfigurationsaenderungen aktualisieren denselben Material-Handle. Alle Stufen
behalten Transparenz, Tiefenwirkung und die vorhandenen Renderpaesse; kein neuer
Reflexionspass wird eingefuehrt. Die Kamera rendert HDR; Sonnenlicht ist leicht
warm, kuehles Ambient-Licht und Atmosphaerenlicht halten Schatten lesbar.
Das atmosphaerische Bodenalbedo ist auf blauen Fernnebel abgestimmt, weil der
virtuelle Planetenboden ausserhalb des geladenen Terrains sichtbar wird.
Der Nebel erreicht vor der Chunk-Sichtgrenze volle Deckung. Drei ueberlappende
Schattenkaskaden verwenden Gaussian-Filterung. ACES-Tonemapping bleibt bestehen;
es wurde kein Bloom hinzugefuegt.
Die Biompalette der neutralen Grastextur ist kraeftiger abgestimmt, ohne die
Wetter-/Nass-/Brandfarben zu ersetzen. Ein gemeinsames Oberflaechen-Grading
steuert waermere Mitteltoene und wird beim Auftauchen korrekt wiederhergestellt.

Sand-/Steinufer verwenden die vorhandenen sanften Materialbaender. Schilf ist
jetzt ein individuell entfernbarer `SmallPlant`-Prop am trockenen Ufer bis 2.5 m
ueber dem Pegel; die alten `ShoreReeds`-Details der Standard-GLBs entfallen
zusammen mit diesen unbenutzten Modell-Assets.
Gras und Blumen unter dem Pegel werden unterdrueckt. Implizites Naturwasser
ist kein normaler abbaubarer Terrainblock; Fluid Water (27) ist dagegen eine
platzierbare und wieder entfernbare Quelle im gemeinsamen Zellwassersystem;
gesetzte solide Uferzellen schneiden die Wasseroberflaeche aus und werden nach
Abbau wieder freigegeben. Das Edit-Journal erhaelt diese Anpassungen beim Reload.

Der vorgegebene Link [mira-echoes-of-ash](https://github.com/mira-organization/mira-echoes-of-ash)
enthielt im geprueften `develop`-Commit `42ae234` und Refactor-Branch keinen
Wasser-Shader. Das lokal vorhandene `mira-born-of-flux/assets/shaders/water.wgsl`
diente als konzeptionelles Vorbild fuer Tiefe, Brechung, Schaum und Fresnel.
Terra verwendet eine eigene WGSL-/Streaming-Integration; keine fremde Shader-
oder Rendering-Crate wurde eingebunden.

### Natuerliche Flussprofile

Der vorhandene gecachte `RiverPath` variiert die zentrale Bettiefe zwischen etwa
1.15 und 5 m; flachere Seitenarme teilen denselben abfallenden Pegel. Gelegentliche
deterministische Arme zweigen mit stetiger Richtung ab und vereinigen sich wieder,
sodass zwischen ihnen echte trockene Inseln bestehen koennen. Zusaetzliche
Spill-Samples beruecksichtigen ihre volle Breite. Kein zweites Flusssystem und
keine konkurrierende Wasserhoehe werden eingefuehrt.

Die aeussere Terrainhoehe schliesst sich ueber einen variierenden Bereich der
Ueberdeckung statt an einer planen Schnittwand. Asymmetrische Erosion und eine
wechselnde gewoelbte Decke formen den Tunnelmund. Hinter grossen Faellen laeuft
die hohe Schachtdecke ueber bis zu 28 m in den niedrigeren Tunnel aus. Derselbe
vorbereitete Hoehlenboden gilt fuer Meshes, Headless-Welt und Wasserabfragen.
Eine begrenzte spaltenweise Pruefung erhaelt nur Dachspannen mit voxelgenauer
Flaechenverbindung zum Ufer. Isolierte Reste der Hoehen-/Gewoelbeueberschneidung
werden vor Baum-/Prop-Platzierung abgetragen. Allgemeines Hoehlenrauschen darf
die Unterseite dieser expliziten Flussdaecher nicht erneut abtrennen; unter dem
Flussbett und ausserhalb des Korridors bleibt es erhalten. Spielerbauten und
gespeicherte Edits werden dabei nicht auf physikalische Tragfaehigkeit reduziert.
Diese neue Form gilt auch beim Regenerieren bestehender `rivers = true`-Welten,
**ohne Generatorversionswechsel**. Gespeicherte Edits bleiben unveraendert an
ihren Koordinaten, koennen deshalb aber anders zum Grundterrain liegen. Fuer
Vergleiche eine neue Flusswelt verwenden. Die damalige reine Flusskorrektur liess
`rivers = false` unveraendert; die spaeteren Hoehlen-/Hochlandprofile gelten auch dort.

### Zellbasiertes Wasser

**Phase 20:** `FluidCell` speichert
Fuellstand in Achtelschritten, Stroemungsrichtung und Quellenstatus. Unveraendertes
Naturwasser bleibt ein implizites Reservoir aus dem vorhandenen Generator;
ausgetretenes Wasser wird als Nichtquellen-Zustand im bestehenden `EditJournal` gespeichert.
`SectionDelta` transportiert die sparse Zellliste und eine eigene `fluid_epoch`.
Vorhandene Saves ohne Fluessigkeitsfelder werden mit leeren Defaults gelesen;
das aktuelle Netzwerkformat benoetigt Protokoll 26 auf Client und Server.

Abbau oder Platzierung aktiviert betroffene Zellen und ihre Nachbarn. Nur der
gemeinsame Integrated-/Dedicated-Server simuliert, mit hoechstens 256 aktiven
Zellen pro Schritt bei 10 Hz. Seit Phase 31 wird der lokale Sollpegel aus dem
Zufluss berechnet: Wasser von oben setzt den Fallstrang durchgehend fort;
seitlicher Zufluss verliert je gestuetztem Schritt eine Achtelstufe. Ein
Fallstrang verbreitert sich erst am festen Boden, nicht mitten in der Luft.
Quellen bleiben voll, Nichtquellen gehen ohne Nachschub pro Bearbeitung eine
Stufe zurueck. Damit koennen sich geschlossene Restpfuetzen nicht gegenseitig
erhalten. Das ist ein quellengespeistes Fuellstandsmodell, keine massenerhaltende
Hydrodynamik. Diagnosezaehler trennen Quelle, Nachfuellung, Rueckgang und Rueckkehr
ins Naturreservoir. Naturreservoirs selbst trocknen nicht automatisch aus.
Solide Bloecke sperren den Fluss, unbekannte Sections gelten nicht als Luft.
An ungeladenen Grenzen pausiert Arbeit und wird beim Laden wieder aufgenommen;
Zellzustaende bleiben im Journal und im vorhandenen Save-/Replikationspfad.
Clients uebernehmen bestaetigte Zustaende, keine konkurrierende lokale Simulation.
Fluid Water (ID 27) ist weder in Creative noch Survival abbaubar, auch nicht
ueber direkte Netzwerk-Bau-/Miningbefehle. Ein anderer fester Block ersetzt die
Quelle ueber die normale Platzierung; Eimer und Pumpen bleiben offen. Der feste,
animierte Solid-Water-Dekoblock (ID 14) bleibt ein normaler abbaubarer Baublock.

Wasser-only-Worker verwenden vorbereitete Spalten, aktuelle Block-Snapshots und
Fluid-Deltas. Eigene Wasser-Mesh-Epochen verwerfen veraltete Ergebnisse. Nur die
betroffenen Section-Batches werden erneuert, nicht Terrain, Props oder Collider;
gemeinsame Materialhandles und begrenzte Uploads bleiben bestehen. Schwimmen und
Unterwasserzustand fragen das zusammenhaengende lokale Wasserfeld ab.
Gemeinsame Eckpegel verbinden auch teilgefuellte Fallzellen an vertikalen und
negativen Sectiongrenzen. Nur die freie Fallfront hat eine untere Abschlussflaeche;
zwischen verbundenen Wasserzellen liegen keine kuenstlichen Zwischenboeden.

Bewusste Grenzen: natuerliche Quellen sind unbegrenzt, austretendes Wasser erzeugt
keine neuen Quellen. Keine Druckloesung, Erosion, globale Ozeanbilanz oder
Simulation ungeladener Gebiete. Wellen animieren weiterhin Normalen und Brechung,
keine oszillierenden Zellfuellstaende. Eine Blockplatzierung entfernt das Wasser
der belegten Zelle. Natuerliche Reservoire nehmen zurueckfliessendes Wasser auf;
die Volumenbilanz erfasst Quellenzufluss und Reservoirabfluss getrennt.
302 Workspace-Tests, die fuenf Cargo-Pruefungen, separater Headless-Build/-Start,
native Ansichten und zwoelf isolierte Vorher-/Nachher-Laeufe sind dokumentiert:
[Phase-20-Bericht](tools/benchmarks/results/living-water-foliage-2026-10-15/README.md).
Grosse Flutgebiete und langlaufende Mehrspieler-Simulation sind damit nicht
abgenommen; bestehende offene Langzeit-/Art-Direction-Punkte bleiben erhalten.

### Wasser-Audio

`apps/client/src/water_audio/` erzeugt drei deterministische Rausch-/Stroemungs-
stimmen fuer Ufer, Fluss und Wasserfall. Ein Bevy-AudioPlayer mischt sie raeumlich
aus Entfernung und Blickrichtung. Die Hoerposition sowie der Eintauchzustand
kommen ausschliesslich vom PlayerBody-Kopf, nicht vom Third-Person-Kameraboom.
Ein begrenzter Hintergrundjob sucht alle 0.5 s Quellen im bestehenden Wasserfeld;
entladene oder durch Bloecke verdeckte Quellen werden stummgeschaltet.
Unter Wasser sinkt der geglaettete zweipolige Tiefpass von 12 kHz auf 650 Hz,
zusaetzlich sinkt der Pegel um 35 Prozent. Beim Auftauchen wird der normale Mix
wiederhergestellt. Im Pausenmenue bleibt die Umgebung hoerbar, beim Verlassen
der Sitzung werden Stream und Suchjob entfernt. Noch nicht vorhandene weitere
SFX muessen spaeter an einen gemeinsamen Umgebungsbus angebunden werden.
Keine externen Soundassets, Audio-Entities pro Wasserzelle oder Audioabhaengigkeit
im separaten Dedicated-Server-Build. Nativer Pruefpfad:
`target/debug/terra-client --smoke-test-water-audio`.
Native Aufnahmen, 258 Tests, CPU-/GPU-Passmessungen und konkrete Grenzen:
[Wasser-/Audio-Validierung](tools/benchmarks/results/water-audio-2026-10-15/README.md).

## Server, Physik und Networking

Integrated und Dedicated verwenden `terra_server::build_server` mit denselben
Section-/Generator-/Edit-/Physik-/Netzwerk-Plugins. `simulation_distance` steuert
den horizontalen Simulationsradius pro Spieler (mindestens eine Nachbarspalte),
vertikal werden drei Sections pro Zentrum geladen. Ohne Spieler bleibt ein
Spawn-Bereich aktiv. Mehrere Spieler verwenden die Vereinigung ihrer Bereiche,
nicht einen einzigen Kamera-Fokus. Der Server erzeugt keine Render-Meshes und
registriert weder Renderer noch Fenster oder UI.

Der gemeinsame Weltspawn wird beim Serverstart nach dem Laden des EditJournals
deterministisch auf trockenem Festland gesucht. Der bisherige feste X/Z-Punkt
ueber dem Meeresspiegel ist kein Spieler-Spawn mehr. Zentrum und 3x3-Umgebung
brauchen tragenden Terrainboden, geringe Hoehenunterschiede und trockene
Standzellen; am Zentrum bleiben drei Bloecke fuer den Spieler frei. Generierte
Baeume, Hoehlen, gespeicherte Blockaenderungen und fliessendes Wasser werden
mitgeprueft. Beitritte pruefen den gemeinsamen Spawn erneut und suchen bei
abgebautem oder ueberflutetem Boden einen neuen Platz. `Welcome` uebermittelt
die autoritative Position; `Ready` und Snapshots warten auf geladenen Boden.
Die Suche verwendet hoechstens 5441 Kandidatenspalten in wachsenden Ringen.
Ohne geeigneten Platz folgt ein expliziter Fehler, kein Start im Meer und keine
kuenstliche Insel. Seed, Terrain, Generatorversion und Save-Format bleiben gleich;
feste Offline-Benchmarkkameras bleiben vom Spieler-Spawn unabhaengig.

Avian3D ist in beiden Apps mit Metern, -9.81 m/s^2 und zentralem Fixed-Takt
registriert. Nahe geladene Sections erhalten auf hoechstens zwei Workern
Greedy-Terrain-Collider; Aufbau-Radius 2 und Halte-Radius 3 begrenzen den Aufwand.
Block-Edits invalidieren die betroffenen Collider ueber vorhandene Mesh-Epochen,
veraltete Worker-Ergebnisse werden verworfen. Es gibt keine Collider-Entity pro
Voxel und keine Abhaengigkeit vom sichtbaren Terrain-LOD.

Der kinematische Kapsel-Controller verwendet Avians `MoveAndSlide` und Shape-Casts
fuer Bodenkontakt. Er wartet an noch nicht geladenen oder veralteten
Kollisionsbereichen, statt durch fehlendes Terrain zu fallen. Kamera und
Streaming-Fokus folgen dem Spielerkoerper; Blockplatzierung im eigenen Koerper
wird verhindert. Abgebauter Untergrund wird nach Collider-Aktualisierung frei.
Die logische Blockbelegung bestimmt die Kollisionsform, auch bei dekorativ
abgerundeten Cubes oder kleineren Custom-Meshes. Props und Wasser erhalten
keine neuen Kollisionskoerper.
Lightyear 0.31.0 verwendet echten UDP-Transport und verschluesselte
Netcode-Verbindungen. Vor Beginn der Client-Generierung werden Protokoll- und
Generatorversion geprueft und die vollstaendige Server-Weltkonfiguration
uebertragen. Seed, Generatorversion und Block-IDs bleiben unveraendert.
Das unveraenderte Basisterrain wird deterministisch auf beiden Seiten generiert;
nur bestaetigte Session-Aenderungen und Spielerzustaende werden uebertragen.
Es existiert kein zweites Chunk-, Prop- oder Edit-Journal-System.

Der Client schickt normierte Eingaben, niemals autoritative Positionen. Server
und lokale Prediction verwenden dieselbe `CharacterSimulation` mit Avian.
Der Server simuliert genau einen Bewegungsschritt pro Fixed-Tick; nach einer
Paketverzoegerung werden alte Eingabestaende zusammengefasst, nicht als
zusaetzliche Zeit ausgefuehrt. Die letzte Richtung gilt hoechstens 250 ms ohne
neues Paket. Sprungimpulse werden einmalig uebernommen. Eingabegeschichte und
Warteschlangen sind begrenzt. Bestaetigte Sequenzen werden verworfen und noch
offene Eingaben nach einer Serverkorrektur erneut simuliert. Entfernte Spieler
verwenden interpolierte Snapshots und geteilte Cube-/Material-Handles.
Der Snapshot-Takt betraegt beim 60-Hz-Server 20 Hz.

Lokaler Avatar und Kamera verwenden denselben `CharacterPresentation`-Pfad mit
Fixed-Tick-Interpolation und abklingender Korrektur. Der Avatar folgt nicht mehr
dem unvermittelten Physikkoerper, der bei Serverabgleich sichtbar zuruecksprang.
Grosse Teleports setzen die Darstellung direkt zurueck. `CameraLook` speichert
die unabhaengige Blickrichtung; die zweite F5-Ansicht schaut zum Gesicht zurueck,
ohne Bewegung, Zielstrahl oder Serverblick umzukehren. Eine Kugelabfrage begrenzt
den Kameraboom vor und hinter dem Spieler an Waenden. Das ist keine Abschaffung
notwendiger autoritativer Korrekturen bei hoher Latenz.

Abbau, Platzierung, Prop-Entfernung und kreative Bodenzustandsbefehle werden
zu versionierten Kommandos. Der Server prueft Reichweite, Sichtstrahl,
Material/Prop-ID, Section-Revision, Spielerueberlappung, endliche Zahlen und
Aktionsrate. Gleichzeitige Aenderungen derselben Revision werden nicht doppelt
ausgefuehrt. Survival-Loeschungen benoetigen zusaetzlich eine abgeschlossene
serverseitige Haltezeit; Platzierungen benoetigen eine passende Ressource.
Spielmodus, Fortschritt und bestaetigte Inventar-/Drop-Snapshots sind Teil des
Protokolls 18. Clients importieren nur bestaetigte, geordnete Section-Deltas;
Nachbar-Meshepochen bleiben vom Server-Revisionszaehler getrennt. Prop- und
Appearance-Aenderungen loesen keine Terrain-Neugenerierung aus. Spaeter Beitritt
und Wiederverbindung importieren den aktuellen Serverjournal-Stand. Eine
Wiederverbindung leert vorher alte Daten, Jobs und Spielerkoerper des Replikats.

`game_mode = "survival"` ist der Dedicated-Default in `server.toml`, unabhaengig
vom ebenfalls standardmaessig ausgeschalteten Flugrecht `allow_flight`.
Ohne eigene UUID-Rechte folgen Umgebungswerkzeuge weiter dem Flugrecht;
das setzt den Spielmodus nicht auf Creative. Der lokale Integrated-Host
erlaubt diese Werkzeuge in Creative, nicht in Survival. Wetterzustand und
Schneeakkumulation sind seit Phase 72 serverseitig; Wetterqualitaet bleibt lokal.

Seit Phase 15 speichern Singleplayer und Dedicated ihre vorhandenen Journale
auf Disk. Persistente UUID-Profile haben getrennte Build-/Flight-/Environment-Rechte;
verifizierte Online-Accounts fehlen weiterhin. Keine
Kollision zwischen Spielern, historische Physik-Rollbacks, Lag-Kompensation oder
PvP-Kampfsystem. Spieler-Lebenspunkte und Survival sind seit Phase 43 vorhanden;
zellbasierter Wasserausfluss ist seit Phase 20 integriert.
Bei groesserer Latenz sind sichtbare
Positionskorrekturen moeglich. Reine Render-Smokes verwenden gezielt isolierte
Offline-Fixtures; dies ist kein zweiter normaler Singleplayer-Spielmodus.
`--smoke-test-building` verwendet dagegen einen echten Integrated Server und
den UDP-/Replikationspfad fuer Abbau, Inventar und Platzierung.
`--smoke-test-camera` prueft dort Middle Pick, den dreistufigen F5-Zyklus und
identische Avatar-/Kameradarstellung; `--smoke-test-drops` prueft Q, Shift+Q,
volumetrische Dropmeshes, Wurfrotation und stabile Ruhelage.
Testprotokolle und native Aufnahmen:
[Multiplayer-Validierung](tools/benchmarks/results/multiplayer-2026-10-15/README.md).

TiltUI 0.1.0 ist ueber crates.io integriert; die eigene lokale Bibliothek wurde
ebenfalls geprueft. `build_from`, `include_components!` und `#[html_shared]`
werden tatsaechlich genutzt. Das Plugin kommt vor `DefaultPlugins`.
Es gibt keinen Ersatz durch ein anderes UI-Framework.

## Tests

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test -p terra-core -p terra-voxel -p terra-world --no-default-features
env -u DISPLAY -u WAYLAND_DISPLAY cargo run --locked -p terra-server -- --smoke-test
TERRA_SCREENSHOT=/tmp/terra-blocks.png cargo run --locked -p terra-client -- --smoke-test-mining
TERRA_SCREENSHOT=/tmp/terra-props.png cargo run --locked -p terra-client -- --smoke-test-props
TERRA_CAPTURE_DIR=/tmp/terra-building-survival cargo run --locked -p terra-client -- --smoke-test-building
TERRA_BUILD_CREATIVE=1 TERRA_CAPTURE_DIR=/tmp/terra-building-creative cargo run --locked -p terra-client -- --smoke-test-building
TERRA_CAPTURE_DIR=/tmp/terra-camera cargo run --locked -p terra-client -- --smoke-test-camera
TERRA_SCREENSHOT=/tmp/terra-drops.png cargo run --locked -p terra-client -- --smoke-test-drops
TERRA_SCREENSHOT=/tmp/terra-botanical.png cargo run --locked -p terra-client -- --smoke-test-botanical
TERRA_SCREENSHOT=/tmp/terra-forest.png cargo run --locked -p terra-client -- --smoke-test-overview
TERRA_SCREENSHOT=/tmp/terra-valley.png cargo run --locked -p terra-client -- --smoke-test-landscape
TERRA_SCREENSHOT=/tmp/terra-water.png cargo run --locked -p terra-client -- --smoke-test-water
TERRA_SCREENSHOT=/tmp/terra-forest.png cargo run --locked -p terra-client -- --smoke-test-forest
TERRA_SCREENSHOT=/tmp/terra-plains.png cargo run --locked -p terra-client -- --smoke-test-plains
TERRA_SCREENSHOT=/tmp/terra-desert.png cargo run --locked -p terra-client -- --smoke-test-desert
TERRA_SCREENSHOT=/tmp/terra-ocean.png cargo run --locked -p terra-client -- --smoke-test-ocean
TERRA_RENDER_DIAGNOSTICS=1 TERRA_SCREENSHOT=/tmp/terra-reference.png cargo run --locked -p terra-client -- --smoke-test-reference
TERRA_BENCH_SECONDS=48 TERRA_SCREENSHOT=/tmp/terra-lod.png cargo run --locked -p terra-client -- --smoke-test-lod
TERRA_SCREENSHOT=/tmp/terra-settings.png cargo run --locked -p terra-client -- --smoke-test-settings
TERRA_SCREENSHOT=/tmp/terra-physics.png cargo run --locked -p terra-client -- --smoke-test-physics
TERRA_SCREENSHOT=/tmp/terra-appearance.png cargo run --locked -p terra-client -- --smoke-test-appearance
cargo run --locked -p terra-world --features bevy --example streaming_benchmark
cargo run --locked -p terra-world --example worker_benchmark
cargo run --locked -p terra-world --example reflection_scenes
```

Tests pruefen Koordinaten inklusive -1, Indexierung, Materialgrenzen,
Face-Culling, Dreiecks-Winding, negative/weite Raycasts, Determinismus,
Baum- und Section-Generierung, Hoehlendichte, Konfiguration und Avian.
Die Streamingtests pruefen Entladen, Edit-Replay, Nachbar-Remeshing,
Job-Budget, veraltete Edits und aufeinanderfolgende Edits waehrend Worker-Jobs.
Der Headless-Test startet den echten Server und prueft `AppExit`.
Der Client-Smoke-Test kann einen anvisierten Block entfernen, auf dessen
Neumeshing warten, einen Screenshot erstellen und sich automatisch beenden.
`--smoke-test-props` waehlt einen echten generierten Prop mit der Spielkamera,
wartet auf dessen `WorldTarget`, entfernt ihn ueber die gemeinsame Edit-Pipeline
und prueft Journal, erhaltenen Nachbarn und den fertigen neuen Prop-Batch.

`--smoke-test-lod` fliegt zweimal 224 m hin und zurueck durch negative
Section-Koordinaten und beide LOD-Grenzen. Vier Zwischenaufnahmen ergaenzen den
Start-Screenshot. Der Lauf verlangt mindestens einen abgeschlossenen LOD-
Batchwechsel (`LodSwapStatus.completed`) und wartet vor seinem Ende auf
ausstehende Terrain-/Prop-Jobs. Es existieren keine zeitlichen Dither-Uebergaenge.
`TERRA_BENCH_SECONDS` waehlt 3 bis 120 Sekunden fuer feste Szenen, beim LOD-Flug
24 bis 120 Sekunden mit Standard 48. Screenshots sind ueber `TERRA_SCREENSHOT`
optional; ein Szenenflag pro Prozess vermeidet widerspruechliche Kameravorgaben.

`--smoke-test-custom` benoetigt die explizit aktivierte statische Szenen-Fixture
mit ID 65000. Sie bleibt ausserhalb der 19 normalen Blockdefinitionen.
Ein separater Asset-Ordner aktiviert sie ohne Aenderung der Standardpalette:

```bash
custom_assets="$(mktemp -d)"
cp -a assets/. "$custom_assets/"
cp "$custom_assets/terra/blocks/fixtures/pedestal.json" "$custom_assets/terra/blocks/pedestal.json"
TERRA_ASSET_DIR="$custom_assets" TERRA_SCREENSHOT=/tmp/terra-custom.png cargo run --locked -p terra-client -- --smoke-test-custom
```

Dieser Lauf setzt drei hierarchische Modelle neben einen normalen Stone-Cube
und wartet auf deren Upload mit aktuellen Edit-Epochen. Die Fixture verwendet
`model_layout = "scene"`; `fixtures/pedestal.gltf` bleibt im Unterverzeichnis.
Der Headless-Aufruf oben entfernt beide Display-Variablen und startet nur das
Server-Plugin-Set, ohne Fenster oder Rendering-Plugin.

`--smoke-test-settings` prueft Live-FOV/VSync, Sichtweitenkonfiguration,
Eingabesperre, Vorschau/Abbruch/Uebernehmen und Speichern mit einer isolierten
temporaeren Konfigurationsdatei. Eine zweite Aufnahme verwendet 640x480. Die
Automatisierung sendet TiltUI-Ereignisse an gerenderte Controls; echte
Mausklicks, Dragging und Scrollen muessen zusaetzlich nativ geprueft werden.
`--smoke-test-physics` verwendet einen lokal gesetzten Boden und eine Wand fuer
Fall-, Boden-, Sprung-, Wand-, Menue- und Untergrundabbau-Pruefungen.
`--smoke-test-appearance` prueft Wetter-Materialwechsel ohne Mesh-Neubau,
lokale Nass-/Brandfarben, unveraenderte Nachbarbatches und echtes Entladen/Reload.
Diese drei nativen Modi sowie Mining, Props, Wasser und Headless-Server wurden
am 14.10.2026 erfolgreich ausgefuehrt. Der Settings-Smoke prueft zusaetzlich die
realen NumericRange-/ControlChecked-/OptionData-Werte. Die separate echte
Pointerrunde unter XWayland blieb unvollstaendig, siehe Protokoll unten.
Die spaetere Wayland-Abnahme vom 16.10.2026 verwendet echte OS-Eingaben bei
960x640 physischen Pixeln und 150 Prozent Skalierung: Dropdown, Scrollen,
Slider-Dragging und Verwerfen bestanden. Sie ist separat im
[Inventar-/Bedienungsbericht](tools/benchmarks/results/inventory-ranger-2026-10-16/README.md)
dokumentiert und ersetzt nicht nachtraeglich den historischen Lauf.

`--smoke-test-inventory` prueft das echte TiltUI-Inventar am integrierten Server.
Creative verwendet mit `TERRA_NATIVE_OS_POINTER=1` das optionale KDE-/Wayland-
Hilfsmittel `tools/ui/wayland_pointer.py` fuer reale Mausereignisse; dafuer ist
bereits vorhandener Zugriff auf `/dev/uinput` erforderlich. Es werden keine
Systemberechtigungen automatisch geaendert. `TERRA_INVENTORY_MODE=survival`
prueft den Survival-Aufbau. Der Bau-Smoke prueft zusaetzlich echte Ernte,
Inventar und Platzierungsverbrauch. `--smoke-test-avatar` prueft geteilte
Koerperteilmeshes, Laufposen, Weltblinzeln und beide einmaligen GPU-Portraits.
Diese Renderfixture
ersetzt keinen Netzwerktest; dafuer dienen die separaten Observer-/Companion-
Smokes gegen einen echten Dedicated Server.

Die erweiterten Inventar-, Kamera-, Drop-, Abbau- und Graspruefungen sowie der
identische stationaere Vorher-/Nachhervergleich sind im
[Phase-28-Bericht](tools/benchmarks/results/interaction-camera-drops-2026-10-16/README.md)
mit Rohlogs und betrachteten nativen Aufnahmen dokumentiert.

Die Tests enthalten ausserdem die Lauf-/Flugsteuerung, deterministische
Cube-/Custom-Flaechenzusammensetzung, JSON-Discovery und -Validierung sowie die
GLTF-Fixture, Legacy-Modellvertraege und einzelne PNG-Texturen. Normale
Cube-Definitionen werden auf fehlende Modellreferenzen geprueft.
Ray-Tests auf den echten gerenderten Dreiecken pruefen geschlossene Fugen fuer
alle zehn Materialien, Treppenkanten, negative Regionen und Vier-Chunk-Ecken.
Baumtests pruefen Holzverbindungen ueber sechs Nachbarn, Varianten/Suchgrenzen,
Rasterisierung gegen Einzel-Samples, ferne/negative Koordinaten, Chunk-Reihenfolge
und Abbau mit Edit-Replay an einer negativen Section-Grenze.
Biome-Tests pruefen normierte Gewichte, volle Seeds, Profilmischung und stetige
Uebergaenge auch bei wechselndem dominantem Anteil. Landschaftstests pruefen
Terrassen, Fels-/Bodenschichten, gemischte Materialbaender, Referenzhoehen und
positive/negative Section-Raender. Dichtetests pruefen die 20%-Reduktion statistisch
und stellen sicher, dass beibehaltene Baeume nicht wandern.

Zusaetzliche Regressionstests decken explizite Grass-/Snow-Materialzuordnungen,
mehrmaterialige Custom-Modelle bei Far-LOD, Hysterese, gemischte LOD-Grenzen,
negative Koordinaten, Prop-IDs/Reload/Einzelentfernung, gueltige Micro-Geometrie
und Wasserqualitaetswechsel ohne zusaetzliche Material-Handles ab.
Wassertests pruefen weiterhin gemeinsame Ufer-Vertices, negative/fraktionale
Pegel, Block-Edits, Diagonal-Invalidierung, Eviction/Reload und Mesh-Freigabe.
Die Flug-, Biome-, Cache- und Headless-Tests bleiben Teil des Workspace.

Die historischen lokalen Bilder `screenshots/terra-p4-*.png` dokumentieren den
Stand vor dem ersten Rendering-Umbau. Auch die folgenden Ergebnisse stammen
aus dem ersten Umbau, vor dem aktuellen Referenzbild-Abgleich.
Damals am 14.10.2026 erfolgreich: `cargo fmt --all --check`, `cargo check --locked
--workspace`, `cargo test --locked --workspace` (116 Tests), `cargo clippy
--locked --workspace --all-targets -- -D warnings`, `cargo build --locked
--workspace` sowie separater Server-Build. Der Server-Smoke-Test startete ohne
DISPLAY/WAYLAND_DISPLAY und generierte 27 Sections ohne Fenster.

Native Vulkan-Smoke-Tests fuer Wald, Plains, Wasser, Blockabbau und individuelles
Prop-Picking/Entfernen liefen damals erfolgreich. Die zugehoerigen Aufnahmen
`screenshots/overhaul-final-forest.png`, `overhaul-final-water.png` und
`overhaul-final-plains.png` zeigen dessen Mipmap-/Materialbindung.
Wald und Kueste wurden visuell geprueft: ruhigere Texturen, erhaltene Baumfarben,
sichtbare Kleinvegetation und Unterwasserboden. Wasser-High erzeugte zwei zeitlich
versetzte Aufnahmen; Low mit Wellenstaerke 0 lief ebenfalls ohne Shaderfehler.
Das ist eine Pruefung fester Szenen, kein erschoepfender Kameraflug durch jede
LOD-Grenze. Gemischte LOD-Kanten und Hysterese werden zusaetzlich geometrisch getestet.

### Referenzbild-Abgleich

Das nachgereichte Referenzbild wurde fuer den aktuellen zweiten Durchgang
betrachtet. Die Arbeit betrifft Laubgeometrie, staerker gruppierten Unterwuchs,
eigene Prop-LOD-Distanzen, statische GLTF-Szenen sowie echte
Screen-Space-Wasserreflexionen. Die logische Aufloesung und Baum-Makrostruktur
bleiben erhalten. Der vollstaendige Detailgrad und die Komposition des Bildes
sind damit nicht als erreicht nachgewiesen.

Die folgenden Nachweise gehoeren zum Abschlussstand vor der anschliessenden
Korrektur gemeldeter Luecken und Chunk-Flimmerfehler. Das damalige Dithering
ist inzwischen entfernt; diese Ergebnisse validieren den neuen Batchwechsel
und die neue Randgeometrie nicht.

Die damaligen finalen Wald-/Plains-/Wasserlaeufe mit neuen Texturen und GPU-only-Meshes
sind abgeschlossen. Ein 30-Sekunden-Vergleich bei identischer Wasserqualitaet
und abgeschalteten Wellen zeigt mit SSR die reflektierte Ufergeometrie;
der Unterwasserboden bleibt in beiden Varianten sichtbar. Auch der
48-Sekunden-LOD-Flug ist abgeschlossen; Rueckkehrbilder wurden visuell geprueft.
Die zugehoerigen Zahlen und Einschraenkungen stehen unter "Aktuelle Messung".
Fuer diesen vorherigen Stand am 14.10.2026 erfolgreich: `cargo fmt --all --check`,
`cargo check --locked --workspace`, `cargo test --locked --workspace`
(136 Tests), `cargo clippy --locked --workspace --all-targets -- -D warnings`
und `cargo build --locked --workspace`. Zusaetzlich wurden alle World-Examples
und der Server separat gebaut. Ohne DISPLAY/WAYLAND_DISPLAY generiert der
Server 27 Sections und beendet den Smoke-Test erfolgreich; sein normaler
Dependency-Baum enthaelt weder terra-render, bevy_render, bevy_winit noch wgpu.

Alle 13 nativen Client-Smoke-Modi liefen erfolgreich: Standard, Blockabbau,
Prop-Abbau, Overview, Landscape, Water, Forest, Desert, Plains, Ocean,
Reference, Custom und LOD. Dazu Wasser-Low ohne Wellen und der SSR-Ein/Aus-
Vergleich. Das opt-in Custom-Fixture zeigt drei nichtwuerfelfoermige GLTF-
Modelle neben einem prozeduralen Stone-Cube (`screenshots/reference-custom.png`).
Prop-Picking entfernt genau eine Pflanze und behaelt den Nachbarn; Session-
Replay nach Eviction/Reload wird durch die Streaming-/Render-Tests abgesichert.
Wald, Plains, Kueste, Custom-Fixture, SSR-Vergleich und LOD-Rueckwege wurden
an den nativen Bildern geprueft. Die uebrigen Smoke-Ergebnisse sind keine
Behauptung einer vollstaendigen visuellen Pruefung aller Biome.

`reflection_scenes` findet deterministische bewachsene Ufer im bestehenden Seed.
`--smoke-test-reference` verwendet eine Kamera nahe am Wasser, damit sowohl
Unterwasserboden als auch Reflexionen sichtbarer Uferobjekte pruefbar sind.

### Korrektur von Luecken und Flimmern

Nach dem Referenzbild-Abgleich wurden Luecken und flimmernde Chunks gemeldet.
Zwei unabhaengige Ursachen werden im aktuellen Code korrigiert:

- `meshing/connectivity.rs` passte an verdeckten Nachbarflaechen nur deren
  Normalachse an. An konkaven Drei-Zellen-Ecken konnten dadurch offene Keile
  zwischen unterschiedlichen Bevelbreiten oder LOD-Stufen verbleiben. Jetzt
  werden dort alle Randkoordinaten auf denselben Zellumriss gesetzt; frei
  exponierte Kanten bleiben unveraendert abgerundet.
- Im eingesetzten Bevy-0.19-Depth-only-Prepass aktiviert Dithering allein keinen
  Fragmentshader. Der Prepass konnte deshalb volle Tiefe schreiben, waehrend
  der Hauptpass Fragmente verwarf; Schatten wurden ebenfalls nicht passend
  ausgeblendet. Terrain- und Prop-Dithering sind vollstaendig entfernt.
  Aktuelle Worker-Ergebnisse ersetzen die alten Batches atomar, und die
  Prop-Ferngrenze verwendet eine abrupte Sichtbarkeitsgrenze hinter dem Nebel.

Die Korrektur fuehrt keine neuen Renderpaesse, Targets oder Shadervarianten ein.
Sichtbare Geometriespruenge bleiben als Kompromiss moeglich; Materialbindung,
LOD-Hysterese und die getrennten Prop-Distanzen bleiben unveraendert.
Am 14.10.2026 erneut bestanden: fmt --check, check --workspace,
test --workspace (136 Tests), Clippy --all-targets mit -D warnings und
build --workspace, jeweils mit unveraendertem Lockfile. Neue Regressionen
pruefen schraege Strahlen durch konkave Drei-/Vier-Zellen-Ecken, alle zehn
Materialien, gemischte LODs, negative Section-Grenzen und Micro-Laubanschluesse.
Der vorher fehlschlagende Fall Grass/Dirt/Stone mit Middle/Near/Near trifft
jetzt die Aussenflaeche statt durch den offenen Keil erst die Rueckwand.
Batchtests pruefen exakt einen aktiven Terrain-/Prop-Batch waehrend des
asynchronen LOD-Wechsels und den Erhalt alter Meshes bis zum gueltigen Ergebnis.

Der erneute native 48-s-Flug (gleicher RX-6950-XT-/VSync-Aufbau) vollendet
12997 atomare LOD-Wechsel. 2869 Frameproben ergeben 60.00 FPS,
p95 17.584 ms, p99 18.250 ms; alle Jobs sind am Ende abgeschlossen.
Der Smoke-Test bricht bei jeder nicht-abrupten Mesh-VisibilityRange ab und
wartet nach dem Rueckflug auf die letzte Screenshot-Ruecklesung.
Vier Zwischenbilder liegen unter `screenshots/seam-fix-flight-{1,2,3,4}.png`.
An den geprueften Vergleichs- und Rueckkehrbildern ist das zuvor sichtbare
Dither-Lochmuster nicht mehr vorhanden. Das ist keine frameweise Sichtpruefung
jedes Voxels; die Ecken werden zusaetzlich geometrisch getestet.

Weitere neue Native-Smokes: Blockabbau, individuelles Prop-Picking/Entfernen,
High-Wasser und Headless-Server ohne DISPLAY/WAYLAND_DISPLAY erfolgreich.
Endzeitpunkt-VmRSS im Flug: 655628 KiB, 4404 geladene Sections,
1988 gerenderte Sections, 5181 Mesh-Assets, 50605 dargestellte Props in
721 Batches. GPU-Opaque-Pass Mittel/p95/p99: 1.239/1.390/1.522 ms;
Early-Prepass 0.516/0.627/0.834 ms, je 2800 Werte. Keine Gesamt-GPU-Zeit
oder Durchsatzverbesserung aus dem VSync-Lauf abgeleitet.
Aktuelle Rohprotokolle: `tools/benchmarks/results/seam-fix-2026-10-14/`.
Die folgenden historischen Messwerte bleiben als solche gekennzeichnet.

## Performance und Messverfahren

Der CPU-/ECS-Benchmark nutzt Radius 12/3, vier Worker und einen auf 60 FPS
getakteten App-Loop. Die historische Version-4/5-Optimierung ist getrennt vom
aktuellen Rendering-Umbau zu lesen:

| Szenario | Historisch Version 4 | Historisch Version 5 |
| --- | --- | --- |
| Erstladen, 4375 Sections | 18.30 s / 1095 Frames | 2.63 s / 158 Frames |
| Bewegung um eine Spalte | 0.74 s / 45 Frames | 0.12 s / 8 Frames |
| Teleport und Entladen | 22.44 s / 5365 Sections | 4.93 s / 5273 Sections |
| Rueckkehr / erneutes Laden, 4375 Sections | 18.30 s | 2.63 s |

Diese historische Beschleunigung umfasst keine GPU-Uploads und aufgrund des
Versionswechsels nicht exakt dasselbe Terrain. Sie ist keine neue Verbesserung
durch den Rendering-Umbau.

Die separat gesicherte Version-5-Baseline vor dem ersten Rendering-Umbau verwendet denselben
`streaming_benchmark`, Seed, Radius 12/3 und vier Worker:

| Szenario | Historische Baseline | Nach erstem Umbau, vor Referenzbild-Abgleich |
| --- | --- | --- |
| Erstladen, 4375 Sections | 2.626 s | 2.626 s |
| Bewegung um eine Spalte | 0.117 s | 0.117 s |
| Teleport / Entladen | 4.934 s | 4.934 s |
| Rueckkehr / Reload | 2.626 s | 2.626 s |
| Benchmark-Prozess, Wall-Time | 10.319 s | 10.315 s |
| Benchmark-Speicher, Peak RSS | 163388 KiB | 165960 KiB (+1.57%) |
| Prozess-User-CPU | 4.349 s | 4.360 s |

Diese Messung enthaelt CPU-Generierung und ECS, keinen Renderer/GPU-Upload.
Der getaktete Loop quantisiert die Szenariozeiten; gleiche Millisekundenwerte
belegen keine identischen reinen Worker-Kosten. Der Speicheranstieg wird nicht
als Verbesserung gewertet.

Die historischen nativen Smoke-Tests protokollierten nach fertiger Generierung und
Uploads ein festes 3-Sekunden-Fenster nach 500 ms Aufwaermzeit. Framezeiten stammen aus
`Instant`, nicht aus der begrenzten Spielzeit. Durchschnitts-FPS, p95/p99,
RSS aus `/proc/self/status`, aktive/gerenderte Sections, Mesh-Assets,
sichtbare Mesh-Entities, geladene aktive Props, durch Batches dargestellte
Props und Prop-Batches werden separat ausgegeben. Screenshotnahe Frames werden
ausgeschlossen. `initial_load_seconds` beginnt beim Einrichten des Smoke-Tests
am Ende des App-Builds, nicht beim Prozessstart. Dargestellte Props sind keine
exakte Frustum-/Occlusion-Zaehlung; Mesh-Entities sind keine GPU-Draw-Call-Messung.

Historische native Messungen des ersten Umbaus am 14.10.2026:
AMD Radeon RX 6950 XT, Vulkan/RADV,
Mesa 26.2.4-arch3.1, Dev-Profil opt-level 1 (Dependencies 3), VSync an,
1280x800 logische / 1920x1200 physische Pixel, Radius 12/3, Water High.
Die drei kurzen festen Szenen wurden nacheinander ohne parallele Builds gemessen:

| Historische Nachher-Szene | FPS | p95 / p99 ms | Initial bereit | RSS KiB | Sections / Mesh-Assets | dargestellte Props / Prop-Batches |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Forest | 59.97 | 18.74 / 19.56 | 3.301 s | 1363732 | 4404 / 5171 | 28183 / 711 |
| Coast | 59.96 | 19.07 / 29.33 | 4.194 s | 829928 | 4885 / 3370 | 7777 / 313 |
| Plains | 59.93 | 18.86 / 30.59 | 3.533 s | 1214660 | 4555 / 4044 | 34912 / 650 |

Terrain-Komposition, Summe gemessener Worker-Laufzeiten / teuerster einzelner Job:
Forest 0.556 s / 3.82 ms (1988 Jobs), Coast 0.325 s / 3.04 ms (1500),
Plains 0.347 s / 3.26 ms (1662). Diese Zeiten enthalten das Zusammenbauen der
Rendergeometrie, keine Voxelgenerierung, Prop-Komposition oder GPU-Uploads;
parallel laufende Jobzeiten sind keine zusaetzliche Ladezeit.
Die Zahl frustum-sichtbarer Mesh-Entities betrug 1750/1171/1382.

Vorher wurden zehn GLBs mit insgesamt 15875208 Bytes und 20 importierten
Materialslots geladen. Normale Cubes laden jetzt null GLBs und nutzen zehn
Terrainmaterialien. Neun kleine Palette-Materialien dienen als Farbquelle fuer
die CPU-Vorlagen, ein weiteres weisses Material rendert alle Prop-Batches.
Die Prop-Batches der identischen Waldszene sanken innerhalb dieses Umbaus durch
Vertexfarben von 3846 auf 711 bei unveraendert 28183 dargestellten Props;
dieser Zwischenvergleich ist keine Messung gegen die alte GLB-Vegetation.

Reproduzieren: `cargo run --locked -p terra-world --features bevy --example
streaming_benchmark`; zusaetzliche Prozesswerte liefert
`python3 tools/benchmarks/measure.py target/debug/examples/streaming_benchmark`.
Native Szenen: `target/debug/terra-client --smoke-test-forest` bzw.
`--smoke-test-water` und `--smoke-test-plains`; `TERRA_SCREENSHOT` setzt den Bildpfad.

Native Vorherwerte fuer FPS/p95/p99, GPU-Zeit und Draw Calls wurden beim ersten
Umbau mit dieser Instrumentation nicht erfasst. Die damalige VSync-Messung ist
kein Durchsatzbenchmark; kurze p99-Werte schwanken. Keine nicht gemessene
FPS-/Speicherverbesserung wird daraus abgeleitet.
Die zusaetzliche Nahgeometrie und dritte Schattenkaskade koennen Kosten erhoehen;
Qualitaet, Hardware, Biom, Sichtweite und VSync muessen bei Vergleichen identisch sein.

### Historische Referenzmessung

Die folgenden Werte stammen vom Referenzbild-Abgleich vor der anschliessenden
Luecken-/Flimmerkorrektur und vor Laufsteuerung/Einstellungsmenue. Sie sind
keine Performance-Validierung des aktuellen Controller-/Menue-Standes.

Die folgenden Zahlen dokumentieren den Referenzbild-Abgleich vor dem aktuellen
Luecken-/Flimmer-Bugfix. Sie bleiben unveraendert als Vergleichsbasis erhalten;
neue native Messungen der atomaren LOD-Wechsel stehen noch aus.

Die folgenden nativen Laeufe vergleichen den gesicherten Stand vor diesem
Referenzbild-Abgleich mit der finalen Textur-/Geometriefassung einschliesslich
`RenderAssetUsages::RENDER_WORLD`. Die vorherigen Tabellen bleiben historische
Messungen des ersten Umbaus; sie werden nicht als diese Baseline verwendet.
Gemessen am 14.10.2026, RX 6950 XT, Vulkan/RADV, Mesa 26.2.4-arch3.1,
Dev-Profil opt-level 1 mit Dependencies 3, 1920x1200 physische Pixel, VSync an,
Radius 12/3, vier Generierungsworker, Water High. Je Szene 3 Sekunden nach
500 ms Aufwaermzeit, screenshotnahe Frames ausgeschlossen. Die feste Kamera
und die Einstellungen sind zwischen Vorher/Nachher gleich.

| Szene | Stand | FPS | p95 / p99 ms | Initial bereit | VmRSS KiB |
| --- | --- | ---: | ---: | ---: | ---: |
| Forest | Vorher | 59.96 | 18.59 / 19.00 | 3.310 s | 1364716 |
| Forest | Final | 60.03 | 18.88 / 19.60 | 3.287 s | 592416 |
| Coast | Vorher | 59.84 | 19.11 / 20.71 | 4.201 s | 843364 |
| Coast | Final | 60.64 | 18.90 / 19.33 | 4.223 s | 546320 |
| Plains | Vorher | 60.05 | 18.76 / 19.12 | 3.527 s | 1185740 |
| Plains | Final | 60.03 | 18.63 / 19.97 | 3.561 s | 534676 |

Die finalen Fenster enthalten 180/176/180 Frames. Die FPS sind der Quotient aus
verbleibenden Frames und deren gemessener Zeit, nicht ein uncapped GPU-Benchmark.
Insbesondere 60.64 FPS an der Kueste nach Screenshot-Ausschluss belegt keinen
Durchsatzgewinn. p99 ist bei diesen kurzen Fenstern nur eingeschraenkt belastbar;
Forest und Plains zeigen hier hoehere p99-Werte. Die Ladezeiten bleiben in diesen
Einzellaeufen nahe an der Baseline, sind aber kein statistischer Gleichheitsnachweis.

VmRSS ist der Prozessspeicher zum Messende, kein Peak-RSS und kein VRAM-Wert.
Die gemessenen Reduktionen betragen Forest 56.6%, Coast 35.2%, Plains 54.9%.
Das ist ein Vergleich des gesamten neuen Stands, kein isolierter Wirksamkeits-
nachweis nur der Mesh-Speicherfreigabe. CPU-Vertexdaten der fertigen Meshes werden
nicht mehr doppelt vorgehalten; logische Welt- und Generatorcaches bleiben bestehen.

| Szene | Geladene / gerenderte Sections, beide Staende | Mesh-Assets vorher -> final | Dargestellte Props vorher -> final | Prop-Batches vorher -> final |
| --- | ---: | ---: | ---: | ---: |
| Forest | 4404 / 1988 | 5171 -> 5181 | 28183 -> 50605 | 711 -> 721 |
| Coast | 4885 / 1500 | 3370 -> 3385 | 7777 -> 12188 | 313 -> 328 |
| Plains | 4555 / 1662 | 4044 -> 4086 | 34912 -> 48641 | 650 -> 692 |

Mehr Props entstehen durch neue Verteilung und erhaltene Fern-Silhouetten.
Die Zahl beschreibt in Batches dargestellte Props, nicht ausschliesslich
frustum-sichtbare Pflanzen. Mesh- und Batchzahlen sind keine Draw Calls.

| Szene | Terrain-Worker-Spannen vorher -> final | Teuerster Terrainjob vorher -> final | Finale Prop-Worker-Spannen / Jobs |
| --- | ---: | ---: | ---: |
| Forest | 0.565 -> 0.816 s | 3.78 -> 52.45 ms | 0.153 s / 721 |
| Coast | 0.327 -> 0.364 s | 2.78 -> 55.29 ms | 0.033 s / 328 |
| Plains | 0.357 -> 0.425 s | 3.23 -> 54.86 ms | 0.141 s / 692 |

Die aufsummierten Terrain-Kompositionsspannen steigen um 44.4%, 11.2% und 19.1%.
Auch die maximalen Einzeljobspannen sind deutlich hoeher und werden nicht
verschwiegen. Es sind verstrichene Worker-Zeitspannen einschliesslich moeglicher
Scheduling-Pausen, keine isolierten CPU-Instruktionszeiten oder Mainthread-Stalls.
Parallel laufende Jobs duerfen nicht zur initialen Ladezeit addiert werden.
Der erste Job initialisiert dabei einmalig die 512 wiederverwendeten Micro-
Kronenvorlagen. Die getrennte ungedrosselte Generierungsmessung folgt unten.

Die finalen Bilder liegen unter `screenshots/reference-final-forest.png`,
`reference-final-water.png` und `reference-final-plains.png`; Wasser besitzt
zusaetzlich `reference-final-water_motion.png`. Lokale Messprotokolle:
`tools/benchmarks/results/reference-2026-10-14/terra-reference-final-*.log`.
Weitere finale Mess- und Testprotokolle sind im selben Verzeichnis archiviert.

#### Wasserreflexionen

Ein eigener 30-Sekunden-A/B-Lauf nutzt dieselbe Referenzkamera, High-Qualitaet,
`water_wave_strength = 0`, 1920x1200, VSync und GPU-Diagnostik. Nur
`water_reflections` wechselt. Beide Laeufe haben 1800 CPU-Frameproben und
1799 eindeutige GPU-Messwerte je aufgefuehrtem Pass.

| Messwert | SSR aus | SSR an |
| --- | ---: | ---: |
| FPS | 60.000 | 59.996 |
| CPU-Frame p95 / p99 | 18.627 / 18.936 ms | 18.625 / 18.957 ms |
| `render/early prepass/elapsed_gpu`, Mittel | 0.404 ms | 0.394 ms |
| `render/main_opaque_pass_3d/elapsed_gpu`, Mittel | 0.995 ms | 0.994 ms |
| `render/main_transmissive_pass_3d/elapsed_gpu`, Mittel | 0.085 ms | 0.276 ms |
| Transmissiver Pass p95 / p99 | 0.086 / 0.088 ms | 0.278 / 0.283 ms |

Die gemessene Differenz des transmissiven Passes betraegt rund 0.191 ms.
Sie ist weder eine komplette GPU-Framezeit noch eine Addition aller Renderpaesse.
In `screenshots/reference-ssr-on.png` und `reference-ssr-off.png` wurde die
reflektierte Ufergeometrie mit SSR und der sichtbare Unterwasserboden in beiden
Varianten visuell geprueft. Das bestaetigt die Reflexion in dieser Szene, nicht
vollstaendige oder artefaktfreie Spiegelung ausserhalb des sichtbaren Bildes.
Protokolle: `tools/benchmarks/results/reference-2026-10-14/terra-reference-ssr-{on,off}.log`.

#### Historischer Dither-LOD-Test

Der damalige 48-Sekunden-Flug erreichte 60.06 FPS, p95 17.59 ms, p99 18.28 ms
bei 2869 aufgezeichneten Frames. Er schloss 12337 Dither-Mesh-Uebergaenge ab und
beendete sich mit null aktiven Uebergaengen. Die damalige Bevy-Pipelineinvalidierung
beim Dither-Moduswechsel beseitigte einen veralteten Pipelinezustand, nicht jedoch
die spaeter erkannte Inkonsistenz von Tiefen-, Haupt- und Schattenpass. Dithering
und seine Pipelineinvalidierung sind deshalb inzwischen entfernt. Diese Zahlen
beschreiben nicht die aktuellen atomaren Wechsel mit `LodSwapStatus`.
Die Rueckkehrbilder `screenshots/reference-lod-flight-2.png` und
`reference-lod-flight-4.png` wurden visuell geprueft. Vier Zwischenbilder und
das Startbild sind vorhanden. Es waren Stichproben, keine frameweise visuelle
Pruefung jeder LOD-Kante; sie schlossen den spaeter gemeldeten Fehler nicht aus.
Protokoll: `tools/benchmarks/results/reference-2026-10-14/terra-reference-lod.log`.
Exakte Draw Calls bleiben uninstrumentiert. GPU-Opaque-Pass dieses Fluges:
Mittel 1.484 ms, p95 1.777 ms, p99 1.860 ms (2721 Werte); Early-Prepass
0.537 / 0.676 / 0.859 ms. Kein Vorhervergleich fuer diese GPU-Serien vorhanden.

#### CPU-Streaming und Worker

Erneuter Vergleich mit dem vor diesem Durchgang gesicherten Streaming-Binary,
gleicher Seed/Version, Radius 12/3, vier Worker, 60-Hz-ECS-Takt, ohne Renderer:

| Messwert | Vorher | Final |
| --- | ---: | ---: |
| Erstladen, 4375 Sections / 158 Frames | 2.626 s | 2.626 s |
| Eine Spalte Bewegung / 8 Frames | 0.117 s | 0.117 s |
| Eviction, 5273 Sections / 296 Frames | 4.933 s | 4.934 s |
| Reload, 4375 Sections / 158 Frames | 2.626 s | 2.626 s |
| Prozess-Wall-Time | 10.323 s | 10.316 s |
| Prozess-User-/System-CPU | 4.354 / 0.099 s | 4.367 / 0.122 s |
| Prozess-Peak-RSS | 187584 KiB | 165772 KiB |

Die CPU-Erstlade-Baseline bleibt im getakteten Test erhalten. Die minimal
hoehere User-/System-CPU ist kein Geschwindigkeitsgewinn; Peak-RSS ist hier
eine Prozessmessung und nicht mit dem nativen Endzeitpunkt-VmRSS gleichzusetzen.

`worker_benchmark` misst ohne ECS-Frame-Takt, Renderer oder GPU dieselben 625
Spalten und 4375 Sections um den Standard-Spawn, Seed 42042 und Version 5.
Vier Worker arbeiten nach einem Warmup fuenf Messlaeufe ab. Ausgabe sind die
Wall-Time sowie aufsummierte Worker-Zeitspannen fuer Spaltenvorbereitung und
Section-Aufbau. `headless_generation` und `render_preparation` verwenden
dieselben Section-Positionen. Letzteres enthaelt zusaetzlich Rand-Padding,
Greedy-Meshing und Wasser; ihre Differenz ist keine isolierte Meshing-Messung.
Block-Pruefsumme, Prop- und Quad-Zahlen machen die Eingaben/Ergebnisse vergleichbar.

Finale Worker-Ergebnisse (je 5 Laeufe nach einem Warmup):

| Modus | Mittlere Wall-Time | Median | Minimum / Maximum |
| --- | ---: | ---: | ---: |
| Headless generation | 0.203699 s | 0.203734 s | 0.201838 / 0.205250 s |
| Render preparation | 0.368271 s | 0.368670 s | 0.364406 / 0.373409 s |

Beide Modi erzeugen je Lauf 50506 Props und die Block-Pruefsumme 22955883;
Render preparation erzeugt 450427 Greedy-Quads. Diese neue Instrumentation
hat keinen gesicherten Vorherlauf: daraus wird keine Beschleunigung abgeleitet.
Sie enthaelt keine spaetere Terrain-/Prop-Renderkomposition und keine GPU-Uploads.
Prozess: 3.434 s Wall-Time, 13.474 s User-CPU ueber alle Worker, 12336 KiB
Peak-RSS. Die einzelnen Worker-Spannen stehen im archivierten Protokoll.
Aufruf: `cargo run --locked -p terra-world --features bevy --example worker_benchmark`.

`TERRA_RENDER_DIAGNOSTICS=1` aktiviert Bevys `RenderDiagnosticsPlugin`.
Adapterunterstuetzte Timestamp-/Pipeline-Statistik-Funktionen werden genutzt,
ohne sie auf ungeeigneter Hardware zu erzwingen. Der Startlog nennt die
tatsaechlich verfuegbaren Funktionen. `DiagnosticsStore` liefert zeitversetzte
GPU-Passzeiten in Millisekunden und Shader-/Primitive-Zaehler. Der Client sammelt
pro Pfad neue Zeitstempel einmalig ab Beginn seines Messfensters und berichtet
Mittelwert, p95 und p99. Eltern-/Kindspans koennen ueberlappen; ihre Summe wird
nicht als GPU-Framezeit ausgegeben. CPU-Framezeiten bleiben separat.
Primitive-Zaehler, Meshzahlen und GPU-Multidraw-Batches ersetzen keine exakten
Draw Calls. `TERRA_DRAW_DIAGNOSTICS=1` zaehlt jetzt separat erfolgreiche
PBR-Draw-API-Aufrufe im bestehenden Renderbefehl, getrennt nach Phasen sowie
direkter/indirekter Zeichnung. Ein Multi-Draw ist ein API-Aufruf; GPU-entschiedene
Subdrawzahlen werden nicht aus diesen Aufrufen geschaetzt. Zusaetzlich zaehlt die
Instrumentierung erfolgreiche UI-Node-/Text-, Slicer- und Schattendraws des von
TiltUI benutzten Renderers. `TERRA_GPU_DRAW_DIAGNOSTICS=1` liest alle 60 Frames
die tatsaechlichen indirekten Farb-Drawparameter asynchron aus: maximal ein
8-MiB-Readback, GPU-Count und nichtleere Kommandos separat, mit eigener Frame-ID.
Diese Diagnose setzt COPY_SRC auf Parameterbuffers und hat eigene Kosten.
Wiederverwendete Early-/Late-Prepassbuffers werden nicht als ausgelesen behauptet.
UI-Gradient-/Custom-Material- und engineinterne Fullscreen-Draws bleiben separat
offen; GPU-Passzeiten sind kein Ersatz fuer deren Drawzaehler.
Die Instrumentierung selbst aendert das Batching nicht.
Nur bei Offline-Renderfixtures aktiviert `TERRA_FOLIAGE_REFERENCE=1` einen
Vergleich mit denselben Kronen, aber ohne Maskierung/Wind. Das ist kein
persistierbarer Grafikschalter; Gameplay-/Netzwerksitzungen ignorieren ihn,
und logische Props werden nicht ausgeblendet.

Terrain-, Prop- und Wasseruploads besitzen zusaetzlich zu den Sectionlimits
getrennte Vertexbudgets je Frame. Fertige Workergebnisse warten bei Bedarf,
waehrend die bisherigen Meshes sichtbar bleiben. Baumarten-Snapshots verwenden
begrenzte geteilte Spalten-/Sectioncaches, und fertige PNG-Mips halten keine
zusaetzliche CPU-Pixelkopie. Details, native Wasserplatzierung und alle Messwerte:
[Phase 30](tools/benchmarks/results/water-placement-performance-2026-10-16/README.md).

Phase 34 teilt identische position/normal/UV-Ecken innerhalb verbundener Quads
ohne Dreiecks-/Kantenveraenderung. Windlaub verwendet auf Meshing-Workern
Snorm8-Normalen und Unorm8-Farben mit HDR-Decodierung nur fuer dieses Vertexlayout:
28 statt 48 Vertexbytes, ohne Aenderung von Positionen, UVs, Masken oder Wind.
Vorschauen und Abbauteile mit Floatfarben bleiben korrekt. LOD-Scans verwenden
Meter-Buckets fuer den horizontalen Fokus; Edits und fertige Animationen
invalidieren weiterhin sofort. Lange native Messungen, Frame-Spitzenkontext,
GPU-Readback und konkrete Grenzen stehen im
[Phase-34-Bericht](tools/benchmarks/results/rendering-streaming-2026-10-16/README.md).

Noch offen: externe Account-Verifikation, Mehrrechner-/Internet-Abnahme und
der vollstaendige Referenz-Artstil. Dedicated-Saves, erweiterte Spielersteuerung,
Mehrspieler-Synchronisation und Streaming
um mehrere Spieler sind implementiert; Singleplayer-Edits werden gespeichert.
Crafting mit Werkbaenken ist seit Phase 45 implementiert (siehe unten).
Kreaturen und ein modulares Bausystem bleiben offen. Truheninventare sind
seit Phase 49 implementiert; das Ofen-Backend folgt in Phase 87.

### Aktueller Controller-/Appearance-Nachweis

14.10.2026, gleiche RX 6950 XT, RADV Mesa 26.2.4, VSync, 1920x1200 physisch,
48 Sekunden identischer LOD-Flug, Seed 42042, Radius 12/3. Vorher stammt aus
`seam-fix-2026-10-14/terra-seam-flight.log`, aktuell aus
`player-settings-2026-10-14/flight-final.log`. Kein paralleler Build/Testlauf.

| Kennwert | Vorher | Aktuell |
|---|---:|---:|
| Durchschnittliche FPS | 60.005 | 59.997 |
| p95 / p99 CPU-Wall-Framezeit | 17.584 / 18.250 ms | 18.286 / 29.424 ms |
| Initialer Ready-Zeitpunkt | 3.308 s | 3.310 s |
| Terrain-Komposition, Worker-Spannen | 3.037 s | 3.128 s |
| Prop-Komposition, Worker-Spannen | 1.042 s | 1.147 s |
| RSS am Messende | 655628 KiB | 668812 KiB |
| Aktive / gerenderte Sections | 4404 / 1988 | 4404 / 1988 |
| Mesh-Assets / sichtbare Mesh-Entities | 5181 / 1672 | 5181 / 1672 |
| Repraesentierte Props / Prop-Batches | 50605 / 721 | 50605 / 721 |
| Abgeschlossene LOD-Wechsel | 12997 | 12997 |

Die p99-Regression ist real in diesen Laeufen und bleibt zur weiteren
CPU-/Streaming-Profilierung offen. Zwei vorherige Laeufe dieses Umbaus ergaben
28.415 bzw. 29.116 ms p99; sie wird nicht als einmaliger Ausreisser verworfen.
Die gebuendelte Biomtoenung senkte die gemessenen Prop-Worker-Spannen gegenueber
diesen Zwischenstaenden (1.298/1.318 s), beseitigt aber die Frame-Spitzen nicht.
Es gibt keinen Nachweis eines allgemeinen FPS-Gewinns.

GPU Main-Opaque-Pass: vorher 1.239 ms Mittel, aktuell 1.244 ms
(p95 1.394, p99 1.654 ms). Early-Prepass aktuell 0.501 ms Mittel.
Das sind einzelne GPU-Paesse, keine Gesamt-GPU-Framezeit. Exakte Draw Calls
bleiben nicht instrumentiert; repraesentierte Props sind nicht gleich
frustum-sichtbare Einzelpflanzen.

CPU-Erstgenerierung bleibt im 60-Hz-Streamingbenchmark bei 2.626 s fuer 4375
Sections, ohne GPU-Upload. Vier-Worker-Durchsatz: Headless 0.204690 s statt
0.203699 s, Render-Vorbereitung 0.363971 s statt 0.368271 s (je fuenf Laeufe).
Block-Checksumme 22955883, 50506 Props und 450427 Greedy-Quads bleiben identisch.
Die kleinen Zeitdifferenzen werden nicht als relevante Optimierung gewertet.

Alle fuenf Workspace-Pruefungen bestanden, einschliesslich 171 Tests und
Clippy ohne Warnungen. Server separat gebaut und ohne Display gestartet.
Screenshots, Rohprotokolle, Kommandos und die Einschraenkung der echten
Pointerpruefung stehen unter
[player-settings-2026-10-14](tools/benchmarks/results/player-settings-2026-10-14/README.md).

### Prop-Plaetze Und Weiche Biomfarben (Phase 36)

`terra_world::props::placement` ist die gemeinsame Belegungsregel fuer
prozedurale Gruppen und serverseitiges Pflanzen: vier Einheiten pro Bodenblock,
mittiges Gras kostet zwei, andere Arten eine. Seit Phase 37 stehen einzelne
Pflanzen ohne Gras zentral, zwei nebeneinander, drei im Dreieck und vier im
Quadrat. Bei Gras bleiben Begleitpflanzen diagonal aussen. Die bisherige primaere
Prop-ID bleibt bestehen, weitere Kandidaten erhalten reproduzierbare Slot-IDs.
Gepflanzte Props und Entfernungen benutzen weiter dasselbe EditJournal und
dieselben binaeren Regionen. Gruppenpositionen werden nach aktiven IDs
abgeleitet: Hinzufuegen/Entfernen ordnet nur dieselbe Bodenblockgruppe neu.
Rohpositionen im Journal bleiben unveraendert; Generierung, autoritativer Commit
und Snapshot-/Reload-Anwendung verwenden dieselbe Anordnung. Nicht passende
alte Uebergroessen werden nicht automatisch umgestellt. Neue Nicht-Gras-Props
passen in maximal 0.5 x 0.5 m, also ein Viertel der Grundflaeche; das Gras
umspannt weiterhin etwa 80 Prozent der Bodenblock-Grundflaeche.

Die kanonische Geometrie normalisiert Grundflaeche und Hoehe getrennt: Blumen
sind im Nahbereich 0.65 bis 0.95 m hoch, kleine Pilze 0.35 bis 0.55 m. Varianten
und Groessen bleiben deterministisch. Die Hoehengrenzen gelten auch fuer alte
gespeicherte Blumen-/Pilzskalierungen als reine Darstellung, ohne Save-Migration.
Template-Ausmasse werden einmal je Art/Variante gecacht und ueber alle LODs
zentriert; Rendern, genaue Dreiecksselektion und konservative Bounds teilen die
Geometrie. Keine neuen Entities, Materialbatches oder zusaetzlichen Vertices.
Die Gruppenbildung pro generiertem Bodenblock verwendet nur vier Stackplaetze;
Section-Reflow ist auf veraenderte Prop-Gruppen/Journal-Sections begrenzt.

Die gemeinsame CPU-/Rendergeometrie liefert stumpfe verbundene Grashalme mit
offenen Zwischenraeumen, niedrigen Blaettern und warmen Spitzen. Picking trifft
weiter die tatsaechlichen Dreiecke statt eine volle Grasbox. Nah: 552 oder 644
Vertices, Mittel: 160, Fern: 80. Nahdetails sind damit nicht kostenlos;
Section-Batches, gemeinsame Materialien, Wind und Uploadbudgets bleiben erhalten.

Die bisherige Material-Lotterie pro Bodenblock ist entfallen. Physische Grass-,
Sand- und Snow-IDs bleiben echte Bloecke mit zusammenhaengenden Konturen.
`appearance::GroundTint` verwendet vier gemeinsame Klimaproben, gecachte
Fluss-/Oasenpegel und die tatsaechliche Vertexhoehe. Das vermeidet erneute
Terrain-/Tunnelprofil-Abfragen fuer jede Farbe und behaelt schmale Flussbetten.
Topfarben werden in linearem Farbraum an den mittleren Albedos der gelieferten
Texturen ausgerichtet; Texturen, Materialhandles und Relief bleiben erhalten.
Eigene Resource-Pack-Texturen mit stark abweichendem Grundalbedo koennen eine
erneute Kalibrierung dieser Referenzwerte benoetigen. Gebaute Sand-/Schneebloecke
ausserhalb der Uebergangsbaender werden nicht gruen gefaerbt. Wetterfaktoren
kommen fuer alle drei Materialien aus denselben Regeln und erhalten JSON-Basen.

`meshing::tint_faces` prueft die wirkliche Dreiecksinterpolation grober
Topflaechen. Nur bei mehr als 0.025 Farbfehler an Innenproben wird bis hoechstens
zu Meterzellen unterteilt. Gleichfarbige und linear gefaerbte Flaechen bleiben
Greedy-Rechtecke. So gilt der Verlauf auch innerhalb eines LOD-Quads, nicht nur
an dessen Ecken; Weltkanten und negative Koordinaten teilen dieselben Proben.

`water::biome` mischt kontinuierliche Biomgewichte statt dominante Biomnamen.
Der bestehende Wasserpass bekommt ein RGB-Vertexattribut (12 Byte/Vertex);
UVs fuer Tiefe/Ufer und Stroemung sowie COLOR fuer Fall-/Schaummetadaten bleiben
getrennt. Absorptions- und Streufarbe folgen gemeinsam dem Biom. Meer, Fluss,
Fall, dynamisches Zellwasser und feste Wasserbloecke nutzen ein Material.
Die Streufarbe folgt dem RGB-Verlauf; die Extinktion geht bei abweichenden
Biomfarben weich von den bestehenden Tiefland-Koeffizienten zu neutraler
Absorption ueber. So wird lila Wasser ueber dunklem Boden nicht braun, waehrend
reines Wald-/Plains-Wasser seine bisherigen Werte behaelt.
Vier gemeinsame Section-Proben vermeiden Farbspruenge an Grenzen; keine neuen
Entities, pro-Biom-Materialien oder Reflexionspaesse. Unterwassernebel wird
dadurch nicht automatisch umgefaerbt. Sehr abrupte Pegelwechsel an Wasserfaellen
und Oasen werden bei der Terraintoenung weiterhin raeumlich angenaehert.

Die neuen nativen `TERRA_BIOME_SCENE=water-mushroom|water-desert|water-highland|water-transition`
verwenden `--smoke-test-biomes` und suchen ausschliesslich echtes generiertes
Wasser. Hoehen, Seeds und Speicherformate bleiben gleich; nicht editierte
Oberflaechenmaterialien und generierte Prop-Gruppen aendern sich absichtlich.

Messpaar, Reproduktion und native Pruefszenen:
[Phase-36-Bericht](tools/benchmarks/results/biome-props-2026-10-16/README.md).

## Weltzeit Und Tag/Nacht (Phase 38)

Integrated und Dedicated Server besitzen dieselbe `WorldClock` mit Tagesnummer,
Tagesanteil, Tageslaenge und Pause. Ein kompletter Tag dauert standardmaessig
1440 reale Sekunden (24 Minuten), neue Welten beginnen um 10 Uhr. Die Uhr folgt
der verstrichenen Serverlaufzeit, unabhaengig von Physikticks, Client-FPS und
Zeitzone. Bei gestopptem Server vergeht keine Weltzeit. Das lokale ESC-Menue
haelt einen laufenden Host nicht automatisch an.

Fuer neue Welten/Dedicated in `configs/world.toml`:

```toml
[time]
day_length_seconds = 1440.0
start_hour = 10.0
paused = false
```

Bestehende Singleplayer-Welten besitzen diese Einstellungen unter
`[manifest.world.time]` in `worlds/<name>/world.toml`; Dateien bei geschlossenem
Host bearbeiten. Fehlende Tabellen nutzen Defaults. `start_hour` gilt nur,
wenn noch keine Uhr gespeichert ist. Tageslaenge (60..604800 Sekunden) und
Pause werden beim Start aus den Weltsettings uebernommen, der gespeicherte
Kalender bleibt erhalten. Dedicated darf diese drei Einstellungen aendern,
ohne seine Generatoridentitaet oder vorhandenes Terrain zu veraendern.

Die Uhr liegt im kleinen binaeren `level.data` innerhalb derselben atomaren
Speichertransaktion wie Regionen und Spieler. Der explizite Record-Typ `IDX2`
erganzt den alten `INDX`-Index um die Uhr; alte Indizes werden weiterhin gelesen.
Region-/Spielerdateien, Welt-/Block-IDs und Generatorversion bleiben unveraendert.
Autosave, manueller Save und Shutdown sichern denselben Kalenderstand. Reine
Zeitfortschritte markieren keine Terrain-/Wasserdaten als geaendert.

Protokoll 15 uebermittelt die Uhr im Welcome und danach zweimal pro Sekunde.
Clients extrapolieren und gleichen kleine Abweichungen weich aus; groessere
Korrekturen und Pausenaenderungen werden direkt uebernommen. Reconnect und
Weltwechsel verwerfen die vorige Zeitreferenz. Der Server bleibt headless.

Eine gemeinsame Sonnenbahn steuert Bevy-Atmosphaere, sichtbare Sonnenscheibe,
Mondlicht, Schatten, Umgebungslicht, Belichtung und Nebel. Sanfte Kurven verbinden
Morgen, Tag, Abend und Nacht. Nachts bleiben Terrain und Unterwasserboden durch
schwaches blaues Mond-/Ambientlicht lesbar. Die Brightness- und Shadow-Optionen
gelten weiter; jeweils nur der oberhalb des Horizonts stehende Himmelskoerper
wirft Schatten. Vorschau-/Inventarlichter bleiben unabhaengig.

Der bestehende Himmelabschluss zeichnet zusaetzlich eine strukturierte Mondscheibe
und einen rotierenden, deterministischen Sternenhimmel. Keine Einzelstern-Entities,
zusaetzlichen Texturen oder neuen Fullscreen-Paesse. Bei Tag wird der Sternteil
des Shaders uebersprungen. Fog Off blendet nur den Nebelabschluss aus; Mond und
Sterne bleiben sichtbar. Unter Wasser werden sie ausgeblendet.

Wasser nutzt weiter seine PBR-Atmosphaerenreflexion und optional SSR mit derselben
Beleuchtung. Die dekorative Mondoberflaeche/Sternpunkte sind nicht Teil der
Atmosphaeren-Cubemap; nur SSR kann deren sichtbare Bilddaten erfassen. Mondlicht
und atmosphaerische Nachtfarben sind auch ohne SSR vorhanden. Astronomische
Mondphasen und Jahreszeiten sind nicht Teil dieser ersten Tagesuhr.

Native Reproduktion: `terra-client --smoke-test-daylight` mit den Configs im
[Phase-38-Verzeichnis](tools/benchmarks/results/daylight-2026-10-17/).

## Chat-Befehle Und Voxel-Himmelskoerper (Phase 39)

Der TiltUI-Chat zeigt kontextabhaengige Vorschlaege oberhalb der Eingabe.
Pfeil hoch/runter waehlt einen Vorschlag, Tab uebernimmt und blaettert weiter,
Shift+Tab rueckwaerts; ein Klick uebernimmt ohne sofortiges Ausfuehren. Enter
sendet. Ohne passende Vorschlaege bleibt die Eingabehistorie per Pfeiltasten
erreichbar. Lange Listen scrollen zur Auswahl. Normale Chattexte werden nicht
als Befehle oder HTML interpretiert.

- `/time set day|night|morning|afternoon`: 12:00, 00:00, 06:00 beziehungsweise 15:00.
- `/time set 23:30` oder `/time set 11:30PM`: dieselbe Uhrzeit. `12:00AM` ist Mitternacht, `12:00PM` Mittag. Beide Formate sind immer erlaubt; eine globale Sprachumschaltung existiert bisher nicht.
- `/tp -120.5 80 240`: absolute Fusskoordinaten. Das Ziel darf frei in der Luft liegen; solide Bloecke und ungueltige Zahlen werden abgelehnt.
- `/tp Spielername`: Position eines verbundenen Spielers zum Befehlszeitpunkt. Namen mit Leerzeichen in Anfuehrungszeichen setzen, beispielsweise `/tp "Alice Smith"`. Doppelte Namen werden als mehrdeutig abgelehnt.
- `/clear`: nur den eigenen sichtbaren Chatverlauf leeren. Andere Clients, die Befehls-Eingabehistorie und laufende Teleports bleiben unberuehrt.
- `/help` und `/locate biome <name>` bleiben vorhanden.

Zeit und Teleport benoetigen Creative-Modus oder die serverseitige
Environment-Berechtigung. Die Tagesnummer und Pause bleiben bei `/time set`
erhalten. Zeit wird im bestehenden Journal gespeichert und auf alle Clients
repliziert. Teleports verwenden den bestehenden Lade-/Delta-Handshake samt
neuer Bewegungsgeneration; keine direkte UI-Manipulation der Spielerposition.
Protokoll 16 liefert reale verbundene Spielernamen fuer Vorschlaege und
aktualisiert diese bei Join/Leave. Client und Server gemeinsam aktualisieren;
Generatorversion und Speicherformate bleiben gleich.

Der vorhandene Himmelshader zeichnet jetzt eine goldene abgestufte Sonne mit
acht Blockstrahlen und einen hellen Sichelmond mit kuehlem Rand, Zellrelief und
dezentem Leuchten. Es handelt sich um prozedurale Voxeloptik im Himmel-Layer,
nicht um Tausende einzelne Wuerfel oder neue Draw-Paesse. Bevy-Sonnenscheibe
deaktiviert, Sonnenlicht/Atmosphaere behalten. Die dekorativen Himmelskoerper
sind weiterhin nicht in der Atmosphaeren-Environmentmap enthalten. Unter Wasser
werden sie ausgeblendet; Fog Off und Tageslauf bleiben unterstuetzt. Das ist
eine Umsetzung der Referenzformen, keine pixelidentische Kopie des Bildes.

## Rundum-Streaming Und Ruhigere Vegetation (Phase 40)

Das CPU-Streaming fordert Sections bereits richtungsunabhaengig rund um den
Spieler an. Die optionale Einblendwarteschlange hatte fertig generierte,
hochgeladene Bereiche hinter der Kamera trotzdem unsichtbar gehalten. Die
Hintergrundfreigabe oben beseitigt diese Blickabhaengigkeit. Jobanzahl,
Uploadbudgets, Sichtweite und normales Frustum-Culling behalten ihre Funktion.

Die naechtliche Umgebungshelligkeit betraegt jetzt 300 statt 90 und nutzt einen
etwas neutraleren Blauton. Dadurch erhalten auch Schatten zwischen 19 und 22 Uhr
mehr Fuelllicht. Der bestehende weiche
Tagesverlauf, Tageshelligkeit, Belichtung und dunkle Sternenhimmel gelten weiter;
der gleiche Faktor erreicht auch die Unterwasserdarstellung.

Natuerliche Blumen und kleine Pilz-Props behalten deterministisch ein Viertel
ihrer bisherigen Kandidaten. Der Filter arbeitet nach der vollstaendigen
Gruppenauswahl: entfernte Kandidaten werden nicht durch andere Pflanzen ersetzt,
behaltene IDs/Varianten/Groessen bleiben stabil. Verbleibende Gruppen werden wie
bisher zentriert. Gras, Buesche, Farne, platzierte Pflanzen und grosse Pilzbaeume
werden nicht ausgeduennt. Bereits im RAM geladene Gruppen aendern sich beim
naechsten Generieren/Reload; bestehende Entfernungsdaten bleiben gueltig.

Verteilungszahlen, native Aufnahmen und Testprotokolle:
[Phase-40-Bericht](tools/benchmarks/results/surroundings-2026-10-17/README.md).

## Abendlicht, Pilzdetails Und Menues (Phase 41)

Stilisiertes Sonnen-/Mondlicht behaelt den Himmelsazimut, begrenzt aber seinen
flachen Abendwinkel auf mindestens 30 Grad. So bleiben Schatten auf ebenem
Untergrund kuerzer und Oberflaechen ab 17 Uhr besser beleuchtet. Sichtbare
Himmelskoerper und Tagesuhr bleiben erhalten; die Atmosphaerenbeleuchtung folgt
der kuenstlerischen Lichtrichtung. Keine physikalisch exakte Schattenprojektion.

Das Relief grosser Pilze ist nicht mehr an den etwa 48 Meter nahen Micro-
Oberflaechenwechsel gebunden. Es bleibt bis zum Terrain-Near-LOD voll erhalten
und im Middle-LOD vereinfacht sichtbar (Default 144/240 Meter mit Hysterese,
durch den vorhandenen LOD-Regler skalierbar). Bestehende Section-/Material-
Batches bleiben erhalten. Ein entfernter Detailkoerper ist kein eigener Draw.

Der Ladebildschirm zeigt durchgaengig `fertig / gesamt chunks`; gezaehlt werden
echte horizontale Chunk-Spalten mit allen angeforderten vertikalen Sections und
Terrain-Uploads. Spielstart erst nach Vegetation und initialer Wasserdarstellung,
ohne auf das Ende dynamischen Weiterfliessens zu warten. Die Zahl haengt von der
Sichtweite ab und ist nicht auf den Beispielwert 48 festgelegt.

`UI Scale` steht oben unter Graphics, mit bestehender
Apply-/Reset-/Discard-Persistenz in `configs/graphics.toml`. Seit Phase 42 wird
die Groessenaenderung erst nach Apply uebernommen, nicht waehrend der
Reglerbedienung oder beim Auswaehlen der Reset-Defaults. Die Skalierung
wirkt relativ zur automatisch eingepassten Menuegroesse, auch bei HiDPI und
kleinen Fenstern. Eine Sicherheitsgrenze gegen unbedienbare Uebergroesse wird
im Wert als `fit` angezeigt. Das gespeicherte Wunschmass bleibt erhalten.

Umsetzung, Reproduktion und Pruefergebnisse:
[Phase-41-Bericht](tools/benchmarks/results/menu-lighting-lod-2026-10-17/README.md).

Schatten werden seit Phase 42 nicht mehr hart bei Himmelsrichtung `y=0.05`
abgeschaltet. Dort strahlt die Sonne noch mit rund 47.000 Lux; unbeschattetes
Laub und Props fuehrten so zu abrupten Helligkeitsspruengen. Shadowmaps bleiben
bis zur vollstaendig auf null ausgeblendeten Lichtenergie aktiv. In der kurzen
Ueberlappung von Sonnen-Nachgluehen und beginnendem Mondlicht koennen beide
bestehenden Lichtquellen Schatten werfen. Wind, Blattmasken, Cascade-Aufloesung
und Helligkeitskurven bleiben erhalten. Kein zusaetzliches Licht oder Material.
Pruefungen und Grenzen:
[Phase-42-Bericht](tools/benchmarks/results/apply-shadows-2026-10-17/README.md).

## Player Stats Und Survival (Phase 43)

`terra-core::PlayerStats` behaelt Abbau-/Bau-/Prop-Erntezaehler und ergaenzt
Todeszaehler sowie `SurvivalVitals`. Gesundheit, Ausdauer, Hunger und Sauerstoff
liegen zwischen 0 und 100. Neue/alte Spieler ohne Survival-Daten starten voll.
Nur der Server entscheidet ueber Schaden, Essen, Tod und Respawn. Creative
behaelt volle Werte und nimmt weder Fall- noch Hunger-/Ertrinkenschaden.
Die gemeinsame `CharacterSimulation::survival_step` begrenzt Ausdauer auf
Server und in Client-Prediction/Replay mit denselben erfolgreichen Physikticks.
Fehlende Kollisionsdaten verbrauchen keine Simulationszeit fuer Vitalwerte.

| Regel | Survival-Default |
| --- | --- |
| Sprint | 12 Ausdauer/s bei tatsaechlicher Bewegung |
| Sprung | 10 Ausdauer und 0.2 Hunger pro ausgefuehrtem Bodensprung |
| Schwimmen/Tauchen | Zusaetzlich 4 Ausdauer/s bei aktiver Bewegung |
| Erholung | Nach 1 s Pause 18 Ausdauer/s; Erschoepfung bei 0, Sprint/Sprung erst ab 25 wieder frei |
| Hunger | 0.025/s Grundbedarf, +0.025 Bewegung, +0.15 Sprint, +0.07 Schwimmen |
| Heilung | Ab 75 Hunger und 4 s schadensfrei 1.25 HP/s; kostet zusaetzlich 0.16 Hunger/s |
| Fallschaden | Ab mehr als 3 m Fallhoehe 8 HP je weiterem Meter; Wasser/erlaubter Flug setzen Fallhistorie zurueck |
| Atemluft | Kopf unter realem Wasser: 5 Sauerstoff/s, damit 20 s bei voller Luft; an Luft +30/s |
| Ertrinken / Verhungern | 8 HP/s ohne Luft unter Wasser / 1 HP/s bei leerem Hunger |
| Nahrung | Kleine gesammelte Pilze: +18 Hunger, R im gewaehlten Hotbarplatz |

Erschoepfung sperrt keinen Schwimmauftrieb und keinen bewussten Uferausstieg.
Der Atemtest verwendet den Spielerkoerper samt Augenhoehe, nicht die
Third-Person-Kamera. Quelle ist dieselbe reale Wasserbelegung wie fuer Bewegung.
Nahrung nutzt bestehende Prop-Ressourcen, keinen neuen Itemtyp und kein zweites
Inventar. Der Server prueft Spielmodus, Leben, Slot, Nahrungswert, Rate und
Inventarrevision; Entnahme und Saettigung werden zusammen uebernommen.
`Use / Eat Selected Item` ist unter Controls rebindbar. Alte Controls erhalten
die neue Aktion kompatibel auf einer freien Taste; bestehende Belegungen bleiben.

Bei null HP werden Bewegung, Abbau, Platzierung und automatische Aufnahme
gesperrt. Der Respawn-Button nutzt die vorhandene Festland-Spawnpruefung und
wartet auf freien, trockenen und tragenden Boden sowie geladene Zielkollision.
Die bestehende Teleport-/Chunk-Synchronisierung setzt Eingabegeschichte und
Bewegungsgeneration zurueck. Erst dann werden die vier Vitalwerte gefuellt und
Gameplay/Mauseinfang wieder aktiviert. Zwei Sekunden Schutz verhindern Schaden
unmittelbar nach Respawn. Das Basisregelwerk behaelt Inventar, Cursor und
Zaehler; es gibt noch keinen konfigurierbaren Inventarverlust beim Tod.
Reconnect fuellt nichts auf, tote Spieler bleiben bis zum Respawn tot.
Die Serversimulation laeuft auch im ESC-Menue weiter.

Spielerwerte bleiben im vorhandenen Journal nach UUID und Welt gespeichert.
`players/<UUID>.data` schreibt `PLY2` statt des alten `PLYR`-Record-Typs. Der
explizite Altdecoder erhaelt Inventar, Cursor, Revisionen und die drei alten
Zaehler. Neue Vitalwerte nutzen volle Defaults. Die bekannte atomare Transaktion,
Regionsdateien und Weltformat 2 bleiben erhalten; kein neues Save-System.
Metadatenupdates umgehen bewusst Bevy-Terrain-Change-Detection und erhoehen
stattdessen `EditJournal::player_state_revision`. Dedicated-Autosave erkennt
diesen Zaehler auch bei stillstehender Weltzeit. Owner-Statistiken werden bei
Aenderung maximal zehnmal pro Sekunde ueber den zuverlaessigen Kanal versendet;
Bewegungssnapshots tragen ebenfalls die autoritativen Vitalwerte fuer Prediction.
Respawn-/Statistiknachrichten sind an die Bewegungsgeneration gebunden.
Protokoll 17 erfordert gemeinsam aktualisierte Clients und Server.

Das TiltUI-HUD liegt unten links und ist unabhaengig von F1/Debug. Ein
facettiertes rotes Herz und ein goldener Blitz begleiten HP-/Ausdauerleisten.
Das Hungersymbol wechselt in fuenf Stufen von voller Keule bis zum blanken
Knochen; Atemanzeige erscheint nur bei fehlender Luft. Ein gemeinsamer
transparenter Bitmapatlas versorgt alle Symbole. Schadenfeedback, Erschoepfung,
Tod und wirklicher Respawnstatus werden ebenfalls ueber TiltUI dargestellt.
In Creative und Menue-/Inventarseiten bleibt das Gameplay-HUD ausgeblendet.

Native Reproduktion mit isoliertem frischem Save:

```bash
env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra-survival-menu-fresh \
  TERRA_SURVIVAL_SMOKE_DIR=/tmp/terra-survival-world-fresh \
  TERRA_CAPTURE_DIR=/tmp/terra-survival-captures \
  target/debug/terra-client --smoke-test-survival
```

Der Test nutzt echten Integrated Server, UDP, gespeicherte Ausgangswerte und
serverbestaetigte Inventaraktionen. Danach: Laufen bis zur Erschoepfung, Erholung,
autorisierter Testflug, echter Sturztod und Respawn ueber den gerenderten TiltUI-
Button. Der Fixturepfad muss neu sein, vorhandene Benutzerwelten bleiben unberuehrt.
Native Aufnahmen, Tests, Art-Prompt und Grenzen:
[Phase-43-Bericht](tools/benchmarks/results/survival-2026-10-17/README.md).

## HUD-Layout, Spielermodus Und Schatten (Phase 44)

Das kompakte TiltUI-Survival-HUD beginnt oben links mit 12 UI-Pixeln Abstand:
rosa Herzbalken, gelber Ausdauerbalken und die bestehende Keule mit Bissstufen
und Hunger-Prozentwert. Unter Wasser kommt der Sauerstoffbalken hinzu.
Creative blendet die Survival-Anzeige weiterhin aus.

`Options > Graphics > HUD Layout > Edit Layout` oeffnet einen eigenen Editor.
Nur der Statuscontainer ist ziehbar; die Hotbar bleibt fest und nicht interaktiv.
`Save` schreibt `hud_position = [x, y]` in die vorhandene `configs/graphics.toml`.
Die Werte 0..1 beziehen sich auf den verfuegbaren Platz nach Abzug von Panel und
Rand. Dadurch bleibt der Container bei Aufloesungs-/UI-Scale-Aenderungen im Bild.
Alte Konfigurationen ohne dieses Feld erhalten `[0.0, 0.0]`.
`Reset` setzt die Arbeitsposition oben links; `Cancel` und Escape verwerfen sie.
Andere ungespeicherte Grafikoptionen werden dabei weder angewendet noch gespeichert.
Im Hauptmenue beziehungsweise in Creative verwendet der Editor Beispielwerte.

`/gamemode survival` beziehungsweise `/gamemode 0` und `/gamemode creative`
beziehungsweise `/gamemode 1` aendern ausschliesslich den eigenen Spielermodus.
Vorschlaege und `/help` kennen den Befehl. Der Integrated-Server-Besitzer erhaelt
die Berechtigung automatisch. Dedicated-Server vergeben sie explizit je UUID:

```toml
[player_permissions."00000000-0000-4000-8000-000000000001"]
build = true
flight = false
environment = false
game_mode = true
```

Dies ist eine Beispiel-UUID, durch die echte Profil-ID ersetzen. Ohne `game_mode`
ist die neue Berechtigung standardmaessig aus. Berechtigte Creative-Spieler
koennen fliegen; unabhaengige explizite `flight`-Rechte bleiben erhalten.
Inventar und Cursorstack werden nicht geloescht. Abbau, Ressourcenverbrauch,
Schaden und HUD verwenden den autoritativen Spielermodus. Ein Wechsel verwirft
alte Bewegungseingaben durch eine neue Vorhersagegeneration, beendet Abbau und
setzt die Fallhistorie zurueck. Tote Spieler beziehungsweise laufende Teleports
muessen zuerst respawnen beziehungsweise fertig laden.

Der Override steht in `PlayerStats.game_mode` im bestehenden Spielerdatensatz.
`None` behaelt den Welt-Standard bei. Neue Dateien verwenden `PLY3`; `PLY2`
(Survival) und `PLYR` (alte Zaehler) werden mit expliziten Legacy-Strukturen
eingelesen, ohne Inventar oder Werte zu verlieren. Netzwerkprotokoll 18 erfordert
gemeinsam aktualisierte Clients und Server. Welt-/Generatorversion, Seed und
Block-IDs bleiben unveraendert.

Schattenqualitaet Mittel/Hoch aktiviert Bevy 0.19.1 `TemporalAntiAliasing` und
`ShadowFilteringMethod::Temporal` auf der Weltkamera. Bewegungsvektor-Prepass und
zeitliche Stichproben stabilisieren bewegte Schattenkanten; vorhandene Windshader
liefern bereits die vorherige Vertexposition. MSAA ist in diesem Pfad aus.
Niedrig/Aus nutzt weiterhin Gaussian/MSAA ohne zeitlichen Verlauf. Es wurde nur
das vorhandene Bevy-Feature `bevy_anti_alias` aktiviert, keine Version aktualisiert.
Die erste Schattenkaskade reicht maximal 16 statt 24 Meter, die Ueberlappung
betraegt 25 Prozent. Aufloesungen, Kaskadenanzahlen und maximale Schattenreichweite
bleiben gleich. Grafikoptionen besitzen nun die einzige laufende
Kaskadenkonfiguration; Sichtweitenaenderungen setzen sie nicht separat zurueck.
Kamera-/Teleport-/Uhrzeitspruenge verwerfen den zeitlichen Verlauf.
Transparente Flaechen und Streaming-Fades bleiben ein TAA-Ghosting-Pruefpunkt;
der Filter ist kein kostenloser Effekt und keine generelle FPS-Paritaet ist zugesagt.

### Rezeptbasiertes Crafting (Phase 45)

Die Werkbank verwendet bewusst eine Rezept-/Montageansicht statt eines
verpflichtenden 3x3-Rasters. Im Inventar oeffnet **Crafting** dieselbe Ansicht
fuer Handrezepte. Werkbank herstellen, in der Welt platzieren und mit der
Use-Taste (Default R) oder Rechtsklick oeffnen; Shift unterdrueckt das Oeffnen
und erlaubt normales Platzieren. Auswahl, Suche, Zutatenbestand, Ergebnis,
Menge 1..128, **Craft**, **Craft Max** und Fertigung auf den Cursor sind TiltUI.
Fehlende Zutaten, voller Rucksack/Cursor und fehlende Werkbank werden angezeigt.
Das bestehende persoenliche 2x2-Raster mit echten Inventar-Slotreferenzen bleibt
erhalten und nutzt denselben Rezeptkatalog, einschliesslich Ergebnisvorschau.

`terra-core::crafting::RecipeBook` ist die gemeinsame Registry fuer Handraster,
Montageansicht und Server. `assets/terra/recipes/basic.toml` liefert 18 eingebaute
Rezepte: passende Planks, Sandstein, Staebe, Werkbank, Truhe, Holzspitzhacke,
Steinspitzhacke, Steinaxt und Steinschaufel. Oak/Birch/Spruce-Planks haben eigene
Zutatenvarianten. Die Grundrezepte sind eingebettet; Erweiterungen werden beim
Start aus `<config_directory>/recipes/*.toml` und `*.json` geladen (normalerweise
`configs/recipes/`). Keine zweite Konfiguration oder Laufzeit-Zufalls-IDs.

Beispiel einer additiven Mod-Rezeptdatei mit bestehenden Items:

```toml
schema_version = 1
[[recipes]]
id = "my_mod::compact_sandstone"
name = "Compact Sandstone"
station = "workbench"
output = { item = { block = 11 }, count = 2 }
pattern = { kind = "shapeless", ingredients = [{ item = { block = 4 }, count = 6 }] }
```

Geformte Rezepte nutzen `kind = "shaped"`, `width`, `height` (je 1..3) und
zeilenweise `cells`. JSON kann Luecken als `null` enthalten. Im Handraster
zaehlen Anordnung und Zutatenmenge; die Werkbank-Montageansicht verwendet die
zusammengefasste Stueckliste ohne Rasterzwang. Neue Items muessen weiterhin im
vorhandenen Item-/Blocksystem registriert werden; ein Rezept erfindet keine
neuen Itemtypen. Validierung begrenzt Rezepte, Mengen und Dateigroessen und
verwirft ein fehlerhaftes Erweiterungspaket atomar. Doppelte Namenraum-IDs oder
stabile Schluesselkollisionen sind Fehler, kein stilles Ueberschreiben.

Protokoll **19** uebertraegt den serverseitigen Katalog vor Ready. Client und
Server gemeinsam aktualisieren. Jeder Herstellungsvorgang verwendet Inventar-
Revision und die vorhandene geordnete Transaktionsqueue. Der Server prueft
Zutaten, Menge, Cursor/Platz sowie bei Werkbankrezepten den echten Block,
Reichweite (6 m ab Spielerauge) und freie Sicht. Erst die vollstaendig gelungene
Transaktion ersetzt das Inventar und erhoeht dessen Revision genau einmal.
Wiederholte/veraltete Pakete koennen keine Ressourcen duplizieren.

Neue Block-IDs: **28 Workbench**, **29 Chest**, ausschliesslich platzierte
Spezialobjekte. Eigene statische glTF-Modelle nutzen den bestehenden gebatchten
Custom-Mesh-Importer. Gewoehnliche Bloecke bleiben modellfrei im Cube-Mesher.
Label- und Root-Ladevorgaenge verwenden identische glTF-Loader-Einstellungen,
damit eingebettete Szenengeometrie nicht durch Material-Nachladen verschwindet.
Werkzeugmodelle bestehen aus wenigen wiederverwendbaren 3D-Quadern und liefern
Inventarvorschauen und Weltdrops; Namen/Beschreibungen stehen in
`assets/terra/items/tools.json`. Neue Itemvarianten sind an die vorhandene Enum
angehaengt; alte Inventare und Spielerdateien bleiben lesbar.

Werkzeuge beschleunigen passende Blockklassen: Holzspitzhacke 2x,
Steinwerkzeuge 4x. Glatter/Moosiger Stein benoetigt eine echte gehaltene
Spitzhacke; Stein bleibt fuer die erste Werkzeugherstellung von Hand erntbar.
Der Server ermittelt das Werkzeug aus dem besessenen ausgewaehlten Slot,
beendet Abbau bei Werkzeugwechsel und validiert Fortschritt/Ernte selbst.
Creative bleibt sofortiger Abbau. Werkzeughaltbarkeit und Containerinhalte
der bereits herstellbaren Truhe sind nicht Teil dieses Crafting-Schritts.

Slotmasse sind gegenueber Phase 44 um 25 Prozent vergroessert: Inventar 67.5 px
(responsive 60/52.5), HUD-Hotbar 61.25 px, Handcrafting 45 px (kompakt 40),
Rezept-/Zutatenslots 67.5 px. Die UI-Skalierung gilt weiterhin zusaetzlich.
Native Vulkan-Aufnahmen und reproduzierbare Pruefschritte stehen im
[Phase-45-Bericht](tools/benchmarks/results/crafting-2026-10-17/README.md).

### 3x3-Crafting Und Ausgerichtete Moebel (Phase 46)

Dieser Schritt ersetzt die rasterlose Werkbankansicht aus Phase 45 durch ein
echtes **3x3-Rezeptmuster**. Der Rezeptbrowser bleibt als Auswahl und automatische
Belegung mit vorhandenen Zutaten erhalten. Alternativ im Backpack eine Quelle
anklicken und anschliessend ein Rasterfeld; Rechtsklick leert ein Feld, der
Reset-Button das ganze Raster. Wie beim bestehenden 2x2 sind Rasterfelder
Referenzen auf echte Inventarslots, kein zweites Inventar und kein Item-Escrow.
Die Feldmengen zeigen die fuer das Rezept verwendeten Einheiten; der Backpack
den tatsaechlichen Bestand. Mehrfach verwendete Quellstacks werden aufsummiert.
Der Server prueft das tatsaechliche Muster, die benoetigte Gesamtmenge, Platz,
Cursor, Revision, Sicht und Werkbankentfernung vor dem atomaren Verbrauch.
Ein ungueltiges Muster erzeugt kein Ergebnis. Craft Max und Cursor bleiben
verfuegbar; das persoenliche 2x2 bleibt unveraendert nutzbar.

Grundrezepte liegen jetzt einzeln als `assets/crafting/<name>.json` vor;
`assets/terra/recipes/` wurde entfernt. Jede Datei enthaelt genau ein Recipe-
Objekt mit `id`, `name`, `station`, `pattern` und `output`, ohne Sammelliste.
Die JSON-Dateien sind gleichzeitig eingebetteter Fallback und die beim Start
neu geladenen Asset-Rezepte. Additive Packs unter `configs/recipes/` bleiben
unterstuetzt. Beispiel (Werkbank, oben ein Stein und darunter zwei Staebe):

```json
{
  "id": "my_mod::stone_shovel",
  "name": "Stone Shovel",
  "station": "workbench",
  "pattern": {
    "kind": "shaped", "width": 1, "height": 3,
    "cells": [
      {"item": {"block": 3}, "count": 1},
      {"item": "stick", "count": 1},
      {"item": "stick", "count": 1}
    ]
  },
  "output": {"item": {"tool": "stone_shovel"}, "count": 1}
}
```

Ebenso einzeln: `assets/terra/items/<item>.json`, `props/<kind>.json` und
`inventory/categories/<category>.json`. Die bisherigen tools-/items-/categories-
Sammeldateien sind entfernt. Normale Blockdefinitionen bleiben unveraendert
je Block unter `assets/terra/blocks/`. Sie enthalten nun gemeinsame Mechanik:

```json
"hardness": 1.25,
"resistance": 6.0,
"required_tool": "pickaxe",
"mining_level": 2
```

`hardness` ist die Abbaudauer vor Werkzeugbonus in Sekunden; `resistance`
der gespeicherte Widerstand fuer spaetere Explosions-/Schadensregeln (ein
Explosionssystem wird nicht vorgetaeuscht). `required_tool` ist null oder
pickaxe/axe/shovel, mining_level 0 frei, 1 Holz, 2 Stein. Falsche Klasse oder
zu niedrige Stufe verhindert Survival-Ernte; Creative bleibt sofortig.
`resistence` wird als Schreibweisen-Alias gelesen. Mechanik wird headless aus
denselben Blockdateien geladen, ohne Renderer-Registry oder GPU-Abhaengigkeit.
Der Server sendet die berechnete Abbaudauer fuer eine passende Animation.

Neue Werkbaenke (weiter ID 28) belegen genau zwei horizontale Zellen, in vier
90-Grad-Richtungen anhand der Spieler-Blickrichtung. Beide Zellen benoetigen
festen Boden, muessen frei und geladen sein und duerfen keinen Spieler schneiden.
Eine erfolgreiche Platzierung verbraucht eine Werkbank; der Abbau einer Haelfte
entfernt beide und liefert hoechstens ein Item. Bestehende alte Einzellen-
Werkbaenke werden nicht in Nachbarbauten vergroessert: fuer die neue Form neu
platzieren. Ausrichtung wird nicht von Kamera-Pitch oder Section-Vorzeichen bestimmt.

Truhen (weiter ID 29) sind einzeln schmal; zwei gleich ausgerichtete Exemplare
verbinden sich nur seitlich auf gleicher Hoehe. Kein vertikales oder dreifaches
Merging. Beim Entfernen einer Haelfte bleibt die andere als Einzeltruhe bestehen.
Peer, Facing und Teilflag stehen im vorhandenen SectionDelta/EditJournal,
einschliesslich Metadaten-Meshinvalidierung, Replikation und Regionsspeicherung.
Neue Regionen verwenden Record-Typ REG2; REGN wird explizit mit leerem
Objektzustand eingelesen. Weltformat 2, Generator 5, Seed und Block-IDs bleiben
erhalten. Dieser historische Phase-46-Stand verwendete **Protokoll 20**;
aktuell verwenden Client und Server gemeinsam Protokoll 27.

Modelle folgen der Referenz mit gewoelbtem Truhendeckel, Metallbaendern,
Messingschloss sowie Werkbank-Schublade, Ablage, Werkzeugstaender, Tuch,
Werkplan und Laterne. `tools/assets/build_furniture.mjs` erzeugt reproduzierbar
die glTF-Quelldateien. Nur die Primaerzelle emittiert das ausgerichtete breite
Modell, keine zweite ueberlagerte Haelfte oder einzelne Entity pro Detailvoxel.
Die Laterne besitzt ein emissives Material, noch keine eigene Punktlichtquelle.
Truheninhalt folgt in Phase 49; ein animierter Deckel bleibt offen.

Die neue generierte Holz-Albedotextur hat tatsaechlich **1254x1254 Pixel**,
ohne kuenstliches Upscaling. Materialmuster steigen von 64 auf 512 Pixel mit
zehn Mipstufen; PNGs erhalten weiterhin vollstaendige asynchrone Mipketten.
Anisotropie steigt von 4 auf 8. Nach aussen gewickelte, nicht degenerierte
Modellflaechen und eindeutige Terrainkontakte vermeiden fehlerhafte oder
ueberlagerte Objektflaechen. Das ist keine Zusage, dass jedes Schatten-/TAA-
Flimmern der gesamten Welt beseitigt ist. Hoehere Texturaufloesung kostet
zusaetzlichen GPU-Speicher; FPS-/Speicherparitaet wird nicht behauptet.

### Itembrowser Und Holzvarianten (Phase 47)

Die Inventaransicht wurde vereinheitlicht: Creative zeigt denselben Backpack
und dieselbe persoenliche 2x2-Fertigung wie Survival. Rechts steht bei geoeffnetem
Inventar oder Werkbank der gemeinsame Itembrowser mit Kategorien, Suche und
Seiten. Survival-Klicks oeffnen echte Rezepte des vom Server gelieferten Katalogs;
vorhandene Zutaten werden nur bei passender Station ins Raster eingeordnet.
Fehlende Zutaten bleiben als Rezeptvorschau sichtbar: leicht pulsierender roter
Slot und TiltUIs tatsaechlicher `animated-filter: signal-lost(...)`, kein
erfundenes Filter-Attribut. Rezeptalternativen und fehlende Rezepte werden
explizit angezeigt. Unpassende Werkbankrezepte erzeugen im 2x2 kein Ergebnis.

Rasterfelder reservieren nun konkrete Mengen aus vorhandenen Inventarslots
(`CraftGridCell`), statt unbeschraenkt auf deren gesamten Stack zuzugreifen.
Die reservierte Menge steht im Raster und wird in der Backpackanzeige abgezogen.
Es entsteht kein zweiter Itembestand; Schliessen oder Leeren gibt Reservierungen
sofort frei. Der Server prueft die Summe mehrfach verwendeter Quellen und das
Muster atomar. Mehr Fertigung erfolgt durch groessere Rasterstacks, nicht durch
Plus-/Minus-Schaltflaechen. Ergebnis anklicken fertigt zum Cursor, Shift-Klick
fertigt die aus den Rastermengen moegliche Anzahl ins Inventar. Creative-Klicks
im Browser geben ein Item, mit Shift einen Stack; freie Hotbarslots haben Vorrang.
Neue Inventartypen koennen dieselbe Browserkomponente im gemeinsamen Rahmen
verwenden. Dieser Phase-47-Rahmen allein war noch kein Container-Backend;
Truheninhalte folgen in Phase 49, das Ofensystem in Phase 87.

`struct_block: true` in einer Block-JSON verhindert die Umwandlung von Gras
unterhalb des platzierten Blocks in Dirt. Werkbank und Truhe verwenden das Flag.
Das Entfernen ueberlappender Props bleibt davon unabhaengig aktiv. Server und
Weltbearbeitung lesen dieselben `BlockProperties`; es gibt keine zweite Registry.

Werkbank, Truhe, Staebe und Holzspitzhacke verwenden gemeinsame Rezepte mit
`"wood": true` an Plankenzutaten. Gemischte Eichen-/Birken-/Fichtenbretter sind
erlaubt. Die verbrauchte Mehrheit bestimmt die Moebelholzart; bei Gleichstand
gilt deterministisch Eiche vor Birke vor Fichte. Eine angehaengte ItemKind-Variante
traegt die Holzart durch Inventar, Cursor, Drops, Platzierung und Replikation.
Bestehende Eichenmoebel behalten ihre Itemidentitaet. Die Primaer-/Sekundaerzellen
speichern die Holzart im selben Objektzustand; nur echte Holz-Materialteile der
gemeinsamen Modelle werden eingefaerbt. Metall, Tuch und Laterne bleiben gleich.
Bei einer aus unterschiedlich gefaerbten Einzeltruhen verbundenen Doppeltruhe
behaelt jede Haelfte ihre Farbe. Holzflaechen werden an der Mittelnaht mit
stetigen UVs geteilt, weiterhin im selben Materialbatch; Metall bleibt gemeinsam.
Beim Trennen bleibt jede urspruengliche Holzart erhalten.

Phase 47 schrieb `REG3`; explizite Leser fuer `REG2` und `REGN` uebernahmen
alte Weltstaende (fehlende Holzart wird Eiche). Phase 49 schrieb Regionen als
`REG4`; Phase 50 schreibt `REG5` und liest alle vier Vorstufen weiter. Spieler bleiben `PLY3`, Weltformat 2,
Generatorversion 5 und Block-IDs unveraendert. Client und Server verwenden
seit Phase 50 gemeinsam **Netzwerkprotokoll 24**.

Birkenlaub liegt farblich naeher am Eichengruen, behaelt aber weisses Holz und
eine eigene schmalere, asymmetrische Blattclump-Geometrie. Die existierenden
Near-/Middle-Batches und Windmaterialien bleiben erhalten; keine Einzelentities
fuer Blattdetails. Die erweiterten geteilten Vorlagen sind kein gemessener
FPS-/Speicherparitaetsnachweis.

### Grid-Mausbedienung Und Rezeptvorschau (Phase 48)

Das persoenliche 2x2 und das Werkbank-3x3 unterstuetzen gleichmaessiges
Linksziehen, Einzelablage beim Rechtsziehen, halbe Stapel, Sammelziehen,
Doppelklick-Sammeln, Shift-Ziehen zur Rueckgabe und einzelne Mausradtransfers.
Seit Phase 49 verwenden Itemtransfers Ctrl+Mausrad; normales Mausrad bleibt
dem Scrollen des jeweiligen Slotbereichs vorbehalten.
Waehrend Serverantworten ausstehen, werden weitere Grid-Besuche begrenzt
gepuffert. Das Raster bleibt eine Mengenreservierung auf dem bestehenden
Inventar, keine separate Itemablage. `TransferCursor` verschiebt exakte Mengen
mehrerer Quellen atomar und servervalidiert; ungueltige Quellen, fremde Items,
Ueberziehung oder Stackueberlauf veraendern keinen Teilbestand. Diese angehaengte
Netzwerknachricht wurde mit Protokoll 22 eingefuehrt; aktuell gilt Protokoll 27
auf Client und Server. Dieser Transfer erforderte keine Welt-/Spielerdateimigration
oder neuen IDs.

Ein Browserklick zeigt ausschliesslich die Rezeptvorschau. Erst das Plus im
Rezeptdialog reserviert vorhandene Zutaten bei passender Station und schliesst
das Fenster. Fehlende Zutaten, unpassende Stationen oder ein belegter Cursor
deaktivieren die Uebernahme. Ein einzelnes Rezept zeigt keine Seitennavigation.
Das Plus erhoeht keine Craftingmenge; diese bleibt durch Rasterstacks bestimmt.

Der gemeinsame Itembrowser steht am rechten Fensterrand mit 95 Prozent
Fensterhoehe und auf Desktopgroessen 100 UI-Pixel Abstand zum Inventar.
Kompakte Fenster reduzieren den Abstand und die Slotgroessen; sehr schmale
Ansichten bleiben scrollbar. UI-Skalierung gilt wie fuer die uebrigen Menues.
Werkbankfuesse und Truhenboden liegen jetzt exakt auf Y=0 der belegten Zelle,
statt 0.04 bzw. 0.065 Bloecke darueber. Ausrichtung, Zweizellen-Fussabdruck,
Holzfarben und die gemeinsame Custom-Mesh-/Picking-Pipeline bleiben erhalten.

### Truhen Und Inventarraum (Phase 49)

Die vorhandene Truhe (Material-ID 29) verwendet jetzt dasselbe `Inventory` wie
Spieler, nicht einen zweiten Itembestand. Jede physische Truhe besitzt 18 Slots
(2x9); verbundene Doppeltruhen zeigen die beiden unabhaengigen Haelften gemeinsam
als 36 Slots (4x9). Das persoenliche Inventar bleibt bei 27 Slots einschliesslich
Hotbar. Cursor und Stacklimit 128 bleiben unveraendert.

`terra_core::containers::apply_gesture` verarbeitet Klick, Halbieren, Drag-Verteilung,
Shift-Transfer, Sammeln und Ctrl+Mausrad atomar ueber Spieler und eine oder zwei Haelften.
Normales Mausrad scrollt, ohne dabei Items aus dem sichtbaren Slot zu verschieben.
Virtuelle Indizes 0..26 gehoeren dem Spieler, 27..44 der ersten und 45..62 der
zweiten Truhe. Ungueltige Indizes, doppelte Drag-Ziele, fremde Sekundaer-Swaps,
Stackueberlauf, volle Ziele und Revisionsueberlauf erzeugen keine Teiltransfers.
Nur der Spieler besitzt einen Cursor; jede geaenderte Inventarrevision steigt
pro Geste genau einmal.

Das bestehende Lightyear-Control-Protokoll ergaenzt private Containeranfragen und
Snapshots. Der Server prueft Reichweite, Sichtlinie, lebenden Spieler, aufgezeichnete
Truhenzellen, symmetrische Verbindung sowie Spieler- und Haelftenrevisionen.
Ein entfernter, verdeckter, zerstoerter oder umverbundener Container schliesst die
Ansicht. Gleichzeitige Zugriffe werden serverseitig serialisiert: veraltete Revisionen
erhalten einen Konflikt und den aktuellen Inhalt. Andere berechtigte Betrachter
erhalten aktualisierte Snapshots; Inhalte stehen niemals in oeffentlichen
`SectionDelta`-Nachrichten.

`EditJournal::commit_container_transaction` prueft alle Ergebnisse vor dem
gemeinsamen Commit. Spieler und Haelften werden zusammen gespeichert; die globale
Gameplayrevision steigt einmal. Reine Inhaltstransfers erzeugen weder Terrain-,
Kollisions- noch Wasser-Meshinvalidierungen. Nichtleere physische Truhen lassen
sich nicht entfernen oder ersetzen. Eine leere Haelfte kann von einer gefuellten
Doppeltruhe getrennt werden; deren andere Haelfte behaelt ihren Inhalt und ihre
Holzart. Der Schutz gilt auch fuer interne `commit_block_edit`-Aufrufe.

Regionen speichern Containerlisten nach globaler Zellposition im bestehenden
atomaren Mehrdatei-Commit als `REG4`. Sortierte Positionen machen Ausgaben
deterministisch. Laden prueft Koordinaten, Duplikate, Regionszugehoerigkeit,
aufgezeichnete Truhen und gueltige cursorfreie Inhalte mit hoechstens 18 Slots.
`REG3`, `REG2` und `REGN` werden explizit ohne alte Containerinhalte uebernommen.
Spielerdateien bleiben `PLY3`; Weltformat 2, Generator 5, Seeds und bestehende IDs
bleiben gleich. Phase 49 verwendete Protokoll 23 und Phase 50 Protokoll 24;
seit Phase 53 benoetigen Client und Dedicated Server gemeinsam Protokoll 27.
Dedicated-Autosave und Integrated-Server-Snapshots verwenden denselben Journalzustand.
Zukuenftige Dimensionen erhalten noch keinen neuen Speicherpfad oder eigene Simulation.

Der TiltUI-Inventarrahmen bleibt unabhaengig vom Itembrowser im Fenster zentriert.
Der Browser nutzt rechts nach 100 logischen UI-Pixeln Abstand die verbleibende
Breite und 95 Prozent Fensterhoehe. Kleine Ansichten reduzieren den Abstand;
sehr schmale Fenster stapeln beide Bereiche scrollbar. Spieler-, Truhen-, Werkbank-
und Rezeptbereiche scrollen, ohne ihre quadratischen Slots zusammenzudruecken.
Seitenzaehler stehen zwischen ihren Pfeilen; Rezeptnavigation erscheint nur bei
mehreren Alternativen. Rezeptuebernahme bleibt ausdruecklich am Plus gebunden.

Links neben dem Charakter stehen acht abgeblendete Ausruestungspositionen in
zwei Spalten: Helm/Ohrstecker, Brustpanzer/Kette, Hose/Guertel, Schuhe/Socken.
Diese zeigen Typ-Silhouetten und Tooltips, sind aber bewusst deaktiviert:
Ausruestungsitems, passende persistente Slots und Werteboni sind noch offen.
Ein echtes Ausruestungssystem bleibt offen; das Ofen-Backend folgt in Phase 87.

Geformte Rezepte pruefen jetzt den vollstaendigen Fussabdruck auch rechts und
unten; eine einzelne Ecke kann keine abgeschnittene Werkbank ergeben.
Verschiebung im passenden 2x2-/3x3-Raster bleibt erlaubt, innere Musterluecken
bleiben erforderlich. Leere aeussere Musterreihen/-spalten werden als ungueltig
abgewiesen. Shapeless-Zutaten erhalten eine begrenzte, reihenfolgeunabhaengige
Zuordnung auch bei gemischten generischen Holzgruppen und exakten Arten.
Reserviertes Crafting verbraucht exakt die validierte Zellzuordnung, nicht eine
neue Zuordnung gegen groessere Gesamtstacks. Truhe, Werkbank und Werkzeuge bleiben
`shaped` in ihren vorhandenen `assets/crafting/*.json`-Dateien.

Die Zweizellen-Werkbank besitzt von beiden Zellen aus eine gemeinsame Auswahlbox;
verbundene Truhen bleiben getrennt waehlbare physische Objekte. Der Abbaueffekt
importierter statischer Modelle zerlegt ihre wirklichen Oberflaechendreiecke in
budgetierte Teile, statt einen ausgefuellten Wuerfel einzublenden. UVs, Materialfarben
und Modellluecken bleiben erhalten. Die Vorbereitung erfolgt asynchron und nur
fuer das aktive Nahziel; das ist keine neue Voxelaufloesung oder Gameplay-Physik.

**Abnahme Phase 49:** Container-UDP, REG4-Reload und native Crafting-/Mausgesten
sind geprueft; Werkbank-/Truhenmodellabbau bestanden mit vier Bruchstufen,
Wiederherstellung nach Abbruch und ohne Wuerfeleffekte. Headless-Start mit 27 Sections
und Server-Abhaengigkeitsbaum ohne Renderer/UI bestaetigt. Alle fuenf
Workspace-Pruefungen bestanden; 781 Tests erfolgreich, ein bestehender
Langzeittest ignoriert (`--test-threads=2`). Eine zuvor gefundene Race-Condition
in der Survival-Testfixture wurde durch korrektes Warten auf private Inventardaten
behoben, ohne Produktionsregeln zu aendern. Die abschliessende native Inventarprobe
erzeugte sieben Aufnahmen und pruefte erzwungenes Scrollen, erreichbare letzte
Slotreihe und erhaltene Itembestaende mit echter OS-Maus. Silhouetten, Zentrierung,
Browserabstand und Pagination sind nativ gesichtet. Dieser Schritt enthaelt
keinen neuen FPS-, GPU-Speicher- oder Langzeitparitaetsnachweis.
[Logs und Nachweise](tools/benchmarks/results/containers-2026-10-17/README.md).

### Inventarbedienung, Verbundene Werkbaenke Und Oekologie (Phase 50)

Mouse-Tweaks zeigen bereits waehrend des Ziehens die entstehenden Slotstacks
und den verbleibenden Cursorbestand. Diese Vorschau verwendet dieselben
Verteilungsregeln wie der autoritative Commit, veraendert aber keine echten
Inventare vor der bestaetigten Aktion. Das gilt fuer Spieler-, Crafting- und
Truhenslots. Der Crafting-Reset hat keinen sichtbaren Rahmen mehr; Status-
und Fehlertexte erscheinen neutral statt als auffaellige rote/gelbe Warnungen.
Fehlende Rezeptzutaten bleiben fachlich erkennbar. Der rote Schadensschein
bleibt erhalten; unter 30 Prozent Gesundheit kommt der echte TiltUI-
`signal-lost`-Filter hinzu und verschwindet bei ausreichender Heilung.

Eine Doppeltruhe bekommt eine hoehere Ansicht, damit ihre 36 Slots auf
normalen Desktopfenstern vollstaendig sichtbar sind. Kleine Fenster behalten
den Scroll-Fallback statt Slots abzuschneiden. Die Werkbank kann links den
Inhalt genau einer angrenzenden Einzel-/Doppeltruhe zeigen. Beide Werkbankzellen
werden fuer horizontale Nachbarschaft geprueft. Entscheidend ist die zuerst
platzierte noch vorhandene angrenzende Truhe, nicht eine beliebige HashMap-
Reihenfolge oder die zuletzt geoeffnete Kiste. Eine Trennung oder Entfernung
schliesst veraltete Ansichten; erneutes Oeffnen waehlt den aktuell berechtigten
Verbund. Reichweite/Sichtlinie beziehen sich auf die Werkbank; private Inhalte
gehen weiterhin nur an berechtigte Betrachter.

`ContainerSnapshot::workbench` kennzeichnet diesen Stationskontext. Uebertragungen
respektieren die im Craftingraster reservierten Zutaten atomar, sodass verschobene
oder nur teilweise verfuegbare Stacks nicht doppelt verbraucht werden. Die
Werkbank selbst wird mit vier Brettern `shapeless` hergestellt; diese vier
Zutaten muessen im Craftingraster vorhanden sein, auch ein 2x2-Quadrat funktioniert.
Truhen und Werkzeuge bleiben `shaped`. Gemischte Holzarten verwenden weiterhin
die mengenmaessig dominierende Holzfarbe.

Die bestaendige Platzierungsreihenfolge ist private Journal-Metadaten und wird
mit Regionen als `REG5` gespeichert, nicht an normale Section-Betrachter gesendet.
Bestehende `REG4`, `REG3`, `REG2` und `REGN` bleiben lesbar. Alte Regionen hatten
keine historische Reihenfolge: Fuer diese alten Moebel gilt einmalig eine
stabile Koordinatenreihenfolge, keine erfundene Rekonstruktion des Bauzeitpunkts.
Faerben/Verbinden erhaelt die Reihenfolge; wirkliches Ersetzen erzeugt einen
neuen Eintrag. Spieler bleiben `PLY3`, Generator 5 und Weltformat 2; IDs und
Seed werden nicht geaendert. Client und Server verwenden gemeinsam Protokoll 24.

Truhen und Werkbaenke verwenden fuer Auswahl und Sichtlinien die aus denselben
Modellbausteinen exportierten Teilvolumen statt ganzer Zellwuerfel. Datengetriebene
`collision_boxes` folgen den mit denselben Asset-Werkzeugen erzeugten Moebelmodellen;
Avian-Terrainkollision beruecksichtigt deren Form ebenfalls. Leere Zwischenraeume koennen durchblickt
werden. Normale volle Cube-Bloecke behalten ihre vollstaendige Form. Bruchstuecke
entstehen auf den tatsaechlichen Oberflaechen; beim Abbau brechen kleine gebatchte
Teilchen heraus. Das Grasblock-Vorschaumodell erhaelt seine gruene Oberseite,
ohne die neutrale, im Terrain biomgefaerbte Textur zu ersetzen.
Waehrend des Abbaus bleibt die urspruengliche volle Modell-Auswahlkontur verborgen,
damit sie herausgebrochene Bereiche nicht als scheinbar intaktes Gitter zeigt.
Physische Sections erhalten ihren jeweils geclippten Anteil verbundener
Moebel, sodass eine zuerst geladene zweite Haelfte nicht vom Collider ihrer
noch fehlenden Nachbarsection abhaengt. Modellteile werden innerhalb derselben
Section nicht durch beide Objektzellen doppelt eingefuegt. Lose Items und
ihre Pickup-Sichtlinien verwenden denselben teilvolumenbewussten Weltkontext.

Exponierte Dirt-Oberflaechen nahe einem Grasblock werden auf derselben
autoritativen Weltseite wie Integrated-/Dedicated-Spielregeln begruent.
Nach 8-16 Sekunden erscheinen vier sichtbare Entwicklungsstufen mit jeweils
etwa acht Sekunden Abstand; unter geringer Last dauert ein Zyklus rund 40-48
Sekunden. Der Abschluss schreibt Dirt -> Grass ins bestehende Blockjournal.
Feuer, starke Verkohlung, Schnee und ueberflutete oder voll verdeckte Oberseiten
stoppen Wachstum; `struct_block`-Objekte ersticken die Oberflaeche nicht.
Unbeladene Zellen pausieren und setzen gespeicherte Entwicklungsstufen nach
dem Reload fort. Nicht erledigte Startwartezeit wird nicht als absolute
Weltuhr gespeichert; beim Reload wird sie neu angesetzt.

Die kleinen, nicht erntbaren Wachstumsstufen nutzen vorhandene `WorldProp`-
Datensaetze und Section-Batches, keine weitere Persistenz oder Entity pro Halm.
Sie belegen keine der vier Gameplay-Pflanzenpositionen und verschieben vorhandene
Blumen nicht. Wenn bereits ein Grasbueschel auf Dirt steht, speichert eine
unsichtbare Fortschrittsvariante nur den Zyklus, statt es sichtbar zu duplizieren.
Varianten 64..67 sind sichtbare Spruesslinge, 68..71 reine Fortschrittsmarker.
Budget: maximal 256 aktive Zellen, acht Aenderungen je Sekunde, zwei
Discovery-Sections je Sekunde und hoechstens 4096 wartende Edit-Kandidaten.
Zwischenstufen aendern nur Prop-Epochen; erst der Materialabschluss remesht Terrain.

Grosse Pilzkappen tragen nun unregelmaessige, verbundene Myzelbaender unter
der sichtbaren Unterseite. Die Baender sind 0.35-1.30 Meter lang und werden in
bestehenden Near-/Middle-Materialbatches mit begrenzter Segmentzahl dargestellt,
nicht als einzelne Entities. Eiche, Birke, Herbstbaeume und Fichten erhalten
ein deterministisches 24-Prozent-Ticket fuer kleine Stammpilzcluster in Rot mit
hellen Tupfen, Braun oder Weissgrau. Freier Platz und tatsaechlich vorhandenes
Stammholz bleiben Voraussetzung; Palmen und Grosspilze erhalten keine solchen
Stammcluster. Stabile IDs, Modell-Picking, Einzelabbau und gespeicherte Entfernung
nutzen das vorhandene Prop-System. Supportverlust und verbauende Nachbarbloecke
entfernen nur betroffene Cluster, auch ueber Section-Grenzen. Kandidatensuchen
werden einmal im geteilten `PreparedColumn` vorbereitet und vertikal wiederverwendet.

Die opt-in Probe `--smoke-test-ecology` zeigt echte generierte Stammpilze,
Kappenunterseiten und einen ueber `BlockEditRequest` gestarteten vollstaendigen
Echtzeit-Wachstumszyklus. Sie ersetzt keine Multiplayer- oder Langzeitlastabnahme.
Alle fuenf Cargo-Pruefungen und 816 regulaere Tests bestanden; ein bestehender
Langzeittest bleibt ignoriert (abschliessender Testlauf mit `--test-threads=1`).
Native OS-Inventargestik, Crafting/Stationslager,
REG5-Reload, kritische Gesundheit/Respawn und der Oekologiezyklus mit sechs
GPU-Aufnahmen sind bestanden und relevante Ansichten gesichtet. Der Dedicated
startete ohne Fenster mit 27 Sections; sein Abhaengigkeitsbaum bleibt renderer-/UI-frei.
Die beiden abschliessenden Modellabbauproben bestanden mit vier Bruchstufen,
Wiederherstellung nach Abbruch und einer strengeren Positivassertion fuer
tatsaechliche Bruchteilchen (Werkbank vier, Truhe zehn). Kein neuer allgemeiner
FPS-/Speicher- oder Langzeitparitaetsnachweis. Details und konkrete Grenzen:
[Phase-50-Bericht](tools/benchmarks/results/ecology-inventory-2026-10-17/README.md).

### Lokale Testprofile, Mineralien Und Schattenkosten (Phase 53)

Zwei Clients auf demselben Rechner verwenden unabhaengige persistente Profile:

```bash
cargo build --locked --workspace
target/debug/terra-client --profile tester-a --connect 127.0.0.1:5000
target/debug/terra-client --profile tester-b --connect 127.0.0.1:5000
```

Die beiden Clientbefehle in getrennten Terminals starten. Ohne `--profile` bleibt
`configs/profile.toml` unveraendert der Standard. Benannte Profile liegen unter
`configs/profiles/NAME.toml`; erlaubt sind 1..32 ASCII-Buchstaben, Ziffern,
Unterstriche und Bindestriche mit alphanumerischem Anfang. Der vorhandene
Override `TERRA_PROFILE_FILE` bleibt erhalten, darf aber nicht gleichzeitig mit
`--profile` gesetzt sein. Serverpruefung und Ablehnung derselben UUID bleiben
aktiv. Lokale Profile sind keine verifizierten Online-Accounts.

Seit Phase 65 waehlt `--gender=f` beziehungsweise `--gender=m` die weibliche
oder maennliche Avatarvariante. Die Auswahl wird im gewaehlten Profil gespeichert;
ohne Argument bleibt dessen Auswahl erhalten, alte Profile starten maennlich.
Beispiel: `cargo run --locked -p terra-client -- --profile tester-a --gender=f`.
Die UUID bleibt dabei gleich. Client und Server muessen gemeinsam Protokoll 38
verwenden, damit andere Spieler dieselbe Avatarvariante sehen.

Stein und die sechs Erz-Felskerne teilen ein neues mattes 1254er-Bitmap-Albedo
mit vollstaendigen asynchronen Mips und dem bestehenden anisotropen Sampler.
[Asset und Generierungsprompt](assets/terra/blocks/textures/stone-strata-hd.md).
Erzfaces erhalten verzweigte dunkle Bruchbetten mit hellen Mineraladern, im selben
Section-/Materialbatch. Die kanonischen Dreiecke werden auch fuer Picking
verwendet. Der tragende Wuerfel bleibt geschlossen. Das ist sichtbares
Mineralrelief, keine durchbrochene Felswand oder lichtemittierende Erzquelle.

Neue Welten speichern jetzt `geology_revision = 2`. Dripstonekammern erhalten
deterministische, mehrblockige verjuengte Boden-/Deckenformationen mit geprueftem
Support. Deskriptoren werden pro horizontalem Bereich begrenzt gecacht, nicht
pro Voxel mit einer zweiten Noise-Pipeline neu berechnet. Salzformationen nutzen
den angehaengten Block `teramic::salt_formation` (38), einen hellen mineralischen
Koerper; beim Abbau faellt die bestehende Salzressource an. Der normale
Tropfsteinkoerper `teramic::dripstone_formation` (39) teilt sein Material mit der
Wandbeschichtung (37), besitzt aber die breiten Formationsspitzen statt deren
kleinen, sparsamen Anbauten. Normale Kollision,
Voxelabbau, Regiondateien und Replikation bleiben zustaendig. Keine Entity pro
Tropfstein und keine neue GLB. Alte Welten mit Vertrag 0/1 bleiben unveraendert;
kein automatischer Retrogen. Generator 5, Weltformat 2 und IDs 0..37 bleiben
erhalten. Client und Server benoetigen gemeinsam Protokoll 27.

Nahe Cube-Schatten nutzen separate grobe Section-Schalen statt jeder winzigen
Relief-/Blattfacette. Opaque Schalen teilen einen positionsbasierten Batch pro
Section; Laub behaelt eigene Cutout-/Windmaterialien. Sichtbare Geometrie und
Picking werden nicht vereinfacht. Glas/Wasser und Custom-Meshes bleiben aus
diesem Ersatzpfad ausgenommen. Schattenproxies sind von Farbpass/Fade-Feedback
getrennt; die vorhandene Sichtbarkeitslogik wartet nicht auf einen
unsichtbaren Proxy-Draw. Leere Terrain-/Prop-Uploadqueues scannen unveraenderte
geladene Sections nicht erneut.

Konkrete Messwerte, Speicher-/Ladezeitkosten und native Nachweise stehen im
[Phase-53-Bericht](tools/benchmarks/results/minerals-performance-2026-10-17/README.md).
Kein Versprechen allgemeiner 100 FPS oder identischer Speicherlast. Sehr grosse
Streaminglasten, weitere GPUs und fallende Tropfsteinphysik bleiben offen.

### Cliffted Desert, Hoehlentypen Und Renderkosten (Phase 52)

Neue Welten speichern `geology_revision = 1`; fehlende Felder alter Welten bleiben
Revision 0. Bestehendes Terrain und gespeicherte Edits werden nicht neu generiert.
Generator 5, Weltformat 2 und vorhandene IDs bleiben erhalten. Client und Server
verwenden gemeinsam Protokoll 26. Ein neues Spiel ist fuer die neue Geologie und
natuerliche Fichten mit dem neuen Stamm erforderlich.

`Cliffted Desert` entsteht nur innerhalb starker Desert-Klimaregionen, mit
normalem Desert-Saum, Sand und tieferem Sandstein unter den Mesakanten.
`/locate biome cliffted_desert` und die Debuganzeige kennen das Biom. Drei Formen
erweitern das gemeinsame Hoehlenfeld: grosse Cheese-Kammern, breite Dripstone-
Gewoelbe und verbundene Spaghetti-Gaenge. Zehn Prozent geeigneter 192-m-Regionen
im Cliffted Desert erhalten deterministisch eine besonders breite Salzkammer;
ausserhalb dieses Bioms entsteht im neuen Vertrag kein Salz. Salz-/Tropfstein-
Spitzen sind facettierte, gebatchte Anbauten ihrer tragenden Voxel, keine separaten
Entities oder fallenden Gameplay-Bloecke.

`teramic::spruce_log` (36) besitzt eigene dunkle Rinde und Stirnholz;
`assets/crafting/spruce_planks.json` verbraucht genau diesen Stamm.
`teramic::dripstone` (37) ergaenzt matte mineralische Hoehlenwaende. IDs 0..35
bleiben unveraendert. Stein und Erz-Felskerne teilen gebrochene, waermere
Gesteinskoerner und ein mattes Material. `cube_material.reflectance` (0..1,
kompatibler Default 0.5) ist Teil des gemeinsamen Material-Cache-Schluessels;
Stein nutzt 0.08 bei Roughness 0.99. Das ersetzt keine neutrale Hoehlenbeleuchtung:
kaltes Umgebungslicht kann weiterhin Schatten blau faerben.

Bevys Zweiphasen-GPU-Occlusion-Culling verwendet ab Sichtweite 12 den vorhandenen
Depth-Prepass. Kleinere Sichtweiten behalten den einphasigen Pfad, da dort die
zusaetzliche Verarbeitung kaum Geometrie verwirft. Aenderungen der Sichtweite
stellen die bestehende Kamera um, ohne neue Kameras oder Renderer zu erzeugen.
Fernes Gras behaelt seine Pflanzen, Wurzeln und flachen Spitzen mit 32 statt
80 Vertices; ferne Prop-Normalen/Farben werden mit dem vorhandenen Packformat
hochgeladen. Regulaere Fernvertices sinken von 48 auf 28 Bytes. Erzdetails
benoetigen maximal 52 statt 104 Dreiecke pro freiem Face. Nah-/Mittelgras,
Prop-Identitaet, Interaktion und Sichtweiten bleiben erhalten. Neue Salzwaende
nutzen bereits berechnete Kammerdichten statt eines dichten zweiten Salzrasters.

Messbedingungen, Rohlogs, native Aufnahmen und Grenzen stehen im
[Phase-52-Abnahmebericht](tools/benchmarks/results/cliffted-caves-2026-10-17/README.md).

### Erze, Kristalle Und UI-Feinschliff (Phase 51)

Alle fuenf Cargo-Pruefungen mit 845 Workspace-Tests, native Erz-/Oekologie-/
Survival-/Crafting-/Inventarproben und Headless-Abnahme sind bestanden. Ein
vorhandener 180-s-UDP-Soak bleibt ignoriert. Die nativen Laeufe speicherten
58 GPU-Aufnahmen; wichtige Tooltip-, Slot- und Schadensansichten wurden gesichtet.
CPU-Messpaare und konkrete Nachweise sind im
[Phase-51-Bericht](tools/benchmarks/results/ores-growth-2026-10-17/README.md)
dokumentiert. Der abgeschlossene Erzschritt steht in PATCH; naechster TODO-Punkt
ist die Tier-KI. Zusaetzliche GPU-/Speicher- und Langzeitmessungen bleiben im Backlog.

Die bestehenden Block-IDs bleiben unveraendert. Neue normale Cube-Materialien:
Salz 30, Eisen 31, Kohle 32, Gold 33, Diamant 34 und Rubin 35. Ihre einzelnen
JSONs liegen unter `assets/terra/blocks/*_ore.json`; `crystal_material` ordnet
den Detailanteil einem Material derselben Registry zu. Es gibt keine Erz-GLBs
und keine Einzelentity pro Kristall. Geschlossene Felskerne und facettierte
Einschluesse nutzen das gemeinsame Section-/Materialmeshing. Detailspitzen werden
dem tragenden Erzblock zugeordnet; sie sind keine zusaetzlichen Gameplay-Zellen.
Rendering und Praezisionsauswahl teilen deterministische Formen aus
`terra-world::ore_shapes`. Grobe LODs behalten den primaeren Erztyp.

Die Generierung ersetzt ausschliesslich geeigneten natuerlichen Stein nach dem
vorhandenen Terrain-/Hoehlenaufbau. Sparse Lagerstaetten werden nach Weltseed und
globalen Suchzellen bestimmt, einschliesslich negativer Koordinaten und Nachbar-
Sections. Gespeicherte Blockaenderungen werden anschliessend wie bisher angewendet.
Tiefe ist der Abstand unter der jeweiligen Oberflaeche, nicht der absolute Y-Wert.
Salzregionen reichern Waende vorhandener Hoehlen geologisch an; sie erzeugen keinen
weiteren Hoehlenalgorithmus und keine garantierte Kammer an jeder Suchkoordinate.

Neue Welten speichern `ores.enabled = true` und den separaten Featurestand
`ores.revision = 1`. Alte Weltkonfigurationen ohne `ores` bleiben bei `false/0`.
`enabled = true` ohne nativen Featurestand wird abgewiesen: keine unbemerkte
Nachgenerierung in alten Welten und kein Eingriff in Spielerbauten. Weltformat 2,
Generator 5, Seed und Welt-ID bleiben erhalten; REG5 und PLY3 werden weiterverwendet.
Client und Server benoetigen wegen der angehaengten Item-/Werkzeugvarianten
gemeinsam **Protokoll 25**. Ein Erz-Retrogen fuer alte Welten ist nicht implementiert.

`WorldConfig::ores` speichert je Art `min_depth`, `max_depth`, `frequency`, `radius`
und `cave_affinity`. Konfiguration fuer eine neue Welt, als Ausschnitt aus
`configs/world.toml`:

```toml
[ores]
enabled = true
revision = 1

[ores.iron]
min_depth = 8.0
max_depth = 240.0
frequency = 0.85
radius = 4.2
cave_affinity = 0.28
```

Frequenz ist die Chance pro Lagerstaetten-Suchzelle, nicht der Erzanteil aller
Steinbloecke. Der Suchraster ist fuer Salz 10 m, fuer andere Arten 32 m.
Gueltige Tiefen liegen zwischen 3 und 1024 m mit `max_depth > min_depth`, Radius
zwischen 0.5 und 8 m, Frequenz und Hoehlenaffinitaet zwischen 0 und 1. Salz bleibt
immer an Hoehlenwaenden; seine Affinitaet bestimmt die Enge des Wandbands.
Explizite native Einstellungen werden nicht stillschweigend ersetzt. Fehlt der
neue Parameter in einer alten globalen Dedicated-Konfiguration, uebernimmt der
Startpfad den Erzvertrag des vorhandenen Saves; ein wirklich neuer Dedicated-
Save erhaelt den nativen Stand. Dadurch deaktiviert ein Neustart nicht die Erze
einer bereits angelegten neuen Welt. Fuer eine neue Welt ohne Erze ist
`enabled = false, revision = 1` die ausdrueckliche Einstellung.

Salzkristalle, Roheisen, Kohle, Rohgold, Diamanten, Rubine sowie Eisen-/Goldbarren
sind eigenstaendige Ressourcen mit JSON-Metadaten unter `assets/terra/items/`.
Salz/Kohle benoetigen eine Holzspitzhacke, Eisen/Gold mindestens Stein,
Diamant/Rubin mindestens die neue Eisenspitzhacke. Creative bleibt unmittelbar.
Die bestehenden autoritativen Abbau-, Drop-, Inventar- und Speicherpfade gelten
auch fuer diese Items. Historisch ergaben zwei Roherz plus eine Kohle an der
Werkbank einen entsprechenden Barren. Phase 87 ersetzt diese provisorischen
shapeless-Rezepte durch den autoritativen Ofen mit Brennstofftimer.
Die Eisenspitzhacke bleibt shaped.

Verknuepfte Werkbanktruhen verwenden dieselben 66/53/46/36/31 logischen Slotpixel
wie das normale Inventar. Links wechseln sechs beziehungsweise drei Spalten nach
verfuegbarem Platz; sehr schmale Fenster stapeln den Bereich mit neun Spalten.
Tooltips haben keine Scrollflaechen, brechen lange IDs bei Bedarf zwischen Zeichen
um und bleiben ausserhalb von Inventar-Scrollclips sichtbar. Ihre Breite wird am
Viewport begrenzt. Sehr lange benutzerdefinierte Texte koennen dennoch mehr Hoehe
als ein winziges Fenster benoetigen; eine unverkuerzte Anzeige beliebig langer
Texte auf jeder Aufloesung wird nicht behauptet.

Schaden erzeugt vier sanft auslaufende rote Randlichter statt einer roten
Vollbildflaeche. Kritische Gesundheit unter 30 Prozent kombiniert pulsierende
Randlichter mit dem vorhandenen TiltUI-Stoerfilter; Heilung/Respawn/Creative
beenden den kritischen Zustand. Die Bildmitte bleibt vom roten Overlay frei.
Haengende Pilzfasern reichen nun bis etwa 2.5 m und bewegen sich mit verankerter
Wurzel im vorhandenen Windpfad. Neue Grasstadien skalieren ueber 2.4 Sekunden
weich aus der Wurzel; der autoritative Wachstumszyklus und seine gespeicherten
Stadien bleiben unveraendert. Ein erstes Laden gespeicherter Stadien startet
die Animation nicht von vorn. Es gibt weiterhin keine Entity pro Faser/Halm.
Ein aussergewoehnlich lange laufender Mesh-Job kann einen alten Interpolations-
Snapshot spaeter wieder aufnehmen; normale achtsekundige Stadien liegen deutlich
ueber der 2.4-s-Ueberblendung. Kleine Motion-Vector-Abweichungen waehrend dieser
Skalierung sind noch moeglich; die native Normalprobe ersetzt keinen Langzeitnachweis.

CPU-Worker-Mediane bei 4375 Sections, vier Workern und fuenf Messungen im gleichen
optimierten Dev-Build: Erzfeature aus/an an der Oberflaeche 0.257071/0.298418 s
Headless (+16.08%) und 0.464425/0.520461 s mit Render-Vorbereitung (+12.07%).
Im Untergrund 0.433616/0.477393 s (+10.10%) beziehungsweise 0.808082/0.885329 s
(+9.56%). Das sind zusaetzliche Featurekosten, kein historischer Buildvergleich,
keine FPS und keine isolierte Meshing-/GPU-/Speichermessung. Rohdaten und Grenzen
stehen im Phase-51-Bericht; allgemeine Performanceparitaet wird nicht behauptet.

### Kristallgruppen Und Salzspitzen (Phase 54)

Die bestehende Erzgeometrie verwendet breite kantige Kristallgruppen und
kleinere Mineraleinschluesse. Gemeinsame PBR-Materialien, Section-Batches,
kanonisches Dreiecks-Picking und drei Detailstufen bleiben bestehen. Die
geschlossenen Traeger-Cubes erhalten keine Sichtloecher. Mineral-UVs richten
sich nach der Facette; das gemeinsame Mip-Texturmuster bleibt auch an langen
Seiten lesbar. Die vorhandene blaue Hoehlenbeleuchtung beeinflusst weiterhin
die wahrgenommene Farbe; kein neues Licht- oder Emissionssystem.

Grosse Salzcluster sind am Boden mindestens 2 m hoch und haengen von Decken
hoechstens 10 m herunter. Ihre sparse Freiraumbeschreibung entsteht auf den
Generierungs-Workern und bleibt im vorhandenen Meshcache. Nur Salz-Sections
beziehen benachbarte vertikale Blockrevisionen in die Render-Gueltigkeit ein.
Nach Blockaenderungen werden die betroffenen Spitzen auf dem Worker erneut
gegen den Journal-Snapshot geprueft; Physik und normale Terrainsections werden
nicht wegen dieser dekorativen Clearance neu generiert. Auch Mining-Isolation
erhaelt die lange Geometrie. Die Arme sind anklickbare Dekoration am Traeger,
keine separaten kollidierenden oder fallenden Physikkoerper.

Neue Welten speichern Geologievertrag 3, der massive Salzkoerper durch diese
begrenzten Gruppen ersetzt. Alte Welten 0/1/2 bleiben in ihrer gespeicherten
Geologie; fuer die neue Gesamtform eine neue Welt erstellen. Normale
Tropfsteinformationen, Generator 5, Seeds, IDs und binaere Speicherformate
bleiben erhalten. Client und Server gemeinsam auf Protokoll 28 aktualisieren.
[Mess- und Abnahmebericht](tools/benchmarks/results/crystal-clusters-2026-10-17/README.md).

### Tiere, Tropfstein Und Chatverlauf (Phase 55)

Schafe, Kuehe, Schweine und Fische verwenden eine gemeinsame serverseitige
Population mit stabilen Habitat-IDs, lokaler Voxel-Navigation und pausierter
Fernsimulation. Pro 100-ms-Takt werden maximal 128 Tiere aktualisiert und zwei
Spawn-Habitate untersucht. Je Client werden hoechstens 128 Tiere in 80 m Umkreis
repliziert; geteilte Meshes und interpolierte, gegliederte Modelle stellen sie dar.
Fische pruefen echtes Zellwasser, Stroemung und Wasserverlust. Linksklick trifft,
Rechtsklick fuettert verletzte Landtiere mit Gras beziehungsweise Pilzen.
Schaden, Ausdauer, 3D-Drops, Nahrung und Wolle nutzen die vorhandenen Spielsysteme.
Prozedurale Tierlaute teilen einen Audiostream und die Master-/Ambience-Regler.

REG6 speichert auch leere Spawn-Habitate und tote Tieridentitaeten, sodass Reload
oder Late Join keine neuen Tiere duplizieren. Alte Regionsformate bleiben lesbar;
PLY3 und Generator 5 bleiben erhalten. Client und Server gemeinsam auf Protokoll
29 aktualisieren. Navigation ist lokales Steering, keine globale A*-Wegsuche;
Tierkoerper sind keine stapelbaren Physikkoerper. Weitere Grenzen und Messungen:
[Phase-55-Bericht](tools/benchmarks/results/creatures-caves-2026-10-18/README.md).

Neue Welten verwenden Geologievertrag 4: seltene, freiraumgepruefte Tropfstein-
Deckengruppen bis 14 m und zusammenhaengende Erdflecken in Hoehlen. Die vorhandene
Salz-/Mineral-Batchpipeline wird erweitert, keine Entity pro Spitze. Alte Geologie
0/1/2/3 wird nicht umgeschrieben. Im Chat blaettern Pfeil hoch/runter durch die
letzten 32 gesendeten Eingaben und stellen den vorherigen Entwurf wieder her;
Vorschlaege bleiben per Tab/Shift-Tab oder Klick erreichbar.

### Tierbeduerfnisse Und Umweltklang (Phase 56)

Geeignete 32-m-Habitate erzeugen nun haeufiger kleine Gruppen derselben Tierart.
Die vorhandene KI besitzt Nahrung und Wasser: Kuh 10 Leben/30 Nahrung/30 Wasser,
Schaf 7/35/25, Schwein 8/20/40, Fisch 3 Leben ohne diese Beduerfnisse.
Aktive Landtiere verlieren langsam Nahrung/Wasser, suchen passende Props und
erreichbares Wasser und trinken am trockenen Ufer. Fressen entfernt genau ein
Prop ueber den vorhandenen autoritativen Edit-/Replikationspfad. Ressourcen-
suchen sind auf acht je Takt begrenzt; lokale Navigation und Fernpause bleiben
bestehen. Faustschlaege verursachen auch in Creative einen Lebenspunkt.

F3+E schaltet die Werteanzeige fuer das anvisierte Tier ein/aus. Die neue Aktion
Inspect Animal Stats ist in Controls belegbar; das normale Inventar-E wird von
der Tastenkombination unterdrueckt. Das TiltUI-Panel zeigt aktuelle Leben,
Nahrung, Wasser und Verhalten und braucht das allgemeine Debug-Overlay nicht.

Echte aufgenommene Tierstimmen und natuerliche Foley ersetzen die bisherigen
synthetisierten Rufe. Fressen, Schritte, Schuetteln, Schwimmen und Trinken
loesen eigene Cues aus. Wind, Baumkronen und Hoehlen besitzen sanft ueberblendete
Umgebungsspuren. Ein gemeinsamer begrenzter Audiostream verwendet die
Spielerposition, Voxel-Verdeckung, Master/Ambience/Mute und Unterwasserdaempfung.
Dateien, Quellen, Anpassungen und separate Audiolizenzen:
[Audio Credits](assets/audio/CREDITS.md). Bevys WAV-Feature aktiviert hound 3.5.1;
keine bestehende Dependency-Version wurde aktualisiert.

Neue Regionen sind REG7, der explizite REG6-Leser erhaelt Tiere und tote IDs,
rechnet die Gesundheit anteilig um und startet Nahrung/Wasser voll. Zuvor leere
Habitate duerfen einmal nach den dichteren Gruppenregeln geprueft werden.
PLY3, Weltformat 2, Generator 5 und bestehende Geologievertraege bleiben erhalten.
Phase 56 verwendete Protokoll 30; aktuell benoetigen beide Protokoll 31.
[Phase-56-Bericht](tools/benchmarks/results/animal-needs-audio-2026-10-18/README.md).

### Materialschritte, Hoehlenstil Und Landschaft (Phase 57)

Die Overworld verwendet jetzt aufgenommene Vogel-/Insektenkulissen mit Tageszeit-
und Bewuchsgewichtung statt der bisherigen Dauer-Wind-/Laubschichten. Dieselbe
begrenzte Audioquelle spielt sechs Untergrundtypen mit je zwei Aufnahmen:
Gras, Sand, Stein, Erde, Schnee und Holz. Tritte folgen der tatsaechlich
zurueckgelegten Bodenstrecke von lokalem Spieler, replizierten Spielern und
Landtieren; Flug, Teleports, Luft und Wasser erzeugen keine Bodentritte.
Spielerposition, Verdeckung, Live-Lautstaerke und Unterwasserfilter bleiben
erhalten. Ein gedaempftes Delay-Netzwerk ergaenzt Schritt-Hall in Hoehlen;
es ist kein physikalischer Faltungs-Hall und verwendet keine Modaufnahmen.

Sand-/Schneespuren halten fuer Spieler 10/22 Sekunden, fuer Tiere 3/5 Sekunden.
Eine gemeinsame transiente Mesh-Entity aktualisiert maximal 512 Kontakte mit
20 Hz; Sohle und gespaltene Hufe unterscheiden sich. Die letzten 35 Prozent
der Lebensdauer blenden aus. Entfernte, ueberbaute und entladene Untergruende
entfernen die Spuren ebenfalls. Diese kurzlebige Clientdarstellung aendert
weder Voxel, Regionen noch Serverphysik und wird nicht gespeichert.

Geladene Deckenproben steuern weiche Hoehlenuebergaenge. Ein eigener WGSL-Pass
verwendet die vorhandene Prepass-Tiefe fuer kuehle Fels-Schatten, erhaltene warme
Highlights und dezente Tiefenstaffelung. Er laeuft nur bei Hoehleneinfluss;
Umgebungslicht, Himmels-Environmentmap und Belichtung werden dazu abgestimmt.
Unterwasser beherrscht weiterhin die bestehende Wasserumgebung. Dies ist ein
eigener stilisierter Ansatz, keine Implementierung von Hytales Renderer/GI.

`world.landscape_revision = 1` gilt fuer neue Welten; alte TOML-Dateien ohne das
Feld lesen 0 und behalten ihr Terrain. Die Weltanlage setzt 1 auch dann, wenn
die globale Vorlage aus einem alten Projekt stammt. Continentalness verwendet
das bestehende Kontinentfeld; neue unabhaengige Erosion-, Peaks/Valleys-,
Jaggedness- und Surface-Lake-Felder beeinflussen vorberechnete Spalten.
Geschlossene Oberflaechenseen nutzen vorhandene lokale Wasserstaende, Quellen,
Fluid-/Render-/Speicherpfade und die gemeinsame Beckenform der Oasen.
Welt-/Block-IDs, Seed, Generatorversion 5, REG7 und PLY3 bleiben unveraendert.
Client und Server gemeinsam auf Netzwerkprotokoll 31 aktualisieren.
[Phase-57-Bericht](tools/benchmarks/results/soundscape-landscape-2026-10-18/README.md).

### Hoehlenlicht, Fackeln Und Bienen (Phase 58)

Der Hoehlenpass laeuft jetzt explizit vor TAA und Tonemapping. Der bisherige
pauschale blaue Schattenlift entfaellt; eine sehr dezente neutrale Distanzhaze
wird nur in ausreichend hohen Kammern verwendet. Fehlende Dach-Sections
zaehlen nicht mehr als neue Lichtoeffnung. Sonnenlicht nutzt weiterhin echte
Schattenmaps an den tatsaechlichen Hoehlenoeffnungen; dunkle Seitentunnel
erhalten keine kuenstliche blaue Grundhelligkeit.

Wichtig fuer Bevy 0.19: `AtmosphereEnvironmentMapLight.intensity` wird nur bei
der Erzeugung in `GeneratedEnvironmentMapLight` und `EnvironmentMapLight`
kopiert. Der Hoehlenwechsel synchronisiert deshalb alle drei Komponenten,
nicht nur die Ausgangsdefinition. Andernfalls bleibt die wirksame blaue
Himmelsfuellung trotz scheinbar reduziertem Einstellwert auf Tagesniveau.
Zusaetzlich wird die Integrationsstrecke der Aerial-View-LUT in geschlossenen
Raeumen weich begrenzt: Aussenluft-Streuung kennt keine Hoehlenwaende und
wuerde sonst beim Anpassen der Augenbelichtung einen blauen Schleier erzeugen.
Der Himmel an Oeffnungen bleibt aktiv; die Oberflaeche behaelt ihre bisherigen Werte.
Sonne/Mond werden zur Hoehlen-Augenbelichtung kompensiert, damit direkt
besonnte Flaechen am Eingang nicht ausbrennen. Lokale Fackeln bleiben warm
lesbar; diese stilisierte Abstimmung ersetzt kein indirektes Lichttransportmodell.

Section-Mesher speichern einen 736-Byte-Bitset fuer opake Nachbarzellen.
Asynchrone Renderjobs berechnen daraus interpolierte Kontaktabschattung;
gemeinsame Eckwerte werden pro Job nur einmal ausgewertet. Transparente
Voxel und Spezialmodelle verschliessen keine ganzen AO-Zellen. Blockaenderungen
invalidieren auch betroffene diagonale Halo-Sections. Normale LDR-Farben werden
als Unorm8 gepackt, HDR-Holz-/Biomfarben bleiben unangetastet.
Opake Schattenproxies sperren Licht von beiden Seiten; nur nach unten sichtbare
Hoehlendecken bleiben dadurch auch ohne geladene Oberflaeche richtige Lichtblocker.
Hierfuer werden nur umgekehrte Indizes derselben Schattenvertices ergaenzt,
keine weiteren Draws oder Materialkopien. Laubmasken bleiben unveraendert.

`configs/graphics.toml` ergaenzt `cave_quality`, ueber Graphics / Cave Lighting
mit dem vorhandenen Apply-/Speicherpfad einstellbar:

| Stufe | Zusaetzliche Darstellung | Lokale Lichter | Punkt-Schattenmap |
| --- | --- | ---: | ---: |
| Low | Gecachte Voxel-Kontaktabschattung | 4 | 512 |
| Medium (Default) | Dezente kammerabhaengige Distanzhaze | 8 | 512 |
| High | SSAO Medium, dezentes Bloom | 12 | 1024 |
| Ultra | SSAO High, Bloom, begrenztes 32-Step-Nebelvolumen mit Sonnenstrahlen | 16 | 1024 |

SSAO wird nur ohne MSAA verwendet (normalerweise mit dem bestehenden TAA).
Bei MSAA bleibt die gecachte Abschattung aktiv. Zusaetzliche Normal-Prepasses,
Bloom und Volumetrik werden beim Verlassen der Hoehle wieder entfernt.
Unterwasser wird kein zusaetzliches Hoehlen-Nebelvolumen angelegt.
Hierfuer ist nur Bevys bestehendes `bevy_post_process`-Feature aktiviert,
keine Dependency-Version erhoeht und kein eigener Bloom-Renderer gebaut.

Die angehaengte Block-ID 40 ist `teramic::torch`. Die Fackel nutzt ein echtes
statisches Voxel-glTF, passende Teilkollision, `struct_block = true` und das
Rezept `assets/crafting/torch.json` (ein Stock + eine Kohle). Ihr warmes Licht
reicht 18 m und schwankt kontinuierlich um hoechstens 5.5 Prozent. Normale
Cube-Bloecke bleiben auf der gemeinsamen Geometrie-Pipeline. Optionale
Block-JSON-`light`-Metadaten und Material-`emission` werden validiert; Diamant-
und Rubinkristalle erhalten echte farbige lokale Lichter. Verdeckte Erze
leuchten nicht durch ganze Steinschichten; nahe Erzgruppen teilen einen
Emitter. Der begrenzte Pool verwendet echte Punktlicht-Schattenmaps.
Die Quellen liegen ausserhalb undurchsichtiger Flammen-/Kristallgeometrie,
damit sie sich nicht selbst vollstaendig abschatten. Eine viertelsekuendliche,
auf 64 Kandidaten begrenzte Sichtpruefung spart Schattenmaps fuer andere
geschlossene Kammern; unveraenderte Section-Emitterlisten bleiben gecacht.
Fackel-Edits werden direkt aus dem vorhandenen Journal ueberlagert; Licht
wartet nicht auf einen asynchronen Meshwechsel und bleibt nach Abbau nicht stehen.

Spielerkontakte unterscheiden Landung und Aufprall nach mehr als zwei Metern.
Bestaetigte Abbau-/Setzereignisse verwenden die aufgenommenen Material-Foley
mit angepasster Lautstaerke/Tonhoehe, inklusive vorhandenem Hoehlenhall und
Unterwasserfilter. Fehlgeschlagene Edits erzeugen keinen Erfolgs-Impakt.

Bienen sind eine weitere Art im bestehenden autoritativen Tiermodell.
Eine eigene ID-Namespace erhaelt alte Tieridentitaeten; maximal zwei Bienen
gehoeren zu einem Nest. Die logische Koerpergroesse ist 0.12 x 0.075 x 0.25 m.
Geteilte Body-/Fluegelmeshes animieren den Flug, keine Entity pro Detailvoxel.
Die KI besucht echte Flower-Props, landet und krabbelt auf geladenen soliden
Blockflaechen. Sie verbraucht die Blumen dabei nicht und verwendet das
bestehende begrenzte Such-/Updatebudget. Eigene prozedurale Flug-/Landecues
liegen im gleichen Audiomixer, keine uebernommenen Spielsounds.

Nester sind `BeeNest`-Props mit normalem Picking, Abbau, Section-Batching und
Journal-/Regionspersistenz. Die Laufzeit-Oekologie setzt sie ausschliesslich
an Waldbaeume, maximal einmal je 32x32-m-Habitat (vier Chunkspalten), mit
zusaetzlicher Seltenheitsauswahl. Terrain, Generatorversion, Seeds und
existierende Block-IDs bleiben unveraendert. Entfernte Nester/tote Bienen
werden beim Reload nicht neu ausgewuerfelt. REG7/PLY3 bleiben lesbar;
Client und Server gemeinsam auf Netzwerkprotokoll 32 aktualisieren.

`--smoke-test-caves` aktiviert eine isolierte handgesetzte Licht-Testhoehle,
nicht eine neue Weltgenerierung: Felskammer, clearancegepruefte Tropfsteine,
Sonnenoeffnung, echte Fackel-/Kristalllichter, statischer Solid-Water-See und
dunkler Seitentunnel. Natuerliche unterirdische Mehrschichtseen, Lava,
leuchtende Pflanzen und indirekte GI werden damit nicht als umgesetzt ausgegeben.
Details und Messungen: [Phase 58](tools/benchmarks/results/cave-lighting-bees-2026-10-18/README.md).

### Lebendige Tierwelt Und Ziel-HUD (Phase 59)

Fackeln besitzen sichtbares animiertes Feuer und geneigte Wandmontage.
Modell, Trefferform und budgetiertes PBR-Licht verwenden dieselbe Pose.
Geschlossene Relief-Schattenproxies vermeiden schwarze Mikroflaechen, ein
schwaches Hoehlen-Grundlicht erhaelt die Orientierung. Pflanzen erhalten
kurze weiche Bodenkontakte statt langer Sonnenschatten.

`assets/terra/mobs/*.json` konfiguriert die fuenf vorhandenen Arten. Bienen
sammeln endlichen Blumennektar, kehren nachts in Nester zurueck und lagern
dort Nektar fuer die Honigreifung. Neue Gruppen enthalten bis vier Bienen,
ein Nest fasst zehn. Leere Blumen regenerieren nach 180 aktiven Sekunden;
100 Nektar reifen in 300 Sekunden, abzueglich 2.5 Prozent Zeit pro anwesender
Biene, zu einer Einheit Honig. Honigentnahme/Bestaeubung bleiben Erweiterungen.
Landtiere schlafen nachts, lokale Ausweichwege und Schwimmen sind erweitert,
Fische springen gelegentlich. Wasserkontakte erzeugen begrenzte Spritzer,
Schwimmspuren und gelegentliche Atemblasen nur fuer Saeugetiere.

Die normale TiltUI-Zielkarte oben mittig zeigt Block-/Propnamen und Registry,
tatsaechlich erforderliches Werkzeug und Stufe, Mining-Fortschritt oder
Mobstatus und Lebensbalken. `hud_stats` in Prop-/Block-JSONs waehlt `bar` oder
`text` fuer vorhandene Nektar-/Honig-/Belegungs-/Reifungswerte.
Client und Server gemeinsam auf **Protokoll 36** aktualisieren. Neue Regionen
sind **REG9**, mit expliziten REG8-/REG7-Lesern; **IDX3** speichert die laufende
Oekologie-Uhr im kleinen Weltindex, ohne unveraenderte Regionen neu zu schreiben.
IDX2/INDX und aeltere Regionen bleiben lesbar, Spielerdateien bleiben PLY3.
Keine Generator-, Seed-, Block-ID- oder Dependency-Aenderung.

Details, Quellen, native Ansichten, Tests und Messgrenzen:
[Phase 59](tools/benchmarks/results/living-world-2026-10-18/README.md).

### Weltleben Und Bedienung (Phase 60)

Natuerliche Mobs werden bis etwa 150 m um alle Spieler gesucht. Default ist
ein gemeinsames Limit von 256, kein Limit je Spieler. Dedicated Server koennen
`natural_mob_limit = 256` in `configs/server.toml` setzen (0..4096); bei der
Einzelspieler-Erstellung speichert ein Slider das Limit je Welt. Besitzer-
zuordnungen sind persistent und werden nicht mitgezaehlt, eine spielbare
Zaehmung bleibt eine eigene Erweiterung.

Bienen teilen Blueten untereinander auf, bleiben maximal 100 m beim Nest und
verschwinden zum Abliefern darin. Ruhige Schlafposen mit geschlossenen Augen,
weniger stationaeres Drehen und ein zusaetzlicher Riverfin-Fisch erweitern die
gemeinsame Tierdarstellung. Chat bietet weiches Ausblenden, kontextabhaengige
Pfeilnavigation und @-Erwaehnungen mit Hervorhebung/Benachrichtigung. Im
HUD-Editor sind Vitalanzeigen, Chat und Zielkarte getrennt verschiebbar.
Strg antippen schaltet gerichtetes Tauchen, Leertaste laesst auftauchen;
Spieleravatare verwenden eine Schwimm-/Tauchpose. Aufnehmbare 3D-Drops fliegen
vor der autoritativen Inventaruebernahme zum Spieler.

Nur neue Welten aktivieren automatisch `bounded_world`: Block-Y -128..356,
Grundstein an -128/-127, keine Hoehlen unter -120, mehr trockene Eingaenge/
Schluchten und Erz-Obergrenzen. Alte Weltmetadaten ohne dieses Feld behalten
ihr Grundterrain. Neue Dedicated-Welten uebernehmen den Vertrag bei der
Erstellung; existierende Welten werden nicht still umgestellt. Grundstein
ist ID 41 ohne eigene GLB und kann in Survival nicht zerstoert werden.
Neue Regionen sind REG9; REG8 und alle bisherigen Leser bleiben unterstuetzt.
Client und Server gemeinsam auf das aktuelle Protokoll 35 aktualisieren.

Technische Grenzen, Audioquellen und Abnahme stehen im
[Phase-60-Bericht](tools/benchmarks/results/world-life-2026-10-18/README.md).

### Fackeln Und Gemeinsame Inventare (Phase 61)

Fackeln bleiben exakt anwaehlbar, blockieren aber keine Bewegung. Kleine
verwurzelte Flammenzungen mit weicher Shaderbewegung, bewegtes/flackerndes
PBR-Licht und sanftes koerperbezogenes Feuer-Foley erweitern dieselbe Pipeline.
Die Lichtauswahl besitzt keine feste 50-m-Abschaltung mehr und reagiert direkt
auf Edits. Nachladen wird budgetiert. Aktive Fackeln im eigenen und in bis zu
drei nahen entfernten Spielerslots erzeugen getragene 18-m-Lichter.

Das Wildtierlimit ist jetzt lokal: Default 256 lebende unbesessene Tiere in
150 m um den jeweiligen Spieler, nicht 256 in der gesamten gespeicherten Welt.
Alte Tiere werden bei Unterschreitung des Limits nicht geloescht oder geklont.
Bienen nutzen ausser Wald auch passende Baeume in Plains und Highland.

Options besitzt einen Multiplayer-Tab. `configs/multiplayer.toml` speichert
Spielernamen, Truhencursor und optionale eigene Cursorfarbe erst bei Apply.
Automatische Farben sind profilbasiert, gesaettigt und stabil; Grau/Schwarz/Weiss
sind keine Cursorfarben. Namen mit leichtem Hintergrund und Textschatten werden
ueber entfernten Spielern dargestellt. Geteilte Truhencursor stammen nur von
autorisierten Betrachtern derselben Truhe; Item und Name kommen vom Server.
Ruhende Positionen bekommen kurze Lebenszeichen, Namen werden nur bei
Rosterwechseln uebertragen. Das ist kein Inventarzugriff auf fremde Truhen.

Chat und Survival-HUD bleiben beim geoeffneten Inventar sichtbar. Das Debug-HUD
liegt darueber. Das Inventar wird erst nach stabiler Layoutberechnung sichtbar.
Netzwerkprotokoll **35**, Client und Server gemeinsam aktualisieren; REG9 bleibt
unveraendert. Details: [Phase 61](tools/benchmarks/results/torch-social-2026-10-18/README.md).

## Bewegung, Inventar Und Itemmodelle (Phase 62)

Netzwerkprotokoll dieses Stands: **36**. Client und Server gemeinsam neu bauen/starten.
Tierprodukte und Werkzeuge erweitern die bestehenden Enums ausschliesslich am
Ende; vorhandene Varianten und Block-IDs bleiben stabil. REG9 und PLY3 bleiben
lesbar. Aeltere Spielversionen kennen die neuen Items nicht und sind kein
unterstuetzter Downgrade fuer damit gespeicherte Inventare.

Die serverseitige KI nutzt begrenzte Voxel-A*-Umwege (96 Knoten), artspezifische
Freiraumpruefungen und raeumlich gebinnte Abstandshaltung. Bienen bremsen/lenken
weich, reservieren einzelne Blumen und koennen bei Hindernissen ausweichen,
statt automatisch am Baum zu landen. Flugrouten sind kurzlebig gecacht und
werden vor Bewegung gegen das aktuelle Terrain geprueft. Dies ist keine globale
Navmesh und keine Garantie fuer beliebig lange Labyrinthe.
Nahe Tiere werden mit 10 Hz, entfernte mit 5 Hz und tatsaechlich verstrichener
Zeit simuliert; maximal 512 Updates pro Simulationstakt. Die fruehere rotierende
128er-Grenze verlangsamte groessere Bestaende. Identische Fern-Snapshots starten
die Interpolation nicht erneut. Neue Pakete beginnen am bereits dargestellten
Ort, nicht am veralteten Paket-Endpunkt.

Volumenpruefungen lesen dieselbe belegte Zelle nur einmal und nutzen vorbereitete
Terrain-/Wasserspalten. Live-Fluidaenderungen haben Vorrang; ohne vorbereitete
Spalte bleibt die bisherige kanonische Abfrage der Fallback. Kein neuer
persistenter Wasser- oder Navigationsspeicher.

Schlafbeginn und Aufwachen sind stabil pro Tier versetzt. Schlafende Tiere rufen
nicht, sondern verwenden leises Atem-Foley. Gesetzte Fackeln verwenden eine
gefilterte CC0-Feueraufnahme; getragene Fackeln bleiben akustisch stumm.
Diamant und Rubin besitzen keine Eigenemission oder Punktlichter mehr; ihre
Materialreflexion bleibt. Fackeln sind weniger grell abgestimmt und besitzen
24 m Cullingradius; Bevy-PBR schwaecht Licht weiterhin physikalisch ab, nicht
als gleichmaessig helles Voxel-Blocklicht.
Der vorhandene kleine schattenlose Stuetz-Fill sitzt etwas unterhalb des
Halters und mildert dessen dunklen Nahschatten. Das ist eine lokale
Streulichtapproximation mit 2.4 m Reichweite, keine indirekte Lichtsimulation.

Neue Welten speichern `ravine_revision = 1`: gewundene, unterschiedlich tiefe
Schluchten mit 3..11 m Oeffnungsbreite im Hauptlauf und auslaufenden Enden.
Fehlende Einstellung in alten Welten bedeutet 0 und behaelt deren Terrain.
Seed, Welt-ID und Generatorversion bleiben unveraendert.

Eigene JSONs in `assets/terra/items/` definieren rohe/gegarte Fleisch- und
Fischvarianten, Leder, Wolle, Honig sowie Holz-/Steinwerkzeuge. Gemeinsame
farbige Micro-Voxelmodelle werden fuer Inventar und 3D-Drops wiederverwendet;
vier neue geformte Werkzeugrezepte liegen in `assets/crafting/`.
Kuehe droppen auch Leder; Clownfische ihren eigenen Fischtyp. Gekochte Produkte
und Honig sind nutzbare Nahrung und im Creative-Katalog. Der Ofen aus Phase 87
kocht Tierprodukte; Honigentnahme bleibt offen.

`configs/multiplayer.toml` ergaenzt `player_name_distance = 40.0` (8..150 m),
mit Apply/Reset/Reload im bestehenden Multiplayer-Tab. Cursor werden zwischen
den 10-Hz-Updates geglaettet; Symbole stammen aus dem vorhandenen Tilt-Katalog.
Der Vorschaubild-Bridge ueberschreibt TiltUIs Deckkraft nicht mehr. Das Inventar
ist bereits vor der ersten Bindings-/Layoutphase unsichtbar, statt seine Bilder
kurz mit voller Deckkraft an der Startposition zu zeigen.

Abnahme und Messgrenzen: [Phase 62](tools/benchmarks/results/life-polish-2026-10-18/README.md).

## Licht, Schwimmen Und Gehaltene Items (Phase 63)

Aktuelles Netzwerkprotokoll: **37**. Client und Dedicated Server gemeinsam
neu bauen/starten. Der Server uebertraegt den ausgewaehlten Itemtyp und einen
Mining-Zustand aus seinem autoritativen Inventar; keine fremden Inventarinhalte.
REG9, PLY3, Block-IDs, Weltgenerierung und Dependencies bleiben unveraendert.

Fackeln verwenden 1450 statt 11500 Lumen bei 28 statt 24 m Cullingradius.
Der kleine Stuetz-Fill verwendet vier statt zehn Prozent. Dadurch bleibt das
warme Nahlicht begrenzt, statt den Hoehlenboden weiss zu ueberstrahlen. Die
PBR-Abschwaechung bleibt physikalisch; kein konstantes Blocklicht bis 28 m.

Die gemeinsame Physik auf Client und Server bremst nach dem Loslassen von
Abtauchen auf die aktuelle Tiefe ab, statt automatisch zur Oberflaeche zu
steuern. Leertaste steigt auf und behaelt den vorhandenen Ufer-Ausstieg.
Arme beschreiben seitliche Brustschwimmzuege statt ueber den Kopf zu schlagen.
Die vorhandene vertikale Kollisionskapsel bleibt erhalten.

3D-Items in der ersten und dritten Person verwenden denselben Mesh-/Material-
Cache wie Welt-Drops, einschliesslich Werkzeugen, Waffen, Props und Spezial-
bloecken. Die erste Person zeigt nur das Item, keine Hand. Die dritte Person
befestigt es am rechten Arm; lokale Schlaege bewegen Arm und Objekt nach vorn.
Remote-Survival-Mining animiert ebenfalls den Arm. Fuer entfernte einzelne
Creative-/Luft-/Mobschlaege fehlt noch ein allgemeines Attack-Ereignis.
Die erste Person verwendet die vorhandene Weltkamera, keinen zusaetzlichen
Renderpass; Wandclipping wird nicht mit einer separaten Viewmodel-Kamera geloest.

Truhencursor verwenden eigene kleine gecachte Mauszeiger-Bitmaps mit oberer
linker Spitze statt eines Papierfliegers. Namensschilder haben feste 17-UI-
Pixel-Schrift und werden nicht mit der Entfernung vergroessert. Der Tilt-
Colorpicker bleibt auch bei automatischer Farbe bedienbar: eine Wahl schaltet
auf manuelle Farbe, Apply speichert wie bisher `configs/multiplayer.toml`.
Der Itembrowser berechnet vollstaendige Reihen und Spalten aus dem echten
Layout inklusive Skalierung, Abstaenden und Resize statt fester 45er-Seiten.

Validierung und Grenzen: [Phase 63](tools/benchmarks/results/hand-light-ui-2026-10-18/README.md).

## Wassertreten Und Werkbanklaterne (Phase 64)

Fackelgeometrie ist jetzt komplett `NotShadowCaster`; andere Weltobjekte
werfen weiterhin Schatten im Fackellicht. Der bisherige zusaetzliche Nah-Fill
ist entfallen. `light.cave_intensity = 0.6` reduziert gesetzte und getragene
Fackeln in vollstaendigen Hoehlen um 40 Prozent, weich mit dem vorhandenen
Hoehlenblend. Aussenhelligkeit und 28-m-Cullingradius bleiben erhalten.

Die positive Armrotation schlaegt zur lokalen Blickrichtung (-Z), nicht
hinter den Koerper. First Person behaelt die Vorwaertsrotation und fuehrt
das Item deutlicher nach vorn; Regressionen messen Hand und Werkzeugspitze
statt nur den Ursprung des Modells.

Stillstehende Schwimmer richten den Koerper auf und treten mit langsam
weiterlaufenden Arm-/Beinzyklen Wasser. WASD aktiviert weich die horizontale
Schwimmpose. Lokale Bewegung verwendet die konfigurierten Tasten; entfernte
Avatare schaetzen den Antrieb weiterhin anhand ihrer Bewegung. Stroemung kann
daher bei entfernten Spielern als Schwimmen erscheinen. Kollisionskapsel,
Tiefe und Kameraposition werden durch diese reine Darstellung nicht geaendert.

Die Werkbank-JSON ergaenzt eine warme 4-m-Laterne im vorhandenen Lichtpool.
`light.offset` ist eine lokale Modellposition und folgt Ausrichtung und
zweiblockbreiter Geometrie; die sekundaere Zelle erzeugt kein zweites Licht.
`light.shadows = false` vermeidet eine zusaetzliche Cubemap. Fehlende neue
Felder behalten kompatible Defaults (Hoehlenfaktor 1, bisherige Position,
Schatten an). Der vorhandene emissive Laternenkoerper bleibt erhalten.
Keine neue Registry, kein Speicher-/Generator-/Protokollwechsel (weiter 37).

Abnahme: [Phase 64](tools/benchmarks/results/water-pose-lantern-2026-10-18/README.md).

## Referenzmodelle Und Streaming (Phase 65)

Zwei Avatarvarianten teilen die bestehende artikulierte Lauf-, Schwimm-, Schlag-
und Blinzelanimation. Kleidung, Haarbueschel, Gesicht, kurze Haelse, Hosenaufschlaege
und nackte Fuesse orientieren sich an der neuen Referenz. Meshes werden pro Variante
einmal erzeugt und zwischen lokalen/entfernten Spielern und Portraets geteilt.
Die Darstellung ersetzt keine neue Kollisionsform oder ein Skeletal-Animationssystem.

Lebensmittel und Werkzeuge erhalten volumetrische, verdeckte Innenflaechen entfernende
Voxelmodelle. Inventar, gehaltene Items und Drops verwenden weiterhin denselben
Modellcache. Ein neutrales mattes Material traegt die Vertexfarben; das Honigglas
ergaenzt einen zweiten transparenten Batch. Mehr Formdetails erhoehen die Vertexzahl,
nicht die Zahl einzelner Detail-Entities. Aus einzelnen Referenzansichten entsteht
eine eigenstaendige 3D-Rekonstruktion, keine behauptete pixelidentische Kopie.

Kies (42) und Strukturstein (43) ergaenzen die bestehende Registry mit normalen
Cube-Definitionen. Bestehende IDs und die Weltgenerierung bleiben unveraendert.
Die neuen Materialien verwenden gemeinsame kachelbare Texturen und Mips; kein
Block-GLB und kein neuer Terrainrenderer. Details und native Ansichten stehen im
[Phase-65-Bericht](tools/benchmarks/results/reference-streaming-2026-10-18/README.md).

Beim Streaming invalidiert das reine Entnehmen fertiger Mesh-/Wasserpayloads nicht
mehr den gesamten ChunkStore. Prop-Uploads markieren ihre Registry nur bei echten
Aenderungen. Terrain-/Prop-Jobs sortieren nur die fuer freie Worker benoetigte
Teilmenge; veraltete Section-Epochen werden vor der Auswahl ausgefiltert. Volle
Workerqueues ueberspringen die erneute Terrain-Kandidatensuche. Leere Sections
brauchen keinen Custom-Modell-Snapshot. Die Lichtsuche leiht unveraenderte Blockdaten
und prueft bei Nachbarwechseln nur bekannte Emitter statt erneut alle 4096 Zellen.

`TERRA_CPU_SCOPES=1` aktiviert optionale aufsummierte CPU-Zeitbereiche fuer native
Benchmarks. Ohne Variable laufen keine Scope-Uhren und keine zusaetzlichen Logs.
Die Werte enthalten auch den Start; verschachtelte Bereiche sind nicht addierbar.
Sie sind kein Ersatz fuer GPU-Zeitmessungen oder lange Multiplayer-Flugtests.

## Charaktere, Tierdetails Und Fackellicht (Phase 66)

Die beiden Avatarvarianten verwenden zehn gemeinsam gecachte Koerpermeshes mit
Schulter-, Ellenbogen-, Hueft- und Kniegelenken sowie einem Blinzelmesh. Geformte
Gesichts-/Koerperflaechen, Haarbueschel, Finger, Zehen und Kleidung ersetzen die
bisherigen starren Gliedmassen. First Person zeigt nun die echte rechte Hand und
einen Teil des Unterarms; gehaltene Items und Hand teilen denselben Griffanker.
`--gender=f` und `--gender=m` verwenden unveraendert das bestehende Spielerprofil.
Die Animation bleibt prozedural artikuliert, kein importierter Motion-Capture-Rig.

Schaf, Kuh, Schwein, Biene und Clownfisch erhalten facettierte Formdetails, Augen,
artspezifische Silhouetten und weichere Ruhe-/Bewegungsuebergaenge. Der bestehende
Flussfisch bleibt unveraendert. Gemeinsame Meshes und Materialien sowie die
bisherigen Teil-/Entitybudgets der Tiere bleiben erhalten; kein Fell-, Fluegel-
oder Hufdetail erzeugt eine eigene Entity. KI, Kollisionsformen und Spawnregeln
werden dadurch nicht veraendert.

Fackel, Eisen- und Goldbarren nutzen detaillierte gemeinsame Itemmodelle fuer
Inventar, Hand und Drops. Der Item-Fackelkoerper ist farbig und nicht emissiv;
seine Beleuchtung wird bewusst von seinem eigenen nahen Punktlicht entkoppelt.
Gesetzte Fackeln bleiben PBR-beleuchtet, mit separaten animierten Flammen.

`light.falloff = "soft_core"` in der Fackel-JSON waehlt eine weichere lokale
Lichtverteilung. Alte Definitionen behalten standardmaessig `inverse_square`.
Die Erweiterung ersetzt ausschliesslich die markierte Punktlicht-Abschwaechung
in Bevys bestehendem Cluster-PBR-Shader: vier Meter weicher Kern, achtfacher
Lichtfluss und unveraenderter glatter Reichweitenabschluss. Dadurch sinkt der
Nah-Hotspot, waehrend entfernte Flaechen mehr Licht erhalten. Es entstehen keine
zusaetzlichen Lichter, Schattenmaps oder Fullscreen-Paesse. Das ist weiterhin
direktes PBR-Licht mit Flaechennormalen, keine gleichmaessige Voxellicht-Flutung.

Die Shadererweiterung prueft die erwartete Bevy-0.19-Signatur streng. Bei einer
inkompatiblen Bibliothek werden weder Marker noch erhoehter Lichtfluss aktiviert;
die native Hoehlenprobe behandelt diesen Fallback als Fehler. Der interne Radius
`128.375` ist ausschliesslich ein Shader-Marker und wird vor allen drei
Punktlicht-Spekularberechnungen wieder auf `0.38` dekodiert. Experimentelle PCSS-
Quellgroessen, Lichtgizmos und volumetrische Fackellichter muessen vor einer
spaeteren Aktivierung ebenfalls diesen Vertrag beruecksichtigen.

Schaden zeigt schmalere transparente Randverlaeufe mit begrenzter,
trefferabhaengiger Intensitaet und weichem 0.45-s-Ausklang. Mehrere Treffer
addieren keine Deckkraft. Kritische Gesundheit pulsiert schwaecher; die
Bildmitte erhaelt weiterhin keine rote Vollflaeche.

Native Vergleichsbilder, Geometrie-/Lichtmessungen, alle vier Streaminglaeufe
und die vollstaendige Validierung sind im
[Phase-66-Bericht](tools/benchmarks/results/character-light-creatures-2026-10-18/README.md)
archiviert. Referenznaehe und Animationsqualitaet bleiben eine Art-Abnahme;
die Implementierung ist keine pixelidentische Kopie und kein FPS-Paritaetsnachweis.

## Baeren, Hoehlenbelichtung Und Ego-Items (Phase 67)

Das neue `assets/terra/mobs/bear.json` erweitert dasselbe Artenregister wie die
bestehenden Tiere: 24 Leben, 45 Hunger, 20 Durst; ein einzelner Baer in einem
Teil der neu untersuchten feuchten Uferhabitate. Gespeicherte Populationen,
Todesmarkierungen und Habitat-IDs werden nicht ersetzt. Bereits untersuchte
Gebiete werden nicht nachtraeglich mit Baeren aufgefuellt.

Bei hoechstens 15 Hunger sucht der Baer echte lebende Fische in einem begrenzten
raeumlichen Index. Erst der Server bestaetigt einen erreichbaren Fang und
erhoeht den Hungerwert; mehrere Baeren koennen denselben Fisch nicht doppelt
verzehren. Wasserbedarf und Schlafen nutzen die vorhandenen Beduerfnisse und
Tageszeitregeln. Gelegentlich sucht ein satter Baer einen tragenden Baumstamm,
klettert bis sechs Meter und steigt wieder ab. Blockabbau, freie Koerperhuelle
und geladene Zellen werden dabei weiter geprueft. Die lokale Navigation bleibt
budgetiert und verwendet den vorhandenen Wegcache, kein globales Navmesh.

Nahe Survival-Spieler loesen einen Angriff aus: standardmaessig vier Meter
Aggressionsradius, 1.9 Meter Schlagreichweite, 12 Schaden und 1.4 Sekunden
Cooldown. Diese Werte sind in der Mob-JSON aenderbar. Sichtblockaden,
Respawn-Schutz und der bestehende Todesablauf werden serverseitig beachtet;
Creative-Spieler werden nicht angegriffen. Die Darstellung verwendet acht
gemeinsame Teilmeshes mit Fellstufen, Schnauze, Brustfleck, Tatzen/Krallen und
eigenen Jagd-, Schlaf-, Kletter- und Angriffsposen. Es gibt noch keine eigenen
Baerenlaut-Aufnahmen, Zaehmung oder speziellen Baerenprodukte.

Die Fackel-Ueberbelichtung bei laufender Uhr war ein Reihenfolgefehler:
`daylight::update` schrieb vorlaeufig die Aussenbelichtung, die Fackelverstaerkung
las diesen Wert, anschliessend stellte die Hoehlenanpassung die Belichtung um.
Lokale Lichtstaerke wird jetzt nach der endgueltigen Hoehlenbelichtung berechnet.
Soft-Core-Verteilung, Lichtbudgets und Aussenreichweite bleiben unveraendert.
Die native Hoehlenprobe enthaelt nun dieselbe geschlossene Fackelkammer mit
pausierter und laufender Mittagsuhr, damit der Fehler nicht wieder hinter einer
pausierten Testwelt verschwindet.

Die Ego-Hand aus Phase 66 ist auf Wunsch wieder entfernt. Items verwenden
weiterhin die gemeinsam gecachten Dropmodelle; einmalig gespeicherte Modellbounds
verankern ihr unteres Ende ausserhalb des sichtbaren unteren Bildrands.
FOV und Seitenverhaeltnis werden beruecksichtigt. Werkzeuge sind gegenueber der
vorherigen Haltung um eine Vierteldrehung gedreht; Schlagbewegung geht weiterhin
nach vorn. Weltavatare, Inventarportraits und F5 behalten ihre Arme und Griffpunkte.

Netzwerkprotokoll 39 fuehrt die zusaetzlichen Arten-/Verhaltenswerte ein.
Client und Server gemeinsam aktualisieren; alte Peers werden vor der
Replikation abgewiesen. Bestehende Regions-/Spielerformate und Generatorversion
bleiben erhalten, neue Enumwerte sind angehaengt statt umnummeriert.

Geometrie, native Bilder, Lichtmesswerte und die finale Testabnahme stehen im
[Phase-67-Bericht](tools/benchmarks/results/bears-light-viewmodel-2026-10-18/README.md).

## MiniMap Und Beleuchtungsuebergaenge (Phase 68)

Die nordorientierte TiltUI-MiniMap sitzt oben rechts. `M` klappt sie ein/aus,
`+` und `-` beziehungsweise die beiden Kartenbuttons wechseln zwischen
96, 192, 384 und 768 Metern Ausschnittbreite. Die goldene Markierung zeigt
Position und Blickrichtung aus dem bestaetigten Serverzustand, auch im
Singleplayer. Freie Kameras erkunden keine neuen Gebiete. Waehrend Inventaren
ist die Karte ausgeblendet; Chat-/Menueeingaben loesen keine Kartentasten aus.
Alle drei Kartentasten sind ueber die vorhandenen Controls-Einstellungen
neu belegbar; alte Bindings werden ohne Ueberschreiben eigener Tasten ergaenzt.

Der gemeinsame Kartenunterbau in `terra-world::cartography` verwendet bestehende
`PreparedColumn`-Snapshots. Ein Hintergrundjob verarbeitet maximal acht
16x16-Meter-Tiles; es gibt keine zweite Kamera oder erneute Noise-Abtastung.
Erkundet werden vorhandene Spalten im Radius von 96 Metern um den Spieler.
Bis 4096 Tiles bleiben im Cache (vier MiB Pixeldaten plus Verwaltungsdaten).
Das 192x192-Kartenbild wird hoechstens fuenfmal pro Sekunde und nur bei
geaendertem Inhalt, Position, Richtung oder Zoom hochgeladen.

Erkundungsdaten liegen clientseitig unter
`configs/maps/{world_id}-{seed}-v{generator}/dimensions/overworld/{uuid}/x_z.map`.
`TERRA_CONFIG_DIR` verschiebt auch diese Daten. Tiles sind versioniert,
pruefsummengeschuetzt, groessenbegrenzt und werden atomar ersetzt. Sie sind ein
kosmetischer Kartenstand, kein zweites autoritatives Weltinventar/Editjournal.
Unbekannte Bereiche bleiben dunkel. Aktuell wird die natuerliche Oberflaeche
mit Biom-, Hoehen-, Schnee- und Wasserfarben dargestellt; Bauwerke, dynamische
Wasserbearbeitung, Hoehlenkarten und Wegpunkte sind keine Bestandteile dieser
ersten Oberflaechenkarte. Weitere Dimensionen existieren noch nicht.

Sonnen-/Mondlicht werden in belichteter Helligkeit abgestimmt, bevor die Werte
in Szene-Illuminanz umgerechnet werden. Dadurch verstaerkt die Augenanpassung
den Sonnenuntergang nicht zwischenzeitlich. Der Horizontshader mischt Nebel
und eine explizite, leicht entsaettigte lineare HDR-Nachtpalette kontinuierlich.
Ein sanfter Helligkeitsboden verhindert einen komplett schwarzen Himmel nach
ACES-Tonemapping. Tageslicht-Endwerte, Hoehlenanpassung und lokale
Fackellichter bleiben an den vorhandenen Lichtablauf gebunden.

Landtiere verwenden eine gemeinsame schrittweise Fallauflosung ueber das
Koerpervolumen. Teilweise Auflage an Kanten stoppt den Fall an der Kontaktflaeche
statt am widerspruechlichen Mittelpunkt-/Vierfusssensor. Schlafende Tiere
uebernehmen die gepruefte Bodenhoehe nach Bodenabbau ebenfalls.

Tests, native Ansichten und Grenzen:
[Phase-68-Bericht](tools/benchmarks/results/minimap-daylight-grounding-2026-10-18/README.md).

## Raeumliche MiniMap Und Gameplay-Optionen (Phase 69)

Die MiniMap verwendet einen runden Kompassrahmen mit 2,5D-Relief, kurzen
schattierten Hoehenkanten und echten Oberseitenfarben aus den geladenen
Blockmaterialien und Texturen. Gecachte Texturmittelwerte entstehen einmalig,
bevor render-only Texturen ihre CPU-Daten freigeben. Biomturf und Wasser
verwenden dieselben Farbpaletten wie das Welt-Rendering. Geladene Baumkronen,
gesetzte Bloecke und bestaetigte Fluidaenderungen gehen in die Kartentiles ein.

Tierportraets und cyanfarbene Mitspielermarker stammen aus den vorhandenen
replizierten Zustandsdaten. Tote Tiere und Bienen im Nest werden ausgeblendet.
Geladene undurchsichtige Decken trennen Kartenebenen auch fuer Marker; Laub und
Wasser bleiben durchlaessig. Die Spielerposition bleibt serverbestaetigt.

Automatisch wird unter bedecktem Grundterrain in eine Hoehlenansicht gewechselt.
Sie zeigt offene, geladene Saeulen um die aktuelle Hoehe, mit getrennten Caches
in Viermeterbaendern. Unbekannte Zellen bleiben dunkel. Die Karte entdeckt keine
unbeladenen Hoehlen und projiziert keine geschlossenen Felsmassen als Laufwege.
Oberflaeche oder Hoehlen koennen auch fest ausgewaehlt werden.

`Options > Gameplay` enthaelt jetzt `HUD Layout` und `MiniMap > Customize`.
Das TiltUI-Popup bietet Sichtbarkeit, Groesse, Standardzoom, Ansicht,
Spielerdrehung, Relief, Tiere, Mitspieler, Koordinaten, Biom und Uhrzeit.
Alle MiniMap-Einstellungen liegen in `configs/gameplay.toml`; das vorhandene
Apply-/Reset-/Verwerfen- und Display-Rollback-Verhalten gilt auch hier.
Bestehende gespeicherte HUD-Positionen bleiben kompatibel in `graphics.toml`.

Ein Kartenjob verarbeitet bis vier geaenderte Tiles asynchron. Die Darstellung
wird hoechstens fuenfmal pro Sekunde und nur bei geaenderter Position, Richtung,
Geometrie, Palette oder Markerlage neu gezeichnet. Eingeklappt wird nur einmal
pro Sekunde erkundet. Der Cache ist auf 4096 Farbtile-Eintraege und 256 separat
alloziierte Detailtiles begrenzt; fehlende Eintraege reservieren keine Inline-
Detailarrays. Alte Farbtile-Dateien bleiben lesbar und werden bei erneuter
Erkundung mit den aktuellen Farben aktualisiert. Hoehen-/Materialdaten werden
derzeit nur fuer geladene Bereiche gehalten; entfernte gespeicherte Tiles
behalten ihre Farben, aber noch kein dauerhaftes Relief.

Native Ansichten, Testnachweise, Budgets und Asset-Prompt stehen im
[Phase-69-Bericht](tools/benchmarks/results/spatial-minimap-gameplay-2026-10-19/README.md).

## FullMap, Kartenraten Und HUD-Rahmen (Phase 70)

Die FullMap auf `M` und die MiniMap auf `N` verwenden denselben erkundeten
Kartenatlas. `+`/`-` steuern die MiniMap; FullMap bietet Mausziehen,
cursorbezogenen Mausradzoom von 96 bis 4096 Metern Ausschnittbreite und einen
Zentrierbutton. Diese Tasten sind in Controls neu belegbar. Alte Standard-
MiniMap-Belegungen auf `M` werden auf `N` verschoben, eigene Belegungen werden
bei der kompatiblen Ergaenzung beruecksichtigt. Die geoeffnete FullMap gibt die
Maus frei und blockiert Gameplay-Eingaben; Escape schliesst zuerst eine offene
Wegpunktbearbeitung und anschliessend die Karte.

Oberflaeche, lokale Hoehlenebenen, Materialfarben, Tiere und Mitspieler verwenden
die bestehenden Kartendaten und replizierten Positionen. FullMap erzeugt durch
Verschieben keine neue Weltgeometrie: nur bereits erkundete Tiles werden aus
dem clientseitigen Katalog nachgeladen. Unbekannte Gebiete bleiben dunkel;
eine Ladeanzeige markiert ausstehende Kartenarbeit. Benannte Wegpunkte mit
Farbe und Koordinaten werden in `waypoints.toml` im jeweiligen
`configs/maps/{world_id}-{seed}-v{generator}/dimensions/overworld/{uuid}/`
gespeichert. Hinzufuegen, Bearbeiten, bestaetigtes Loeschen und Wiederholen nach
einem Dateifehler verwenden denselben Welt-/Spielerschluessel wie die Karte.
Hoehlenwegpunkte behalten ihre Kartenebene. Weitere Dimensionen existieren
noch nicht; Kartenpins teleportieren keinen Spieler.

Terrain und bewegte Anzeigen haben getrennte Takte. Das schattierte 2,5D-
Terrainbild wird auf dem Compute-Pool mit unveraenderlichen sichtbaren
Tile-Snapshots hoechstens fuenfmal pro Sekunde neu gerastert. Ein kleines
`UiMaterial` an den vorhandenen TiltUI-Bildnodes reprojiziert diesen Cache
ueber einen affinen UV-Transform fuer Bruchkoordinaten, Drehung und Hoehenlage.
Ein transparenter zweiter Texturlayer enthaelt Mob-/Spielermarker. Wegpunkte
verwenden seit Phase 72 separat budgetierte TiltUI-Labels. Seit
Phase 71 entsteht der eigene Richtungspfeil direkt im vorhandenen UI-Material.
Das vermeidet einen vollstaendigen Terrain-Neuaufbau
pro angefordertem Kartenbild; es entstehen weder eine weitere Weltkamera noch
ein zusaetzlicher Welt-/Fullscreen-Pass. Die Kosten von Overlay-Uploads und
einem Custom-UI-Material bleiben dennoch reale Renderarbeit.

`Options > Gameplay > MiniMap > Customize` bietet 24, 30 (Default), 40 oder
60 FPS sowie einen Slider fuer Tierportraets von 1 bis 2,5. Die Rate begrenzt
seit Phase 71 die Markertextur-Updates; UV-Reprojektion und eigener Pfeil folgen
jedem Renderframe. Terrain-Generierung und Server-KI behalten ihre eigenen
Takte. Spielerpfeil und Mitspielermarker werden nicht mit dem Tierslider
vergroessert. Einstellungen werden wie zuvor erst mit Apply in
`configs/gameplay.toml` uebernommen. Die unterste MiniMap-Steuerleiste ist
entfernt, ihre restlichen Lesefelder sind kuerzer und mittig ausgerichtet.

Der Atlas behaelt maximal 4096 volle 16x16-Farbtiles und 256 geladene
Hoehen-/Materialdetails. Bis 65536 separat geteilte 4x4-RGBA-Fernkacheln erhalten
die erkundeten Farben bei grossen Ausschnitten; sie ersetzen keine feineren
Hoehendaten. MiniMap und FullMap verwenden 320x320 beziehungsweise 640x640
Terrain- und Overlay-Texturen, die vorhandenen Kacheldateien bleiben lesbar.
Kachel-IO, Materialrevisionen und ausstehende Rasterjobs sind voneinander
getrennt. Diese Zahlen sind Implementierungsbudgets, keine gemessenen
FPS-/Speichervergleichswerte.

`ornate-frame` ist eine wiederverwendete TiltUI-Komponente mit einem gemeinsam
genutzten transparenten Stein-/Gold-/Smaragd-Bitmap. Bevys vorhandene
`ImageNode`-Nine-Slices halten die Ecken bei verschiedenen Panelgroessen stabil.
Inventare, Truhen, Werkbank, Itembrowser, Rezeptdialoge, Menues, FullMap und
Hotbar verwenden denselben Rahmen. F1-Debug, Zielinfo und Chat behalten ihre
unverzierte Darstellung. Der aeussere Stats-Container ist transparent und
rahmenlos. Seit Phase 71 verbindet eine eigene Gold-/Stein-Chrome den grossen
facettierten Herztraeger und den Blitz mit den dynamischen roten/gelben Balken;
die separate Hungerkarte traegt ebenfalls einen nativen Fuellstand.
Bereits vorhandene Sauerstoffdaten werden weiterhin angezeigt;
zusaetzliche Gift-/Kaelte-/Feuermechaniken werden durch die Grafik nicht
vorgegeben. Die fuenf Keulenbilder wechseln bei 90/75/50/10 Prozent Hunger,
mit einem letzten Fleischstueck zwischen Haelfte und Knochen.

Tierbodenproben unterscheiden nun echte tragende Flaechen von reiner
Wasserauflage und begrenzen die ueberbrueckbare Fuss-Hoehendifferenz.
Gemeinsame Schwimmregeln halten den Unterkoerper von Saeugetieren unter der
tatsaechlichen Fluidoberflaeche, uebernehmen Stroemung und pruefen den
Uferausstieg gegen dieselbe Koerperhuelle. Enge Naehe loest weiterhin Flucht
aus; ruhende Tiere koennen nahe Spieler ansehen, ohne staendig wegzudrehen.
Das Baerenmodell und seine Abmessungen sind gemeinsam um 35 Prozent erhoeht.

1065 Tests bestanden, ein vorhandener UDP-Soak ignoriert. Format, Check, Build
und Clippy ohne Warnungen erfolgreich. Native FullMap mit echtem zweiten Peer,
Wegpunkt-CRUD, Hoehlenwechsel, skalierter Eingabeansicht und Kartenpresets;
Gameplay-Optionen mit Apply/Reset/Reload, transparentes Stats-HUD mit
Drag/Save/Reload und Inventarlayout bis 4K geprueft. Dedicated Server startet
ohne Display. Keine kontrollierte FPS-/Speichervergleichsmessung fuer diese
Aenderungen. Nachweise und konkrete Grenzen:
[Phase-70-Bericht](tools/benchmarks/results/fullmap-ui-animals-2026-10-19/README.md).

## Kartenbewegung, Referenz-HUD Und Materialschritte (Phase 71)

Die MiniMap verwendet fuer ihre Darstellung dieselbe interpolierte lokale
Koerperposition wie die Spielersicht sowie die aktuelle `CameraLook`-Drehung.
Cache-Textur und zugehoeriger UV-Bezug wechseln gemeinsam; die Reprojektion
laeuft in jedem Renderframe. Die langsameren Tier-/Mitspieler-/Wegpunktbilder
besitzen ihren eigenen Projektionsbezug, der eigene Richtungspfeil entsteht
live im selben GPU-UI-Pass. Die vorhandenen Markerpresets bleiben 24/30/40/60 Hz.
Terrain wird weiterhin asynchron und hoechstens fuenfmal je Sekunde gerastert.

Nordorientierte Terraincaches, geographisch verankertes Relief und
geographische Schattierung verhindern einen wechselnden Kartenaufbau beim
Drehen. Die Relief-Hoehenbasis bleibt je Kartenebene stabil, statt beim Springen
dem Spieler-Y zu folgen. FullMap-Mausziehen verwendet den beim Druecken
gespeicherten Mittelpunkt, Zeigeranker und Viewportmassstab. Neue Kartenbilder
aendern diesen laufenden Drag nicht.

Das TiltUI-Stats-HUD folgt der nachgereichten Referenz mit einem grossen
oktogonalen Herzmedaillon, Edelsteinen, zusammenhaengender Gold-/Steinleiste,
rotem Lebensbalken und kuerzerer gelber Ausdauer samt Blitz. Transparente
Rastergrafiken tragen die Verzierung, native Progress-Bars die aktuellen Werte.
Die Hungerkarte zeigt den gelben Fuellstand und die bisherigen fuenf Keulen-
stadien; 90/75/50/30/10 Prozent bleiben die Referenzbeispiele. Sauerstoff bleibt
bedingt sichtbar, ohne beim Einblenden die aeussere HUD-Geometrie zu aendern.
Gift, Kaelte und Feuer aus dem Referenzbild erhalten keine erfundenen Werte.
Art-Prompts und Herkunft stehen in `src-ui/assets/survival-chrome.txt`.

Schrittgeraeusche verwenden neun Materialfamilien mit insgesamt 51 kurzen
Kontakten: Gras (5), Sand (6), Stein (6), Erde (6), Schnee (6), Holz (5),
Kies (10), Glas (5) und Laub (2). Alle vorhandenen Blockmaterialien sind
explizit zugeordnet; Erz-/Mineralbloecke verwenden Stein, Holzvarianten Holz,
Pilzmaterialien Erde und die drei Laubvarianten Laub. Die Auswahl vermeidet die
letzte Aufnahme je Familie und variiert Tonhoehe/Lautstaerke behutsam.
Neue CC0-Aufnahmen ergaenzen die vorhandenen Quellen: Kenney-Foley,
Monte32-Erde und C-Dogs-Kies. Glas verwendet leichten Kontakt-Foley, keine
behauptete Aufnahme laufender Fuesse auf Glas. Quellen und Verarbeitung:
[Audio-Credits](assets/audio/CREDITS.md), `tools/assets/prepare71-steps.mjs`.

1072 Tests bestanden, ein bestehender UDP-Soak ignoriert. Format, Check, Build
und Clippy ohne Warnungen erfolgreich. Native Kartenpruefung mit 146.83-m-
Boostflug, Drehung, Hoehenwechsel und mehrteiligen FullMap-Drag-/Haltetests
bestanden. Stats-HUD, native Meteranimation und gespeicherte Verschiebung
geprueft; alle neun Audiofamilien ueber echten AudioSink abgespielt.
Neue Gebiete koennen beim schnellen Fliegen kurz dunkel bleiben, bis die
budgetierte Kartierung nachgezogen hat. Es gibt keine neue FPS-/Speichermessung
und keine menschliche Hoerabnahme. Nachweise und konkrete Grenzen stehen im
[Phase-71-Bericht](tools/benchmarks/results/map-motion-stats-steps-2026-10-19/README.md).

## Wetter, HUD-Kanaele Und Wegpunkte (Phase 72)

`WorldWeather` ist ein gemeinsamer, deterministischer Serverzustand mit Dauer,
Intensitaet, Wind und langsam trocknender Naesse. Ein klarer Abschnitt dauert
300..600, ein nasser 120..300 Sekunden, jeweils bezogen auf einen 24-Minuten-Tag.
24 Sekunden Smoothstep verbinden die Zustaende; pausierte Weltzeit pausiert
auch das Wetter. Hoehe, Temperatur, Feuchtigkeit und Wuestentrockenheit mischen
Regen/Schnee kontinuierlich. Alle Clients erhalten Startzustand und Snapshots,
interpolieren/extrapolieren lokal und erhalten nach Reconnect denselben Zustand.
Netzwerkprotokoll **40** verlangt gemeinsam aktualisierte Client-/Serverbinaerdateien.

Der kleine Weltindex `level.data` schreibt **IDX4** mit Wettermetadaten. IDX3,
IDX2 und INDX bleiben lesbar; Regionen bleiben REG9, Spieler PLY3. Keine
Generator-, Welt-, Material- oder Seed-Aenderung. Kalender-/Wetterupdates
invalidieren keine Terrain-/Fluidjobs und schreiben unveraenderte Regionen nicht neu.

Regenstrahlen, verzweigte Schneeflocken, Aufschlagringe und bewegte geschichtete
Wolken verwenden einen gemeinsamen transparenten GPU-Batch. Nur Dach-/Bodendaten
werden bei Bewegung oder betroffenen Blockaenderungen budgetiert aktualisiert:
maximal 24 Dachproben je Frame, maximal acht Geometrieuploads je Sekunde.
Keine Entity pro Tropfen und keine pro Frame neu animierten CPU-Vertexlisten.
Unbekannte Dachbereiche bleiben geschuetzt; feste und Glasdaecher, Hoehlen und
Unterwasseransichten werden beruecksichtigt. Klare Standardbedingungen brauchen
keinen Niederschlagsbatch. `Options > Graphics > Weather Quality` bietet
Off, Low, Medium, High (Default), Ultra. Off schaltet Effekte aus, nicht das
Serverwetter. Alte `weather`/`weather_intensity`-Felder werden weiterhin gelesen;
Produktionssitzungen verwenden sie nicht als Wetter-Override.

Wolken stimmen Sonnenlicht, Nebel und Farben ab, Naesse Materialfarbe/Rauheit.
Regen nutzt eine kreditierte CC0-Aufnahme im bestehenden Umweltmixer mit sanften
Uebergaengen, Dachfilterung, Hoehlenabsenkung und Unterwasserfilter. Hoerpunkt ist
der Spielerkoerper, nicht die Kamera. Regen reduziert Vogel-/Insektenpegel;
Schneefall selbst ist leise. Quelle/Aufbereitung stehen in
[Audio-Credits](assets/audio/CREDITS.md).

Schneeakkumulation und Schmelzen verwenden vorhandene persistente
`SurfaceCondition`s: eine geladene 16x16-Spalte je zwei Sekunden, maximal
65.536 Oberflaechenzustaende. Daecher werden statt Innenboeden getroffen,
Wasser/Feuer beeinflussen Schmelzen und Regen loescht exponierte Feuer.
Schnee ist kosmetische Deckung, kein neuer Block, keine Schneemassenphysik und
kein Collider. Blockabbau/-platzierung raeumt den betroffenen Zustand auf.
Unbeladene Gebiete werden nicht rueckwirkend simuliert; bei vielen geladenen
Spalten kann sichtbare lokale Akkumulation dem globalen Wetter nachlaufen.
Wolken sind eine Shader-Ebene, keine volumetrischen Wolken. Aufschlagringe
verformen die Wasseroberflaeche nicht physikalisch.

Die HUD-Chrome verwendet ein UI-Material innerhalb des vorhandenen TiltUI-Bilds,
das die roten/goldenen Fuellungen bis in die abgeschraegten Kanalenden zeichnet.
Die nativen TiltUI-Progress-Bars liefern weiterhin Werte und Animation; keine
zweite Vitalsverwaltung. Nur das rote Herz pocht: unter 60% alle 1,2 Sekunden,
unter 30% alle 0,62, unter 10% alle 0,36 Sekunden, jeweils als kurzer Doppelschlag.
`Options > Gameplay > Health Heartbeat` ist ueber Apply/Reset/Verwerfen steuerbar
und in `configs/gameplay.toml` gespeichert. Bei Tod, gesundem Herz und im
Layouteditor ist die Bewegung aus. `HUD Layout` verschiebt jetzt auch die
MiniMap; deren DPI-unabhaengige Position wird wie die bisherigen HUD-Positionen
in `graphics.toml` gespeichert. Die Hotbar bleibt fest.

Wegpunkte speichern zusaetzlich `icon` und `show_in_world`; alte TOMLs laden
mit Pin und ausgeschalteter Weltanzeige. Der FullMap-Editor bietet sechs Symbole
und einen Weltlabel-Schalter. MiniMap/FullMap zeigen Namen und Entfernung,
ausserhalb liegende Wegpunkte als Rand-Pins: `999m`, `1km`, `1.1km`, `2km`.
Symbole sind responsiv etwa 90% der Spielerpfeillaenge. Maximal 32 Karten- und
24 kamera-projizierte Weltlabels halten UI-Kosten begrenzt; nahe Randmarken
werden entstapelt. Wegpunkte bleiben lokal je Welt/Dimension/Spieler, ohne
Server-Teilung und ohne zusaetzliche 3D-Entities.

## Commands, Wetterfronten Und Weltkarte (Phase 73)

`/weather set clear|rain|snow|storm` aendert den gemeinsamen Serverzustand.
Der Wechsel dauert acht Sekunden, die Vorgabe gilt 240 Weltsekunden; danach
uebernimmt wieder der deterministische Klimazyklus. Storm bedeutet Starkregen
und staerkeren Wind, noch keine Blitze oder Donner. Bei pausierter Weltzeit
wird ein expliziter Wetterbefehl sofort voll wirksam; automatische Wetter- und
Oberflaechenentwicklung bleibt dabei eingefroren. Wetterbefehle unterliegen
denselben Serverberechtigungen wie andere Weltbefehle. Netzwerkprotokoll **41**
und Weltindex **IDX5** speichern den Wettermodus; IDX4 und aeltere Leser bleiben
unterstuetzt. Client und Server gemeinsam aktualisieren.

Die Dedicated-Konsole nimmt Befehle ohne `/` entgegen. Tab vervollstaendigt
Befehle und bekannte Spielernamen; Pfeil hoch/runter holt den Verlauf.
Globale Befehle wie `time set night`, `weather set rain`, `help`, `list` und
`stop` benoetigen keinen Spieler. `as Alice tp 10 80 20` setzt dagegen explizit
den Spielerbezug, etwa auch fuer `locate`. Direkte Adminformen sind
`tp Alice 10 80 20` und `gamemode creative Alice`. Die Eingabe laeuft getrennt
vom ECS. Unix-Terminals verwenden einen bewacht wiederhergestellten
`stty`-Modus, Windows `Console.ReadKey` ueber PowerShell. Fehlende interaktive
Terminalfunktionen und umgeleitetes stdin bleiben als zeilenweise Konsole
benutzbar; `TERM=dumb`/Pipes bekommen keine ANSI-Steuerzeichen.
Windows/macOS muessen separat nativ abgenommen werden.

`/help [Seite]` benutzt denselben Befehlskatalog wie die Konsole. Die Chatansicht
zeigt hoechstens fuenf Befehle mit getrennt gefaerbter Syntax und Beschreibung,
dazu echte TiltUI-Seitenpfeile statt eines einzigen langen Hilfetexts.

Regen besitzt dichtere, breitere Tropfenlayer und klarere Aufschlagringe.
Mehrschichtige bewegte Schoenwetterwolken sind auch ohne Niederschlag sichtbar;
eine dunklere Front beginnt bis zu 75 Weltsekunden vor dem automatischen Regen.
Cloud-Advektion wird integriert, sodass ein Windwechsel keine Texturspruenge
erzeugt. Richtung und Staerke desselben autoritativen Winds beeinflussen Laub,
Gras, Buesche, Pilzfasern und fallende Blaetter innerhalb bestehender Bounds.
Es handelt sich um geschichtete Shaderflaechen, nicht volumetrische Wolken.

Gespeicherte, dachgeschuetzte Oberflaechennaesse steuert einen lokalen PBR-Film
ueber UV1. Trockene Hoehlenboeden werden nicht durch einen globalen Regler
glaenzend. Betroffene Terrainbatches teilen sich ein WetMaterial je Basismaterial;
Mip-/Textur- und Materialaenderungen aktualisieren dieselben Handles.
Das bisherige Budget von einer geladenen Spalte je zwei Sekunden bleibt bestehen.
Regelmaessige Nahbereichsbesuche aller Spieler werden mit einem fairen
Hintergrunddurchlauf kombiniert. Bei vielen getrennten Spielern steigt die
Aktualisierungslatenz entsprechend, statt das Arbeitsbudget unkontrolliert zu erhoehen.

Die FullMap besitzt eine linke Werkzeugleiste, oberen Titel, Kompass und untere
Biome-/Koordinaten-/Zoom-/Hoehenzeile. Die Karte nutzt den restlichen rechteckigen
Platz. Raster, Mobanzeige, Hoehlen-/Oberflaechenebene und 2.5D/3D sind umschaltbar.
3D verwendet ein asynchron erzeugtes, tiefengeprueftes Hoehenfeld aus erkundeten
Kacheln und eine eigene orthografische Kamera auf RenderLayer 29. Ein gemeinsames
Mesh, kein nachgeneriertes Terrain und keine Entity pro Kartenblock. Die Kamera
ist ausserhalb der 3D-Karte aus; Pan/Zoom und Marker bleiben gemeinsam projiziert.
Es ist eine geografische Hoehenkarte, kein vollstaendiger gerenderter Welt-Diorama-
Export mit einzelnen Props oder Hoehlenueberhaengen.

Der Wechsel zwischen flacher/plastischer MiniMap kam vom Verlust der Hoehen bei
Farbkachel-Nachladejobs und Detailcache-Eviction. `TRMAP002` speichert deshalb
zusaetzlich 256 vorzeichenbehaftete Hoehen und 256 Wasserbits. `TRMAP001` bleibt
lesbar und wird beim erneuten Erkunden um reale Hoehen ergaenzt. Alte entfernte
Farbkacheln koennen ohne erneuten Besuch keine fehlenden Hoehen rekonstruieren.
Unbekannter Raum bleibt unbekannt. Das Format ist nur ersetzbare Kartografie,
keine Migration autoritativer Weltregionen.

`N` oeffnet einen unabhaengigen Wegpunktdialog mit den exakten Spielerkoordinaten;
die MiniMap liegt standardmaessig auf `J`. Vorhandene unveraenderte N-Defaults
werden kompatibel migriert, eigene abweichende Bindings bleiben erhalten.
Neue Wegpunkte sind standardmaessig in der Welt sichtbar; vorhandene gespeicherte
Schalter bleiben unveraendert. Ein Such-Popup bietet den echten TiltUI-Iconkatalog.
Weltlabels zeigen ein groesseres Symbol ueber `Name [100m]`; Karten behalten
Namen, Rand-Pins und kompakte Kilometerangaben. Die Wegpunkte bleiben lokal
je Spieler/Welt/Dimension, nicht im Multiplayer geteilt.

Native Pruefungen, Screenshots und Grenzen der kurzen Framezeitmessungen:
[Phase-73-Bericht](tools/benchmarks/results/commands-weather-map-2026-10-19/README.md).

## Kartenrelief, Pfuetzen Und Wetterstaerken (Phase 74)

MiniMap-Wegpunkte zeigen ihren Namen unmittelbar neben dem Icon, solange die
Marke innerhalb der Karte liegt. Rand-Pins bleiben reine Icons. Die MiniMap
zeigt keine Wegpunktentfernungen; FullMap und Weltlabels behalten sie.
Die FullMap zentriert Werkzeugtexte und Metadaten, verwendet in 3D ein groesseres
rechteckiges Renderziel mit MSAA und geschlossene, richtungs-/kontaktgeschattete
Hoehenseiten. Ihre 3D-Ansicht ist echte erkundete Topografie, kein vollstaendiger
Welt-Diorama-Renderer fuer Hoehlenueberhaenge oder einzelne Propmodelle.

Die alte 96-m-Snapshotgrenze, der +/-32-m-Reliefdeckel und Pixelsplat-Luecken
sind entfernt. Alle geladenen Spalten werden fortlaufend mit dem unveraenderten
Vier-Snapshotbudget erfasst. Fernuebersichten behalten kompakte Hoehen/Wasserbits.
Quellrevisionen ueberleben die Detailcache-Verdraengung; unveraenderte Diskdaten
loesen weder neue Paintjobs noch Speicherung aus. Die Kartenpalette benutzt
stabile Asset-Albedos statt der laufend wettergetoenten Materialfarbe. Damit
invalidiert eine Wetterueberblendung nicht mehr fortlaufend die ganze Karte.
Alte TRMAP001-Farbdateien ohne Hoehen brauchen weiterhin einen Streamingbesuch;
unbekannte Geometrie wird nicht nachgeneriert oder erfunden.

Die Terrainkomposition uebernimmt eine konservative Dachhoehe aus geladenen
Bloecken und bestehenden vorbereiteten Spalten. UV1 traegt lokalen Nasszustand,
Exposition und bei flachen Meeresufern einen begrenzten Kuestenwert. Waehrend
Regen folgt exponierte Naesse dem gemeinsamen Wetterwert statt der Reihenfolge
der Section-Jobs. Weltkoordinatenbasierte Beckenmasken machen auch Teilflaechen
eines Blocks nass; Glanz, Abdunklung und kleine Regenrippel liegen im vorhandenen
PBR-Material. Trockene Flaechen ueberspringen die Pfuetzenberechnung. Keine zweite
Naessetextur pro Block, keine Decal-Entity pro Pfuetze und keine neuen Draws pro Zelle.

Am Meer werden vorhandene Seehoehen-Wasserzellen in der geladenen 3x3-Nachbarschaft
zu Ozeanproben verbunden. Nur passende flache Ufer erhalten eine gemeinsam
gebatchte transparente Auflaufhaut. Tiefenkonturen steuern heranlaufende
Schaumbaender; eine ruhige gemeinsame Phase laesst den Saum kommen/zurueckweichen
und einen teilweise nassen Film zurueck. Nachladende vorbereitete Nachbarspalten
invalidieren nur betroffene Kuestensections. Es sind kosmetische Brandungsfilme,
keine Gezeiten oder hydrodynamisch versetzten Fluid-Fuellstaende. Entfernte
Meeresverbindungen ausserhalb des geladenen Halos sind nicht Teil dieser Probe.

Wolken sind jetzt zusammenhaengende, asymmetrische Voxelvolumen mit sichtbaren
Seiten und unterschiedlichen Unterseiten. Nur exponierte Faces entstehen auf
einem Worker; der Himmel bleibt ein geteilter Batch. Wind verschiebt ihn ohne
Geometrie-Upload pro Frame. Wetterfronten lassen die Wolken wachsen und weiss,
grau oder dunkel werden; entfernte Baenke gehen ohne sichtbares Punktmuster in
den Szenennebel ueber. Dies ist 3D-Geometrie, keine volumetrische Lichtstreuung.
Sturmboeen bewegen Laub und windfaehige Props bis zur vierfachen bisherigen
Maximalauslenkung. Rendergrenzen, exaktes Client-Picking, Auswahlkonturen und
serverseitige Swept-Pruefung verwenden zusammenpassende Wind-/Grenzregeln.

Neue Wetterbefehle, ebenso ohne Slash in der Dedicated-Konsole:

```text
/weather set thunder
/weather set thunderstorm
/weather set snow 1
/weather set snow 2
/weather set snow 3
/weather set snow 4
/weather set snow 5
/weather set blizzard
```

Schneestufen gegenueber Stufe 1: 1x/1x, 2.2x/1x, 2.6x/1.3x, 3x/1.75x und
Blizzard 4x/2x fuer Flockenzahl/-groesse. Sicht-/Dach-/Qualitaetsbudgets gelten
weiterhin; Blizzard besitzt staerkeren Wind. Clear/Rain/Snow/Storm bleiben gueltig.
Thunder und Thunderstorm planen gemeinsame serverseitige Blitzereignisse;
starke automatische Klimafronten koennen ebenfalls Gewitter erzeugen.
Ein kleiner Anteil plant einen Bodeneinschlag, der nur auf geladenem,
exponiertem Terrain aufgeloest wird. Schaden, Tierzustand, Drops und Feuer nutzen
vorhandene Regeln; unbekanntes Terrain wird fuer Blitze nicht generiert.
Clients zeichnen einen kurzen verzweigten Blitz/Lichtimpuls und spielen einen
entfernungsverzoegerten, synthetisierten Donner im vorhandenen Audiomixer.
Der Hoerer ist der Spielerkoerper, nicht die Kamera. Bei pausierter Weltzeit
laufen keine neuen autoritativen Blitze; ein alter Snapshot startet sie nicht staendig neu.

Netzwerkprotokoll **42**, Wetterindex **IDX6** mit Rueckwaertslesern fuer IDX5/IDX4
und aeltere Staende. Regionen REG9, Spieler PLY3, TRMAP002, Seed, Generatorversion
und bestehende IDs bleiben unveraendert. Client und Server gemeinsam aktualisieren.

Native Karten-/Wetteransichten, alle fuenf Cargo-Pruefungen, 1154 bestandene Tests,
Headless-/Konsolennachweise und kurze Framefenster:
[Phase-74-Pruefbericht](tools/benchmarks/results/weather-coasts-maps-2026-10-19/README.md).

## Wolkendecken, Schnee Und Live-Miniatur (Phase 75)

Die aktuelle FullMap-3D-Ansicht ersetzt das Phase-74-Hoehenfeld durch eine echte
orthografische Miniatur der geladenen Spielwelt. Terrain, Wasser, Baeume, Props,
Tiere und Spieler teilen ihre vorhandenen Modelle, Materialien und Animationen.
Die Hauptwelt wird unter der offenen 3D-Karte nicht nochmals gezeichnet; Layer
und Prop-Distanzbereiche werden beim Schliessen wiederhergestellt. Kein zweites
World-Streaming: Fuer entfernte Erkundung bleiben 2D/2.5D zustaendig.

Breite Voxel-Puffbaenke und zwei vereinfachte Fernringe bilden sparse/scattered/
broken/overcast/storm-Decken. Wolken behalten ihre physische Tiefe; nur die
Himmels-/Sonnensphaere nutzt eine explizite Reverse-Z-Hintergrundtiefe. Sonne und
Mond bleiben hinter Wolken, ohne die Terrain-Farplane zu vergroessern.
Die bisherige Blizzard-Dichte ist jetzt Schneestufe 1. Stufen 1..5 folgen lokal
den Faktoren 1/4/4.5/5/45; die extreme Stufe verdichtet budgetierte Zusatzlagen
um Augenhoehe und erzeugt keine Millionen Partikelquads.

Der Ladebildschirm zeigt echte 100%, haelt kurz und blendet weich zur Welt ueber.
Gameplay-Eingabe und Weltmixer werden erst danach aktiviert. Unterwasserflaechen
unterdruecken Regenfilme und gespeicherte Regennaesse; Wasserlinien-Caches reagieren
nur beim Wechsel der betroffenen Blockoberseite. Interpolierte Brandung und
sturmgesteuerte Wellen verwenden die vorhandenen Wasserbatches und Materialien.
Physische Fluid-Fuellstaende bleiben unveraendert.

Technik, Grenzen und Nachweise:
[Phase-75-Pruefbericht](tools/benchmarks/results/cloud-decks-miniature-2026-10-19/README.md).

## Karten, Wetter Und Kontakt-Audio (Phase 76)

Neue natuerliche Tierhabitate werden serverseitig deterministisch auf 60% der
bisherigen Kandidaten begrenzt, unabhaengig von Arten-/Herden-IDs. Bestehende
gespeicherte Tiere werden nicht geloescht, Besitz und konfigurierte lokale
Obergrenzen bleiben erhalten. Eine bereits volle Population schrumpft dadurch
nicht automatisch. Integrated und Dedicated verwenden dieselbe Regel.

Die echte 3D-Miniatur wird ausserhalb geladener Spalten um vorhandene erkundete
Hoehen-/Farbdaten ergaenzt: ein asynchroner Mesh-Batch, maximal 96x96 Rasterzellen
und 48000 Quad-Faces, Dirty-Pruefung hoechstens zweimal pro Sekunde.
Keine zusaetzlichen Gameplay-Chunks, Weltgenerierung oder entfernten Originalmodelle.
Unbekannte Gebiete bleiben unbekannt. Hoehlen werden als Bodenschnitt mit
Randwaenden und vorhandenen echten Tier-/Spieler-/Propmodellen dargestellt;
Daecher werden nur im Miniatur-Layer ausgeblendet und danach exakt restauriert.
Pflanzen-Batches werden nach ihrer transformierten AABB gefiltert, nicht nach
dem Section-Ursprung. Gemischte Oberflaechen-/Hoehlenbatches werden konservativ
ausgeblendet; eine einzelne Geometrie wird dabei nicht in zwei Meshes geschnitten.
Die Kartenerkennung prueft bis zur echten Oberflaechenhoehe statt nur 20 m,
der gemeinsame Licht-/Audio-Sampler ebenfalls ueber seine bisherige 64-m-Grenze
hinaus. Unbeladene Geometrie wird nicht als bekannte freie Luft behandelt.
Replizierte Spieler benoetigen keine fertige Detailkachel mehr, und beide Karten
zeigen ihre Namen aus dem bestehenden SocialState.

Schneestufe 5 behaelt ihre Dichte, verwendet aber kleinere Flocken mit individuell
gehashten Fallgeschwindigkeiten, Wirbeln, Rotation und begrenzter Winkelgroesse.
Wolkenblitze verlaufen
horizontal und verzweigt, mit weissem Kern, weiss/lila Halo und lokaler
Wolkenaufhellung; Einschlaege behalten ihren autoritativen Boden-Endpunkt.
Sturm kann bis 144 statt 48 Blaetter im weiterhin einzelnen lokalen Batch zeigen.
Unter Wasser bleibt nur der bestehende Regen-Aufprall-Batch sichtbar, keine
fallenden Tropfen und keine pauschalen Regenringe auf Hoehlenseen.

Regen verwendet zwei bearbeitete CC0-Aufnahmen im vorhandenen Stereo-Mixer.
Neun geladene Aufprallflaechenproben alle 0.5 s lassen den Klang mit Abstand zu
Boden, Wasser oder Daechern abklingen; oberhalb 32 m ist er stumm. Ein normaler
Sprung schaltet den Regen nicht abrupt ab. Koerperposition, Lade-Audiosperre,
Dach-/Hoehlen- und Unterwasserfilter bleiben erhalten. Tatsaechliche Journal-Naesse
und aktueller lokaler Regen ergaenzen Materialschritte um dezenten Wasserfilm.
Vier nicht direkt wiederholte Schwimmaufnahmen und pro Quelle getrennte Kadenz
ersetzen den einzigen Schwimmtake; das bestehende 24-Stimmen-Budget bleibt.
Neue dekodierte Aufnahmen benoetigen etwa 4.6 MB; keine zusaetzlichen Audio-Entities.

Technik, Validierung und verbleibende Grenzen:
[Phase-76-Pruefbericht](tools/benchmarks/results/weather-audio-map-2026-10-19/README.md).

## Flache Hoehlenkarten Und Stabile Mesh-Uploads (Phase 77)

Hoehlenkarten verwenden jetzt ausschliesslich 2D, auch bei aktivierter Relief-
Option und beim Wechsel aus einer laufenden 3D-Oberflaechenansicht. Dieselbe
Zustandsregel steuert UI, Kamera, Licht-/Schattenanpassung und Gameplay-Layer;
der 3D-Button ist im Hoehlenmodus deaktiviert. Die linke Werkzeugleiste besteht
aus sieben gleich grossen quadratischen Symbolbuttons mit TiltUI-Hovertooltips.

Der grobe Fernterrain-Ersatz und der Hoehlenschnitt aus Phase 76 sind entfernt.
Kartografische Oberflaechenhoehen schliessen Baumkronen ein und sind kein
zuverlaessiges Originalterrain; daraus entstanden die markierten kuenstlichen
Waende. 3D zeigt wieder ausschliesslich geladene Originalgeometrie, entfernte
Erkundung bleibt in 2D/2.5D. Keine zusaetzliche Spielwelt oder neue Kartenspeicherung.

Leere Unterwasser-Aufprallmeshes werden nicht mehr an den GPU-Allocator
uebergeben. Stattdessen bleibt der bisherige gueltige Batch verborgen und wird
bei neuen Quads mit demselben Handle reaktiviert. Bevys Null-Vertex-Allokation
und anschliessende Datenkopie erzeugten das gemeldete Slab-Allocator-Fehlerpaar;
die Meldung wird nicht unterdrueckt. Der zweite Erzeuger war der entfernte
Fernterrain-Batch.

Ein gemeinsamer 200-ms-Cache ersetzt die wiederholten langen Hoehlendachscans
beider Karten pro Frame; Welt-/Moduswechsel und grosse Positionsspruenge
invalidieren sofort. Wetter verwendet eine persistente geordnete Roofqueue und
berechnet Revisionssignaturen einmal pro horizontaler Section statt pro 1-m-Zelle.
Reine Fluessigkeitsaenderungen invalidieren Aufprallflaechen ueber die vorhandenen
Wasserepochen, auch wenn sich die Terrainrevision nicht aendert.
Die Limits von 24 Dachabfragen je Frame und acht Meshuploads je Sekunde bleiben.
Schneestreuung, Sturmlaub, Sichtweite und Wetterqualitaet werden nicht reduziert.

Messbedingungen, Tests, native Bilder und verbleibende Grenzen:
[Phase-77-Pruefbericht](tools/benchmarks/results/map-streaming-fixes-2026-10-19/README.md).

## Unterwasser-Dekoration Und Marine Tierwelt (Phase 78)

Der vorhandene Boden-Prop-Generator erzeugt jetzt Seegras, Seegurken,
verzweigte Korallen, Roehrenkorallen, Plattenkorallen, Seeanemonen und kleine
Steine. Eigene stabile IDs und dieselben Picking-, Abbau-, Drop-, Journal-
und Regionspfade gelten auch unter Wasser. Diese Arten sind ausdruecklich
wasservertraeglich; ein ueberfluteter Standort zerstoert sie nicht wie
Landpflanzen. Verteilung und Groesse beruecksichtigen Wassertiefe, tragenden
Untergrund, Biomtemperatur und Meeresgewicht. Deterministische Riffgruppen
ersetzen kein neues Voxelraster. Gemeinsame Vorlagen, LOD und Section-Batches
vermeiden Entities oder Draw Calls pro Korallenast.

Die gemeinsame Block-Registry enthaelt append-only ID 44 `coral_red`,
45 `coral_blue`, 46 `coral_yellow`, 47 `coral_purple` und 48 `mud`.
Definitionen und Beschreibungen liegen einzeln unter `assets/terra/blocks/`.
Die normalen Cube-Pfade erzeugen geschlossene Reliefoberflaechen mit roten
Rosetten, blauen Roehrenvertiefungen, gelben Platten beziehungsweise violetten
Loben. Mud verwendet braune Erde mit Kies-/Moosflecken. Matte 512px-Texturen
mit vorhandenen Mipmaps benoetigen weder eigene GLBs noch eine neue Pipeline.
Diese fuenf Bloecke sind baubar im Creative-Katalog; die natuerliche
Unterwasserdekoration entsteht ueber Props, nicht ueber nachtraeglich
eingesetzte Korallen-Cube-Vorkommen.

Die existierende Tierverwaltung erhaelt neun neue JSON-definierte Arten:
Squid; Moon Jelly, Fire Jelly, Void Jelly, Sun Jelly und Crystal Jelly;
Reef Shark, Hammerhead Shark und Whale Shark. Die Modelle teilen weiterhin
Meshes und Materialien pro Art; Bewegung und Beduerfnisse bleiben serverseitig.
Squid schwimmt, haelt sich an tragenden Flaechen fest und versprueht bei
Flucht Tinte. Quallen schwimmen und ruhen gelegentlich auf geladenem Boden.
Haie jagen vorhandene Fische; Reef und Whale Shark greifen entsprechend
der gewuenschten Spielregeln auch Spieler an, Hammerheads nicht von sich aus.
Meeres-/Seeauswahl und Populationsbudgets verwenden das bisherige Habitatsystem:
Haie und Quallen im Meer, Squid dort und selten in Seen. Gespeicherte Fische
und tote Tiere werden nicht fuer die neuen Arten ausgetauscht.
Die Biombindung bestimmt bisher nur neue Spawnkandidaten, nicht eine
fortlaufende unsichtbare Begrenzung schwimmender Tiere. Seit Phase 101 stehen
Spawngewichte, Wasser-/Tiefenfilter und optionale Herdengroessen in den
[Mob-JSONs](assets/terra/mobs/README.md). Fehlende Spawnfelder behalten die
bisherige Auswahl; gespeicherte Tiere werden nicht neu gewuerfelt.

Der bestehende Unterwasser-/Hoehlen-Umgebungspass ergaenzt tiefen- und
biomabhaengige Farbe, Lichtstreuungsanmutung und dezente bewegliche Details.
Squid-Tinte wird ueber den tatsaechlichen Spieler-Kopf im geladenen Fluid
getroffen, nicht ueber eine frei bewegte Kamera. Sie laeuft sichtbar nach
unten ab, klingt nach 4.2 Sekunden aus und wird beim Auftauchen/Weltwechsel
zurueckgesetzt. Niedrige Wasserqualitaet schaltet diesen Treffer nicht ab.
Die neuen Tier-/Kontaktgeraeusche verwenden den vorhandenen Mixer und
Unterwasserfilter; eine menschliche Gesamt-Hoerabnahme bleibt erforderlich.

Hoehlenkarten lassen sich auch ueber mehrere gehaltene Mausframes verschieben.
Der 3D-Button ist dort wirklich deaktiviert und sichtbar grau; abgelehnte
3D-Commands beenden keinen Drag. Die weiter entfernte Fackelausleuchtung ist
ab vier Metern 20% heller, ohne den Nah-Hotspot bis einem Meter anzuheben.
Wolken erhalten unabhaengige deterministische Hoehen; inklusive Shader-
Wachstum bleibt ihre gesamte Geometrie zwischen Y 200 und 312. Beim Aufklaren
zeigt High/Ultra hoechstens einen zusaetzlichen Batch mit 72 Vertices in
Wolkenluecken. Diese depth-getestete Weltgeometrie ist eine Lichtstreuungs-
Approximation, kein volumetrisches Wolken- oder Fullscreen-Raymarching.

`--smoke-test-reef` laedt einen echten generierten Meeresboden und platziert
explizite Testprops/-bloecke ueber vorhandene Journal-/Editpfade. Die Bilder
pruefen Modelle, Wasseransichten und Tinte; sie sind kein Nachweis, dass alle
Arten natuerlich unmittelbar nebeneinander vorkommen. Langzeitpopulationen,
Mehrspieler-Navigation, Audiohoerabnahme und Performanceparitaet bleiben offen.
Referenzbilder bestimmen den Stil; eine 1:1-Kopie wird nicht behauptet.

Netzwerkprotokoll **43**: Client und Dedicated/Integrated Server gemeinsam neu
bauen und starten. Keine Dependency-Upgrades oder Aenderung von Seed,
Generatorversion und bisherigen Block-/Welt-/Tier-IDs.
Konkrete Test-, native und Messnachweise:
[Phase-78-Pruefbericht](tools/benchmarks/results/underwater-ecology-2026-10-19/README.md).

## Zusammenhaengende Meeresbereiche (Phase 79)

`SurfaceColumn::ocean` speichert die gemeinsame deterministische Habitatprobe:
Klima und breite Felder mischen Coral Reef, Seagrass Forest und Cold Ocean.
Kleinere Koloniefelder und freie Kanaele verteilen Korallen auf Felsstufen,
hohes Seegras auf weichen Boeden und sparsameren Bewuchs unter Eis. Die
bestehenden IDs, Prop-Templates, Section-Batches, Interaktions- und
Speicherpfade bleiben erhalten. Shader und CPU-Picking verwenden dieselbe
staerkere Wasserbewegung mit festen Wurzeln; Pflanzfreiheit umfasst die ganze
Hoehe, auch ueber Section-Grenzen hinweg.

Neue Welten speichern `ocean_revision = 1` fuer verbundene Riffboden-Vorspruenge,
Materialgruppen und mehrschichtige Eisdecken. Fehlende Werte alter Welten
werden als 0 gelesen; gespeicherte Weltvertraege haben Vorrang vor aktuellen
Serverdefaults. So bleiben alte Boeden und Spielerbauten an ihrem Platz.
Die Prop-Verteilung wird auch dort verbessert. Der Seed bleibt unveraendert.

Block 49 `teramic::ice` ist solide, teilweise durchsichtig und nutzt die
normale Chunk-Cube-Pipeline mit eigener JSON-Definition und 512px-Miptextur.
Es gibt weder ein eigenes GLB noch Emission, Holzrahmen oder automatisch
ablaufendes Gefrieren/Schmelzen. Diese saisonale Integration bleibt offen.
Anemonen besitzen jetzt feine gekruemmte Tentakel; der Squid-Kopf ist in
X/Z 20% kleiner bei unveraenderter Hoehe und unveraenderten Armen.

Netzwerkprotokoll ist jetzt **44**. Client und Dedicated/Integrated Server
gemeinsam neu bauen und starten. REG9/PLY3, bisherige IDs und Generatorversion 5
bleiben erhalten; keine neuen Dependencies. Echte generierte Meeresansichten
prueft `--smoke-test-ocean-habitats`, Modelle separat `--smoke-test-reef`.
Konkrete Test-/Bildnachweise, Messwerte und Grenzen:
[Phase-79-Pruefbericht](tools/benchmarks/results/ocean-habitats-2026-10-19/README.md).

## Hoehlenraeume Und Einzelne Pflanzensegmente (Phase 80)

Neue Welten speichern `cave_structure_revision = 1`; fehlende Werte alter
Spielstaende bleiben 0. Die vorhandenen Hoehlenregionen erhalten feuchte
Salzraeume im bisherigen Cliffted Desert, breite Tropfsteinpfeiler, hoeher
gelegene bepflanzte Kammern und Highland-Raeume mit verbundenen Becken,
Wasserfaellen und Eiskristallen. Oberflaechenwasser und Hoehlenseen koennen an
derselben X/Z-Stelle getrennte Intervalle besitzen. Solver, Meshes, Schwimmen,
Audio und Regenausschluss lesen diese gemeinsame Belegung.

Append-only IDs 50..56 sind native Segmentbloecke mit einzelnen JSONs:
Seegras, Salzkristall, Tropfsteinspitze, Eiskristall, Ranke, Wurzel und
Hoehlenpilz. Gemeinsame Geometrie liefert Dreiecks-Picking und Section-Batches,
Mineralquerschnitte liefern passende Bewegungskollision. Kein GLB pro Zelle,
keine Entity pro Blatt. Metersegmente sind einzeln erntbar; fehlender Halt
entfernt abhaengige Segmente ueber das vorhandene autoritative Regionsjournal.
Abgetrennte Mineralspitzen fallen als vorhandene WorldDrops und koennen
Spieler verletzen. Salzhostbloecke liefern nichts, ihre Kristalle das bereits
vorhandene Salzitem. Tiere erhalten noch keinen Aufschlagschaden.

Natuerliches Meereis ist vollstaendig entfernt, auch beim erneuten Generieren
alter ungeladener Boeden; ausdruecklich platzierter Eisblock 49 bleibt erhalten.
Riffe erhalten breitere, hoehere verbundene Korallenruecken mit Bewuchs darauf.
Die Eisdeckenbeschreibung von Phase 79 ist damit historisch. Saisonales
Gefrieren und Schmelzen bleiben offen.

Fuer neue Hoehlenraeume und natuerlich segmentierte Seegraswaelder eine neue
Welt anlegen. Alte Hoehlenvertraege und ganzheitliche Seegrasprops werden
nicht still migriert; alte Seegrasitems bleiben lesbar und sind als Segment
pflanzbar. Seed, Generatorversion 5, alte IDs und REG9/PLY3 bleiben erhalten.
Netzwerkprotokoll dieser Phase **45**: Client und Dedicated/Integrated Server
gemeinsam neu bauen und starten. Darstellung, Messbedingungen, Testnachweise
und Grenzen: [Phase-80-Pruefbericht](tools/benchmarks/results/cave-structures-2026-10-19/README.md).

### Phase 81: Hoehlenrevision 2 Und Spectator

Neue Welten erzeugen haeufiger groessere Kammern mit weniger durchgehendem
Tunnelnetz. Salz bleibt lokal im bisherigen Biom, wechselt mit Stein und
begleitet geschwungene unterirdische Fluesse mit trockenen Terrassen. In
Gaengen wachsen vereinzelt Wurzeln und Pilze. Bestehende Welten behalten
ihre gespeicherte Hoehlenrevision; keine automatische Terrainmigration.

Breite, facettierte Salz-/Tropfstein-/Eisbuendel ersetzen lange Kegelformen.
Mit passendem Kristallitem eine beliebige Zelle anklicken, um das freie Ende
zu erweitern; fuer eine neue Nachbarkette den Untergrund anklicken. Client
und Server verwenden dieselbe Zielpruefung und das bestehende Editjournal.

`/gamemode spectator` beziehungsweise `/gamemode 2` bietet freien Flug durch
Terrain und neutrale Hoehlenaufhellung. Welt-/Inventarinteraktionen bleiben
gesperrt. Der Rueckwechsel braucht geladenen freien Raum. Das damalige
Netzwerkprotokoll **46**: Client und Server gemeinsam neu bauen/starten.
Seed, Generatorversion 5, Block-IDs und REG9/PLY3 bleiben unveraendert.
Details und Nachweise: [Phase-81-Bericht](tools/benchmarks/results/cave-refinement-2026-10-19/README.md).

### Phase 82: Gemeinsame Cave-Biome Fuer Kammern Und Gaenge

Neue Welten verwenden Hoehlenrevision 3. Eine gemeinsame, seedbasierte
raeumliche Verteilung von Normal, Salt, Lush und Cascade versorgt Kammern,
Verbindungstunnel und Eingangspassagen mit denselben Biomgewichten.
Materialien, Feuchtigkeit, kleine Kristalle, Wurzeln, Ranken, Pilze und
vorhandene Fern-/Grasprops gehen ueber regionale Mischzonen ineinander ueber.
Kleine geometrisch geschlossene Wasserbecken koennen auch in Gaengen entstehen.

Raumformen und Positionen variieren staerker; Doppelkammern und seltene
grosse Raeume erweitern die vorhandenen Profile. Die Noise-Tunnel bleiben
auch zwischen den Kammern vorhanden: moderate Ausduennung statt grossen
vollstaendig geschlossenen Gebieten.

In `configs/world.toml` sind `[cave_biomes.distribution]`,
`[cave_biomes.regions]` und `[cave_biomes.connectors]` dokumentiert und
konfigurierbar. Auswahlgewichte sind keine exakten Weltflaechenanteile.
F1 zeigt unterirdisch Biom, Mischung, Feuchtigkeit, Region und Connector-
Zuordnung. Der CPU-/Datenschnitt-Test ist als `cave_biomes_benchmark` verfuegbar.

Bestehende Welten behalten Vertrag und gespeicherte Biomkonfiguration;
fuer die neue Verteilung eine neue Welt anlegen. Aktuelles Protokoll **47**,
Client und Dedicated/Integrated Server gemeinsam aktualisieren.
[Phase-82-Pruefbericht](tools/benchmarks/results/cave-biomes-2026-10-20/README.md).

### Phase 83: Terrassen, Mineralbelag Und Simulierte Kaskaden

Neue Weltvertraege verwenden Hoehlenrevision 4. Salzterrassen mit getrenntem
Mineralbelag/Erz, Lush-Bodenschichten und mehrstufige Cascade-Becken erweitern
denselben Generator und dieselben biomabhaengigen Connector-Regeln.
Salt Stone 57 und Pink Salt Stone 58 verwenden Cube-JSONs ohne eigene GLBs.
Vorhandene Segment-/Prop-Batches bilden abgestufte Mineralcluster, geschwungene
Wurzeln und groessere Rankenblaetter ab. Props behalten ihre IDs und Entfernung.

`[cave_biomes]` bietet `salt_surfaces=[0.55,0.30,0.10,0.05]` und
`lush_density=0.72`. Die Oberflaechengewichte steuern koharente Belags-/Clusterzonen,
keine garantierten Flaechenprozente und keine Erzwahrscheinlichkeiten.
Neue Kaskaden speisen echte FluidCells aus deterministischen Beckenlippen.
Die vorhandene Autoritaet, 256-Zellen-Takte, Schlaf-/Reload-Logik, Wasserbatches
und Regionsdeltas bleiben zustaendig. Keine zweite Simulation und keine
synthetischen senkrechten Wasserfall-Curtains in diesen Raeumen.

Gespeicherte Hoehlenrevisionen 0 bis 3 behalten ihre logische Generierung;
kein Retrogen. Netzwerkprotokoll 48 verlangt gemeinsam erneuerte Client-/Server-
Binaries. Details, reproduzierbare native Testregionen und Messungen:
[Phase-83-Pruefbericht](tools/benchmarks/results/cave-overhaul-2026-10-20/README.md).

## Performance-Pruefung: Hohe Sichtweiten Und Wetter

Phase 85 erweitert die bestehende Streaming-Pipeline um persistente
invalidierte Kandidatenlisten, dreidimensionale Mikrodetail-/Prop-Abstaende
und gemeinsame Kartensnapshot-Indizes. Verdecktes Zellwasser liefert keinen
leeren Meshpuffer mehr; Terrain-, Prop- und Wasseruploads pruefen dies
zusaetzlich. Keine neuen Dependencies, Generator- oder Weltformataenderungen.

Die nativen 4K-Tests umfassen drei Seeds, Sichtweite 12/20 und echtes
Regen-/Schneewetter. Verbesserungen, verbleibende niedrige FPS, Streuung
und verworfene Versuche sind getrennt dokumentiert. Das Performance-Thema
ist nicht pauschal abgeschlossen:
[Phase-85-Pruefbericht](tools/benchmarks/results/performance-weather-2026-10-20/README.md).

## Essen Und Administrativer Mob-Spawn (Phase 86)

Ein Lebensmittel im aktiven Hotbar-Slot mit **Place Block / Eat halten** essen
(Default `B`, seit Phase 87); **Benutzen halten** (Default `R`) bleibt ebenfalls
moeglich. Beide Aktionen sind in Controls umbindbar. Der Server misst die gesamte Essdauer;
Loslassen, Slotwechsel, Menue und Treffer brechen ab. Ein abgeschlossener
Bissen verbraucht in Survival genau ein Item. Volle Hungerwerte lehnen
weiteres Essen ab; Creative zeigt den Ablauf ohne Verbrauch oder Wertebonus.
Spectator kann nicht essen.

Zwoelf vorhandene Lebensmittel definieren Hunger, Saettigung, Essdauer und
optionale zeitbegrenzte Regeneration in ihren einzelnen Item-/Prop-JSONs.
Der Tooltip zeigt diese Werte. Saettigung puffert Hungerabbau; Restitems
werden gemeinsam mit Verbrauch und Naehrwerten auf Platz geprueft.
Ego-/Avatarbewegung und leises Kau-Foley begleiten den serverseitigen Ablauf.

`/spawn teramic::clown_fish` beziehungsweise `/spawn teramic::cow` verwendet
dieselben Arten, die bestehende Simulation, Replikation und Speicherung.
Vorschlaege listen alle vorhandenen Mob-Namen. Creative/Spectator oder die
Environment-Berechtigung sind erforderlich. Wassertiere benoetigen genug
geladenes Wasser, Landtiere tragenden Boden und freien Raum. Die Serverkonsole
nutzt `as Spielername spawn teramic::cow`.

Phase 86 fuehrte Netzwerkprotokoll **49** ein. **PLY4** schreibt Saettigung und verbleibende Regeneration;
eingefrorene PLY3-/PLY2-Leser erhalten bisherige Inventare und Vitalwerte.
REG9, Generator und bestehende Welt-/Item-/Block-IDs blieben dabei unveraendert.
[Details und Validierung](tools/benchmarks/results/food-spawn-2026-10-20/README.md).

## Oefen, Essen Und Gleichmaessiger Regen (Phase 87)

**Place Block / Eat halten** isst das ausgewaehlte Lebensmittel mit derselben
Belegung wie das Bauen. Die bisherige Use-Aktion bleibt erhalten; zusaetzliches
Shift erlaubt weiterhin das Platzieren essbarer Pilze. Die vorhandene
serverseitige Essdauer, Abbruchregeln, Animation und Audio verwenden denselben Ablauf.
Regen trennt jetzt den Zufallswert fuer die Intensitaetsauswahl von der Fallphase.
Dadurch werden bei leichtem Regen nicht mehr nur zusammenhaengende Hoehenbaender
ausgewaehlt; Partikelzahl und Qualitaetsbudgets bleiben unveraendert.

Der **Ofen** belegt zwei Bloecke uebereinander. Block 59 und die Rohrvarianten
60 bis 64 erweitern die vorhandene Registry, ohne bisherige IDs zu verschieben.
Das Inventar besitzt getrennte, serverseitig gefilterte Plaetze fuer Eingang,
Brennstoff, Ausgabe und einen Blasebalg. Einzelne JSONs unter `assets/smelting/`
und `assets/fuels/` definieren Schmelzrezepte und Brenndauern. Too Many Items
zeigt Ofenrezepte samt Laufzeit und kann ihre Zutaten in einen geoeffneten Ofen
laden. Eisen und Gold werden im Ofen geschmolzen; die bisherigen
Handwerksrezepte zum Raffinieren dieser beiden Erze entfallen.

Alle vier **Bellows** werden aus einem Leder, zwei Holz und dem jeweiligen
Material hergestellt: Stein beschleunigt um 10 %, Eisen um 20 %, Gold um 10 %
mit 5 % Chance auf doppelten Ertrag und Ruby um 15 % mit 7,5 % Doppelchance.
Die Doppelchance folgt Welt-/Positionsseed und gespeichertem Abschlusszaehler.
Neuladen wuerfelt denselben Abschluss nicht neu aus.

Rauch folgt verbundenen, passend gedrehten Rohren ab dem oberen Ofenanschluss.
Ein blockierter Abzug staut zur Ofenvorderseite zurueck. In einem als geschlossen
erkannten Raum entsteht pro Brennsekunde eine Rauchzelle, maximal 256 je Ofen;
Lueften oder Erloeschen baut zwei Zellen je Sekunde ab. Rauchkontakt verursacht
2 Schaden pro Sekunde bei Survival-Spielern und lebenden Mobs, ohne Stapelung
durch mehrere Oefen. Creative und Spectator sind immun.
Das ist eine begrenzte Blockzustands-Simulation, keine Stroemungsphysik.
Unbekannte oder zu grosse Raeume pausieren die Ausbreitung.

Integrated und Dedicated verwenden dieselbe Autoritaet. Ohne verbundene Spieler
oder ohne geladenes Ofenpaar pausiert der Ofen. Je Verarbeitungslauf werden
hoechstens 64 aktive Oefen fair weitergeschaltet, mit maximal acht Sekunden
Nacharbeit je Ofen und Lauf. Die Darstellung nutzt zwei nichtleere,
wiederverwendete Meshes und maximal acht lokale Feuerlichter. Das leise
Feuergeraeusch nutzt den vorhandenen Fire-Loop, keine neue Aufnahme.
Modelle und Inventar sind an den Referenzen orientiert, nicht pixelgenau kopiert.

**Protokoll 50** verlangt gemeinsam aktualisierte Client-/Server-Binaries.
Neue Regionen schreiben **RGA0** mit Ofenfortschritt, Brennzeit und Rauch;
REG9 und aeltere Leserpfade bleiben erhalten. PLY4, Generator und bestehende
IDs bleiben unveraendert. Grenzen, Quellen und noch ausstehende native
Validierung: [Phase-87-Pruefbericht](tools/benchmarks/results/furnaces-rain-2026-10-20/README.md).

## Rauch, Tueren, Fenster Und Ofen-UI (Phase 88)

Die folgenden Aenderungen sind implementiert. Native Modell-/Shaderpruefung,
autoritativer Gameplaylauf mit Speicherung und Headless-Start sind bestanden.
Workspace-Ergebnisse, Aufnahmen und verbleibende Grenzen stehen im
[Phase-88-Bericht](tools/benchmarks/results/openings-smoke-2026-10-20/README.md).
Die Phase-87-Beschreibung oben dokumentiert den vorherigen Stand.

**Rauchdarstellung:** Die vorhandenen gespeicherten Rauchzellen steuern nun
Shader-Wolken und geglaetteten lokalen Tiefennebel, keine sichtbaren Blockwuerfel.
Der vorhandene Hoehlenpass uebernimmt die Raumtruebung; ein gemeinsam gebatchtes
Noise-Material zeichnet Austrittswolken. Rauch tritt entlang des erkannten
Rohrendes aus und steigt anschliessend auf. Das ist eine budgetierte optische
Approximation, keine CFD-Simulation oder physikalisch vollstaendige Volumetrik.

**Lueften:** Brennende geschlossene Raeume erzeugen eine Rauchzelle pro Sekunde.
Ein offenes Fenster mit Weg nach draussen entfernt seit Phase 90 0,5 Zellen pro Sekunde,
jede offene Tuerzelle 0,75; die zweiblockhohe Tuer also 1,5. Gegen die laufende
Erzeugung ergibt das bei einem Fenster netto +0,75 und bei einer Tuer -0,5.
Ein freies Loch in der Aussenwand oder offener Abzug entfernt zwei Zellen pro
Sekunde. Eine offene Innentuer allein ist kein Aussenabzug. Fraktionale Raten
bleiben im Ofenzustand erhalten; Schaden bleibt serverseitig bei zwei Punkten
pro Sekunde ohne Stapelung ueberlappender Rauchzellen.
Mehrere aktive oder noch rauchhaltige Oefen desselben Raums teilen sich die
Fenster-/Tuerleistung, statt sie je Ofen zu vervielfachen. Jede Rauchzelle hat
einen eindeutigen Besitzer; historisch doppelt belegte Zellen werden anhand
der Ofenkoordinaten deterministisch zugeordnet. Der Shader-Ruhepfad prueft ohne
lokalen Rauch kein Terrain und schreibt bei abgeklungener Restdichte keine
unveraenderten Nebel-Uniforms.

Die Suche bleibt auf 64 Rohrzellen, 4096 Raumzellen, 8192 insgesamt untersuchte
Raum-/Aussenraumzellen und 32768 Himmelsproben begrenzt. Unbekannte, unbeladene
oder zu grosse Bereiche pausieren weiteres Fuellen. Maximal 64 aktive Oefen
je Verarbeitungslauf, 256 gespeicherte Rauchzellen pro Ofen, 256 nahe eindeutige
gerenderte Rauchbereiche, 32 Feuer und acht Feuerlichter begrenzen die Arbeit.
Diese Budgets ersetzen keine Langzeit-FPS-/Speicherabnahme.

**Ein adaptives Schornsteinitem:** Nur Material 60 wird neu angeboten und
hergestellt. Platzierung leitet aus Anschluss, Spielerposition und Blick die
gerade, gebogene oder schraege Pose ab. Alle 15 ungeordneten Portpaare sind
weiter achsengebunden; schraege Varianten verbinden dieselben orthogonalen
Nachbarzellen mit anderer Modellform, keine diagonalen Nachbarbloecke.
Die alten Materialien 61 bis 64 bleiben fuer gespeicherte Welten und Inventare
mit Namen, Beschreibung und Modellen erhalten, fehlen aber im TMI-Katalog
und in neuen Craftingrezepten.

**Tueren und Fenster:** Oak-/Birch-/Spruce-Tueren belegen IDs 65/66/67 und zwei
Zellen Hoehe; die passenden Fenster 68/69/70 sind eine Zelle hoch.
Use oder Place auf das Modell oeffnet und schliesst es, ohne gleichzeitig zu
essen oder das Inventar zu oeffnen. Shift behaelt den bisherigen Bypass.
Der Server prueft Reichweite, Objektzustand, Berechtigung und freien
Spielerkollisionsraum vor dem Schliessen. Modelle animieren ihren Fluegel in
0,45 Sekunden und koennen waehrenddessen sauber umkehren; Kollision und
Rauchdurchlass folgen dem bestaetigten Objektzustand. Dies ist keine dynamische
Physiksimulation des Schwenkfluegels.

**Ofen-UI:** Material und Brennstoff stehen gemeinsam zwischen Bellows und
Ausgabe. Leere Plaetze besitzen passende TiltUI-Symbole; Fortschrittsanzeigen
bleiben den zugehoerigen Plaetzen zugeordnet. Das Ofenpanel hat keinen inneren
Scrollbar; kompakte Hoehen verwenden kleinere feste Slots. Creative-R im
Itembrowser zeigt auch Ofenrezepte. Bei geoeffneter Truhe ist Vorschau moeglich,
aber kein unsichtbares Handcrafting oder versehentliches Rezeptfuellen.

Neue Modelle sind an den Referenzen orientiert, keine behauptete 1:1-Kopie.
Modellgenaue Mining-Bruchmesh-Isolation beweglicher Tuer-/Fensterfluegel fehlt
noch; der bisherige Partikel-Rueckfall bleibt. **Netzwerkprotokoll 51** erfordert
gemeinsam aktualisierte Client-/Server-Binaries. Regionsformat RGA0, PLY4,
Generatorversion und bestehende Block-/Welt-IDs bleiben unveraendert.

## Ofen Und Oeffnungsdetails (Phase 89)

Tueren und Fenster verbrauchen den Interaktionsklick, lassen aber Maus-Capture
und Bewegung aktiv. Shift erlaubt weiterhin Platzierung am Objekt. Der Server
verhindert auch beim Oeffnen, dass ein Fluegel in einen Spieler gesetzt wird.
Auf-/Zuklappen, brennende Oefen und Rauchkontakt besitzen eigene Audioreaktionen
im vorhandenen Mixer. Husten folgt dem lokalen Spielerkoerper, nicht der Kamera.
Quellen und reproduzierbare Audiobearbeitung stehen in `assets/audio/CREDITS.md`.

Gerade Rohre und der obere Ofenblock passen sich seit Phase 91 ab drei opaken Nachbarn an
Wand oder Dach an: Bis zu vier Fuellelemente verwenden die jeweiligen Nachbarmaterialien
und schliessen den sichtbaren Zwischenraum. Der innere Rohrkanal bleibt frei.
Die Elemente gehoeren zum Modell und seinen vorhandenen Section-Batches;
deren Kollision und Auswahl verwenden dieselbe Geometrie. Freie oder gebogene
Rohre bekommen keine kuenstliche Ummauerung.

Die vorhandene Raumpruefung liefert jetzt auch sichtbare Austrittsfahnen an
offenen Fenstern und Loechern. Auslasssuche und Darstellung bleiben budgetiert
und lokal gecacht. Im Ofen entfallen Slotlabels und Prozenttexte; Icons und
Balken zeigen die Belegung und den Fortschritt. TMI startet mit All und nutzt
bei unzureichender Breite eine TiltUI-Kategorieauswahl statt abgeschnittener
Kategorietabs. Custom-Kategorien bleiben erhalten.

Pruefungen, Aufnahmen und Grenzen:
[Phase-89-Bericht](tools/benchmarks/results/furniture-fixes-2026-10-20/README.md).

## Kollisionsrevisionen Und Ofenpolitur (Phase 90)

Objektposen werden jetzt ueber `streaming::commit_object_state` geaendert:
Die betroffenen Section-Neubauten werden wie beim Blockedit eingeplant.
Das behebt die serverseitige Bewegungsverweigerung nach Tuer-/Fenstertoggles,
ohne Kollisionsschutz oder autoritative Korrektur abzuschalten. Ein nativer
Survival-Test prueft nach jedem Oeffnen auch die bestaetigte Serverbewegung.

Rauchabzug ueber Fenster betraegt 0,5 Zellen/s; die sichtbaren Austrittsfahnen
nehmen nach angesammelter Rauchmenge weich in Breite, Opazitaet und Steighoehe zu.
Drei dezente Hustenaufnahmen vermeiden direkte Wiederholung, Ofenfeuer ist
im bestehenden Mixer angehoben. Fackelintensitaet steigt um 25%, die bisherige
Soft-Core-Lichtkurve und Budgets bleiben erhalten. Tueren zeigen auch hinten
Planken, Schnitzdetails und Beschlaege. Rohrfuellungen docken pro angrenzendem
Vollblock an und lassen unbebaute Seiten frei.

[Umsetzung, Validierung und Grenzen](tools/benchmarks/results/furnace-contact-polish-2026-10-20/README.md).

## Langzeitperformance Und Replica-Lifecycle (Phase 91)

Der Client uebernimmt bestaetigte Journal-/Fluiddaten ohne autoritative
Simulationsarbeit einzureihen. Quellen, Revisionen, Propentfernungen und
Meshinvalidierungen bleiben erhalten; normale Speicherimporte und Server-
Reloads nehmen Fluid-/Grasarbeit weiterhin auf. Das verhindert die gemessene
ungenutzte Clientqueue, ohne das echte zellbasierte Wasser abzuschalten.

Blockierte Landtiere beachten das vorhandene Acht-Suchen-Budget und folgen
gecachten Umwegen. Suchplaetze rotieren fair zwischen anstehenden Routen.
Beduerfnis-Wasserabfragen verwenden vorbereitete Spalten mit Live-Overrides
und behalten den Rueckfall fuer noch nicht vorbereitete Spalten.

`bash tools/benchmarks/run-soak.sh target/debug/terra-client NAME delayed`
startet opt-in 15 Minuten Standbild und danach 15 Minuten echten integrierten
Fly-Input mit Rueckkehrkontrollen. Die Szenarien `idle`, `explore` und `edits`
sowie explizite isolierte Savepfade sind im Messbericht beschrieben.
`summarize_soak.py` unterscheidet Framefenster, CPU-Scopes, einzelne Renderpass-
Samples und Prozess-RSS/Swap; keine ausgedachte gesamte GPU-Framezeit.

Der 30-Minuten-Lauf zeigte keinen progressiven Kollaps; der Vorherlauf hatte
den gemeldeten Unter-18-FPS-Fall ebenfalls nicht reproduziert. Das ist keine
pauschale Performanceabnahme. Aufloesungsdrift im Flug, Speicherhoechststaende,
einzelne Frame-Spitzen und noch fehlende Lastvarianten bleiben ausgewiesen.
[Codepfade, echte Messreihen und Einschraenkungen](tools/benchmarks/results/progressive-collapse-2026-10-20/README.md).

## Bodenbewegung Und Streaming (Phase 92)

Section-Veroeffentlichungen, Entfernungen und vorbereitete Spalten melden
ihre Position ueber `SectionPublished`, `SectionRemoved` und
`PreparedColumnChanged`. Terrain und Props verarbeiten normale Arrivals
inkrementell, einschliesslich lokaler Exposure-/Kuestennachbarschaften.
Unerledigte Positionen bleiben bei belegten Workern erhalten, auch wenn die
urspruenglichen Bevy-Meldungen bereits abgelaufen sind. Echte globale
Invalidierungen behalten ihren vollstaendigen Auswahlpfad. Dispatch-
Buchhaltung allein markiert `ChunkStore` nicht mehr als geaendert.

Nach der initialen Szenenbereitschaft begrenzt ein weiches 2-ms-Budget die
Terrain-Vorbereitung; mindestens eine nuetzliche Uebergabe bleibt erlaubt.
Die Anfangsladung fuellt weiterhin alle verfuegbaren Worker. Beendete
Einblendungen pruefen nur betroffene aktuelle Roots erneut, ohne pro
Animationsende die gesamte Szene auszuwaehlen. LOD-Prioritaeten, Qualitaet
und ausstehende Arbeit bleiben erhalten.

Vegetation und nasse Oberflaechen teilen einen stabilen 48-Byte-
`ShaderBuffer`: Wind, Naesse/Regen/Schnee mit Wachstumsuhr und Wetterfarbton.
Der Renderpfad schreibt aktuelle Werte direkt in den vorbereiteten Buffer;
seine Material-Handles und Bindings bleiben unveraendert. Das vermeidet
`MaterialModified`-Ereignisse fuer diese dynamischen Kanaele und damit die
zusaetzliche Mesh-Extraktion der gemeinsam verwendeten Materialien.
Tatsaechliche Basismaterial-Aenderungen bleiben sichtbar. Keine neue
Wassersimulation, Vegetationsreduktion oder Dependency ist dafuer noetig.

Der opt-in Benchmark `walk` verwendet den normalen Charaktercontroller
mit Vorwaerts-/Sprungeingaben und bestaetigten Rueckwegen. Er ersetzt
keinen Test beliebiger Terrainstrecken. Konfiguration, Zwischenstaende,
CPU-Stichprobe und der jeweils aktuelle Validierungsstatus stehen im
[Phase-92-Bericht](tools/benchmarks/results/ground-streaming-2026-10-21/README.md).

Die opt-in Image-Probe trennt CPU-Pixeldaten, Fontatlas-Texturen und reine
Descriptor-Payload-Schaetzungen von gemessenem DRM-VRAM. Sie ordnete das
langfristige Bildwachstum wiederholt neuen Systemfont-Blob-IDs zu, nicht
neuen Schriftgroessen. Der UI-Pfad aktiviert deshalb Fontiques vorhandenen
Shared-Sourcecache und haelt tatsaechlich verwendete, nicht bereits als
Font-Assets gehaltene Run-Daten stark. Ein LRU begrenzt diese Pins auf
32 Blobs und 64 MiB; uebergrosse Einzelfonts werden nicht gepinnt.
`UiFontCacheStatus` meldet Belegung und Evictions. Geaenderte Textlayouts
werden nach dem Layout ausgewertet; unveraenderte Texte werden nicht pro
Frame neu gerastert. Schriftdateien, CSS und TiltUI bleiben unveraendert.

## Renderbudgets Und Laufpraesentation (Phase 93)

Die laufende Terrain-/LOD-Auswahl verteilt globale Abgleiche nach der
Anfangsladung auf maximal 1024 Kandidaten beziehungsweise weich 750 us pro
Frame. Bewegungen koaleszieren einen Folgedurchlauf, statt den laufenden
Snapshot immer wieder zu starten. Neue Section-Publikationen besitzen einen
separaten 256-Kandidaten-/250-us-Pfad; fehlgeschlagene Uploads und beendete
Fades reihen nur ihre betroffenen Positionen erneut ein. Dispatch prueft
weiterhin aktuelle Epochen, LOD und geladene Daten. Weltwechsel leeren alte
Auswahl-Snapshots. Anfangsladung, Sichtweiten, Hysterese und Geometriequalitaet
bleiben erhalten.

Der gemeinsame Kartenatlas haelt jetzt einen inkrementellen Metadatenindex
der geladenen Sections und vorbereiteten Spalten. Lifecycle-Meldungen werden
auch bei beschaeftigtem Kartenworker und unsichtbarer Karte verarbeitet.
Journal-/Weltwechsel und fehlende Meldungen behalten konservative Fallbacks.
Worker-Ergebnisse mit ueberholtem Quelltoken duerfen neuere Kartendaten nicht
ersetzen. Der Index folgt den geladenen Daten, nicht der gesamten Reisehistorie.
Journal-Aenderungen pruefen den kleineren Bereich aus Aenderungshistorie und
geladenem Index; entfernte Edit-Historie darf die lokale Beitragspruefung
nicht unbeschraenkt verteuern. Der Reset alter Journal-Epochen bleibt erkannt.

Bei `shadow_quality = "off"` entfallen Shadow-Proxy-Komposition und -Upload;
bereits vorhandene Proxy-Meshes werden freigegeben. Beim Wiederanschalten
werfen die Originalmeshes sofort korrekte Schatten. Optimierte Proxies kehren
mit regulaeren Section-Kompositionen zurueck, ohne einen globalen Farbmesh-
Rebuild zu erzwingen. Dauerhaft schattenlose Materialien bleiben schattenlos.

Wachsendes Gras uebernimmt bei einem verspaeteten Upload die inzwischen
sichtbare Groesse seiner alten Darstellung. Pro-Prop-Vertexbereiche erlauben
die Korrektur ohne Hashabfrage fuer jeden Vertex. Previous-Frame-Windgewichte
verwenden die damalige statt der aktuellen Wachstumsgroesse.

`configs/gameplay.toml` besitzt `view_bobbing = true`, im Gameplay-Tab als
`Walking Camera Motion` mit Apply/Reset/Discard bedienbar. Der Schrittrhythmus
folgt der tatsaechlichen interpolierten Bodenbewegung und der vorhandenen
1,25-m-Schrittfolge, mit sanftem Ein-/Ausblenden und einer kleinen Landefeder.
Flug, Schwimmen, Menues und Thirdperson unterdruecken ihn. Blickrotation,
Spielerphysik, Netzwerkposition und Chunk-Fokus erhalten keinen Bob-Offset.

Die optionale GPU-Draw-Diagnostik kopiert Indirect-Daten vor dem jeweiligen
Early-/Late-/Color-Reset pro Kamera. Ein Readback bleibt auf 8 MiB, einen
gleichzeitigen Auftrag und maximal einen Versuch je 60 Frames begrenzt.
Eigene Hoehlen-/Unterwasser-/Rauch-Fullscreen-Submissions werden direkt am
Draw gezaehlt. Private Gradient-/Tilt-Materialien und engineinterne
Fullscreen-Paesse sind weiterhin keine vollstaendig gezaehlte Frameabnahme.
Messungen und verbleibende Abnahmen stehen im
[Phase-93-Bericht](tools/benchmarks/results/render-walking-2026-10-21/README.md).

## Bauteile, Betten Und Kalender (Phase 94)

Die bestehende Blockregistry fuehrt Betten aus Eiche/Birke/Fichte als IDs
71..73 und 24 Bauteile als IDs 74..97. Treppen, Halbplatten und Zaeune gibt
es jeweils aus Eiche, Birke, Fichte, Stein, Erde, feinem Sand, Glas und Eis.
Die drei Treppenstufen, Halbplatten und Zaunteile stammen aus den einzelnen
`assets/terra/blocks/*_{stairs,slab,fence}.json`. `render_kind = "boxes"`
verwendet die vorhandene Materialbibliothek und Section-Batches, keine
separaten GLBs und keine Entity je Teil. Darstellung, Picking und Kollision
lesen dieselben Boxprofile. Zaunarme verbinden sich richtungsabhaengig mit
Nachbarzaeunen und passenden vollen Bloecken. Die normale Bodenbewegung kann
Stufen bis 0,52 m nehmen; ein ganzer Meter bleibt an Auto-Climb gebunden.

Betten verwenden drei wiederverwendete Spezialmodelle mit Bettpfosten,
Kopf-/Fussteil, Kissen und gesteppter Decke. Sie belegen zwei horizontale
Zellen; beide erhalten dieselbe Ausrichtung, Holzart und Farbe. Sechs
Blumen-/Mischfarbstoffe und die Bett-Rezepte liegen einzeln in
`assets/crafting/`. Mit gehaltenem Farbstoff ein Bett benutzen faerbt die
Decke; Survival verbraucht genau ein Item aus dem ausgewaehlten Slot.
Gefaerbte Betten behalten ihre Farbe beim Abbau, Platzieren und Speichern.
Die Farbe erweitert den bestehenden Objektzustand, nicht das Regionsformat.
Seit Phase 98 sind alle drei Grundbetten weiss. Der Katalog zeigt keine
separaten Farbvarianten. Bett plus Farbstoff ergibt im 2x2- oder Werkbankgrid
ein gefaerbtes Bett; auch vorhandene Farben lassen sich so aendern. Die 18
holz-/farbbezogenen Rezepte liegen einzeln als `*_bed_<farbe>.json` vor.

Ohne Farbstoff benutzt man das Bett zum Schlafen. Fruehling/Herbst erlauben
dies ab 19:00, Sommer ab 20:00, Winter ab 18:00 sowie jeweils vor 08:00.
Bei ausgeschalteten Jahreszeiten bleibt der Beginn fest bei 19:00.
Sind standardmaessig alle verbundenen lebenden Nicht-Spectator-Spieler mindestens zwei
Sekunden in unterschiedlichen Betten, setzt der Server die Uhr auf 08:00.
Nach Mitternacht wird kein zusaetzlicher Tag uebersprungen. Bewegung oder
Springen verlaesst das Bett; Entfernung, Tod, Abbau und Disconnect loesen
ungueltige Schlafplaetze. Avatar und Kamera nehmen die Liegepose ein;
die aufrechte physikalische Kapsel bleibt am sicheren benachbarten
Einstiegspunkt. Eine horizontale Schlafkapsel ist nicht implementiert.

Der Kalender wird aus dem gespeicherten `WorldClock.day` abgeleitet und
ueber die vorhandene autoritative Uhr synchronisiert. In `world.toml`
konfiguriert `[seasons]` `enabled`, `spring_days`, `summer_days`,
`autumn_days`, `winter_days`, `start_season` und `transition_days`.
Defaults sind je sieben Tage und seit Phase 98 der letzte Tag als sanfter
Uebergang. Historische groessere `transition_days` werden auf einen Tag begrenzt. Bestehende
Weltmanifeste ohne diese Felder erhalten dieselben Defaults; ihre Seeds,
Welt-IDs und gespeicherten Blockaenderungen bleiben erhalten. Geaenderte
Root-Einstellungen ueberschreiben keine bestehenden Weltmanifeste.

`/gamerule <rule> <value>` aendert die Weltpolitik autoritativ. In der
Dedicated-Konsole entfaellt der Slash; es wird kein Spieler-Aktor benoetigt.
Spieler benoetigen Creative oder Environment-Berechtigung.
`/gamerule list [page]` verwendet den Chatpager mit maximal acht Regeln je Seite.

- `sleep_percentage`: 0..100 Prozent der lebenden, verbundenen,
  nicht teleportierenden Nicht-Spectator-Spieler. Aufrunden auf ganze Spieler;
  auch bei 0 ist mindestens ein Spieler zwei Sekunden im Bett erforderlich.
- `season_swap_interval`: ganze Ingame-Tage 1..3650 je Jahreszeit. Setzt die
  bestehenden vier Saisonlaengen, ohne zweite Kalenderdaten. Aktuelle Jahreszeit
  und Uhrzeit bleiben erhalten; bei Bedarf rueckt das Kalenderdatum vor.
- `game_time_speed`: 1 = normales bisheriges Tempo, 2 = doppelt,
  0.5 = halb so schnell, 0 = Uhr pausiert. Positive Werte 0.0001..1000.

Regeln und Saison-/Zeitpolitik werden im vorhandenen `world.toml` zusammen mit
dem normalen Autosave/Beenden gespeichert. Kalenderfortschritt bleibt im
Regionsindex. Bestehende Regionen, Seeds und Block-IDs werden nicht umgeschrieben.
Alle verbundenen Clients erhalten die Regelpolitik; neue Verbindungen bereits
im Handshake. Client und Server benoetigen gemeinsam Protokoll 55.

Jahreszeiten beeinflussen Laub-/Grasfarben, Beleuchtungsanmutung,
Wetterwahrscheinlichkeiten und die lokale Temperatur. Biomklima und Hoehe
entscheiden weiterhin ueber Regen oder Schnee. Saisonale Oberflaechen
verwenden den gespeicherten Wetterzustand, keine komplette Neugenerierung
oder neue Winterbiome. Unbeladene besuchte Spalten erhalten beim Wiedersehen
eine begrenzte Klima-Nachfuehrung: maximal 3600 normalisierte Wettersekunden
und 8192 Eintraege; das ist keine vollstaendige historische Wetterwiedergabe.
`SeasonSample` stellt gemeinsame Regeln fuer spaeteres Wachstum, Ernten und
Tierverhalten bereit; neue saisonale Tier-/Ernte-Mechaniken sind nicht Teil
dieses Kalenders. Netzwerkprotokoll **52** verlangt passende Client- und
Serverbuilds; Generator- und Regionsversionen bleiben unveraendert.

Schnee wird kontinuierlich ueber die bestehenden Shader abgedeckt und beim
Schmelzen ausgeblendet, statt ab einem Schwellwert einen Blockmaterialwechsel
auszuloesen. Durchgehende Bedeckung uebernimmt auch exponiertes Gras. Die
Schneeroof-Maske ist von Regen getrennt: kahle natuerliche Laubkronen sperren
den Schnee darunter nicht, Spielerdaecher und gesetzte Blaetter dagegen
schon. Natuerliche Laubblaetter werden samt Schatten saisonal ausgeduennt,
die vorhandenen Holzaeste bleiben sichtbar. Fichten bleiben gruen und tragen
Schneekappen. Das sind kosmetische Zustandswechsel; Blattvoxel und ihre
Kollision bleiben erhalten. Der gemeinsame Wetterpuffer besitzt 80 Bytes;
Saison-/Schmelzanimationen erzeugen keine permanenten Mesh-Rebuilds oder
neuen Materials je Zelle.
Nachladende oder entladene hohe Daecher/Kronen aktualisieren nur verfolgte
Besitzer derselben Spalte. Vor dem Upload werden hoechstens zwei neue
Expositionsmasken je System/Frame verglichen; unveraenderte Masken behalten
ihre Revision. Eine Pending-Barriere und getrennte Streamrevisionen weisen
veraltete Worker-Ergebnisse ab. Cacheeintraege verschwinden mit dem letzten
geladenen Besitzer. Blatt-Mikrogeometrie traegt ihre urspruengliche Zell- und
Materialherkunft als CPU-Metadaten, statt sie aus verschobenen Oberflaechen
zurueckzurechnen. Ein blattspezifischer Depth-/Shadow-Prepass verwendet
dieselbe saisonale Vertex-Alpha wie der Farbpass. Bevys Standard-Prepass
beruecksichtigt diese Alpha nicht; ohne die Erweiterung verdeckten unsichtbare
Blattmasken das Holz dahinter bis auf kleine, scheinbar schwebende Fragmente.
Fasern und Flammen behalten ihre bisherigen Alpha-Regeln.
Langfristige Streamingkosten dieser Nachfuehrung bleiben separat zu messen.

Das Creative-Inventar besitzt eine servervalidierte Muelltonne: ein
gehaltener Mausstack, sonst der ausgewaehlte echte Inventarslot, wird ohne
Welt-Drop geloescht. Survival/Spectator koennen diesen Befehl nicht nutzen.
Der alte Action-Abbrechen-Footer ist entfernt, TMI-Inhalte sind zentriert.
Ein geoeffneter Chat bleibt bei Enter, Mausklick und Fokusverlust offen;
nur Escape schliesst ihn. Survival-Abbau eines Grasblocks liefert Erde,
waehrend Creative-Picking weiterhin einen Grasblock auswaehlt.

## Bauteile, Oberflaechendetails Und Pferde (Phase 95)

Halbplatten gleicher Art lassen sich an ihrer inneren Schnittflaeche zu einer
Doppelplatte verbinden, auch bei seitlicher Ausrichtung. Treppen richten ihre
Stufen zum Spieler aus und koennen an Boden, Decke oder Waenden samt
Eckverbindungen gesetzt werden. Sandzaun-ID 95 bleibt nur fuer bestehende
Spielstaende lesbar. Die Farbstoffauswahl verwendet korrigierte Katalog-IDs.

`/season set spring|summer|autumn|winter` setzt den gemeinsamen Kalender auf
den naechsten Beginn der gewaehlten Saison, ohne die Uhrzeit zurueckzusetzen.
Die Serverkonsole verwendet denselben Befehl ohne Slash. Neben der MiniMap-Uhr
steht die aktuelle Jahreszeit. Grafikoption `UI FPS` (Default 60) begrenzt die
Publikation retained UI-Ansichten, nicht die Gameplay-/Renderframerate.

`[blocks]` in `configs/graphics.toml` steuert Materialdetails, analytische
Bump-Normalen, Oberflaechenvariation, Nahrelief und die Qualitaet von
Eichen-/Birken-/natuerlichen Fichtenkronen. Alles bleibt im vorhandenen
PBR-/Section-Meshing mit begrenzten Jobs und geteilten Vorlagen.

`/spawn teramic::horse` erzeugt die neue Tierart. Natuerliche Pferdeherden
kommen auf geeigneten Plainsflaechen vor. 16 Fellvarianten, Leben 20, Hunger
30, Durst 25; ID-stabile Ausdauer 65..200 und Sprungkapazitaet 1,1..2,25 m.
Beduerfnisse und Hindernisspruenge verwenden die vorhandene Simulation;
Reiten, Zaehmen und laufender Reiter-Ausdauerverbrauch sind noch offen.

Phase 95 verwendet Netzwerkprotokoll **53** und erfordert passende Client-
und Serverbuilds. Bestehende Block-IDs, Generator 5 und Regiondateien bleiben
erhalten. Architektur, echte Messwerte, native Bilder und Einschraenkungen:
[Phase-95-Bericht](tools/benchmarks/results/construction-materials-horses-2026-10-21/README.md).

## Halbplatten, Winterdecke Und Bewegung (Phase 96)

Halbplatten koennen unterschiedliche Materialien in derselben Zelle enthalten.
Die urspruengliche Haelfte und ihre Ausrichtung bleiben gespeichert; Survival
verbraucht eine weitere Platte und liefert beide Materialien beim Abbau.
Obere/untere Viertel einer Seitenflaeche setzen waagerechte Platten, die Mitte
senkrechte. Treppen steigen ohne das bisherige fruehe Abbremsen; eine kurze
kameraseitige Hoehenglaettung veraendert weder Koerperkollision noch Serveraim.

Pferde erhalten eine geschlossene, kraeftigere Koerperform, zusammenhaengende
Fellflecken, abgesetzte Hufe und gestufte Maehnen. Ihre Hindernisspruenge
pruefen den ansteigenden Bogen und besitzen erneute Probe-/Landepausen, damit
blockierte Tiere nicht fortlaufend zwischen Springen und Ruhe wechseln.
Blockvorlagen und Terrain-PBR zeigen staerker abgestuftes Stein-/Erdrelief,
feineres Schneeprofil und materialabhaengige Risse. Die Bilder sind eine
gestalterische Annaeherung, keine behauptete exakte 1:1-Modellkopie.

Schnee waechst in acht echten 1/8-m-Stufen: neue ID 98 fuer Stufen 1..7,
bestehender voller Snow Block 5 fuer Stufe 8. Darstellung, Picking und Kollision
verwenden dieselben Hoehen. Die vorhandene autoritative Wetterarbeit speichert
Schnee ueber das normale Regionsjournal. Natuerliche Kronen verhindern Schnee
auf dem Boden nicht mehr; Spielerdaecher bleiben geschuetzt. Gras bleibt unter
der Schneeschicht Gras und seine gruenen Seiten werden weiss. Schnee bleibt im
Winter erhalten; Highland-Niederschlag wird auch im Sommer zu Schnee.
Aktualisierungen sind budgetiert und betreffen bereits geladene Spalten, keine
globale historische Wettersimulation oder neue Weltgenerierung.

HUD, Chat, MiniMap und Hotbar bleiben unter Inventar/Pause/Optionen montiert.
Die Hintergrund-Hotbar verwendet reine TiltUI-Anzeigeslots ohne doppelte
Interaktionsziele. Neu komponierte, noch unsichtbare Chunks behalten ihre
Einblendung auch nach einer Nachbarrevision. Der Wet-/Snow-PBR-Shader liest
echte Materialdeckkraft getrennt von den Klima-Metadaten in Vertex-Alpha.

Aktuelles Netzwerkprotokoll **54** verlangt gemeinsam aktualisierte Client-
und Serverbuilds. Vorhandene IDs, Generatorversion und Regionsformate bleiben
erhalten. Kurztests, native Aufnahmen und konkrete Grenzen:
[Phase-96-Bericht](tools/benchmarks/results/slabs-snow-horses-2026-10-21/README.md).

### Streaming-, UI- Und Schneekorrekturen (Phase 99)

Vorbereitete Terrain-Spalten bleiben bei Live-Aenderungen von Spielregeln,
Kalender oder Zeit gueltig. Der Generator vergleicht ausschliesslich den
Terrain-Vertrag einschliesslich Seed und Welt-ID. Dadurch benoetigen neue
Hoehenabschnitte und Abbau-/Platzierungs-Remeshes keinen Cache-Neuaufbau.

Seit Phase 100 bleibt nur die leichte Inventarhuelle beim Schliessen montiert;
Slots und Itembrowser existieren nur im geoeffneten Inventar. Der komplette
versteckte Slotbaum aus Phase 99 verursachte eine nachgewiesene FPS-Regression.
Tooltips erscheinen vermessen
am Cursor, ausserhalb von Scrollcontainern. Bauablehnungen verwenden die
vorhandenen TiltUI-Toasts oben rechts. Schlafen oeffnet den Chat und blendet
Old Film bis 80% Abdunklung ein; angefangene Nachrichten bleiben erhalten.
Neue Bett-Kopfenden zeigen in Blickrichtung. Der Kalender besitzt eine silberne
Nadel mit dunkler Kontur und optisch zentrierte Himmelsrichtungen.

Schnee waechst ausschliesslich durch Niederschlag, nicht allein durch Winter.
Seine Staerke bestimmt die Akkumulation; bestehender Schnee bleibt im Winter.
Betten, Oefen, Truhen und Werkbaenke behalten keine natuerlichen Auflagen und
keine globale Shader-Schneeschicht. Weltstaende und Formate bleiben erhalten.

Kurztests, native Bilder und konkrete Grenzen:
[Phase-99-Bericht](tools/benchmarks/results/streaming-ui-snow-2026-10-22/README.md).

### Inventar-FPS-Regression (Phase 100)

Der gleiche native Standvergleich steigt allein durch die bedarfsgesteuerte
Inventarvorlage von 63,27 auf 111,09 FPS. Mit 4K, Sichtweite 20, Regen und Meer
steigt der Vergleich von 34,53 auf 68,58 FPS; die Weltgeometrie bleibt erhalten.
Die native Inventarprobe kontrolliert die Freigabe geschlossener Slots und
Tooltips sowie Bilder/Layout beim Wiederoeffnen. Keine neuen Langzeitlaeufe.
[Messbedingungen, Rohlogs und Grenzen](tools/benchmarks/results/inventory-idle-regression-2026-10-22/README.md).

### Backlog, Audiobusse Und Teilblock-Wasser (Phase 101)

[TODO.md](TODO.md) trennt Features und Optimierungen mit stabilen Aufgaben-IDs.
Die [Altstand-Zuordnung](tools/benchmarks/results/technical-backlog-2026-10-22/migration.md)
erhaelt alle bisherigen 100 offenen Eintraege; die neue Liste loescht weder
historische Leistungsregressionen noch ausstehende Abnahmen.

`audio.toml` erhaelt `animals_volume` und `footsteps_volume` (0..1, Default 1).
Der bestehende Umgebungsregler `ambience_volume` bleibt erhalten. Tiere,
Fusskontakte und Umwelt erhalten getrennte geglaettete Gains samt Hallfahnen;
Master/Mute, Apply/Cancel/Reset und Speicherung verwenden dieselbe AudioConfig.
UI-Klicks und Wasserloops werden nicht vom Tier-/Schrittregler beeinflusst.
Alte TOMLs bleiben lesbar; keine zusaetzlichen Audio-Entities oder Aufnahmen.

Natuerliche Mob-Spawns lesen optionale [Spawnfelder](assets/terra/mobs/README.md)
aus den vorhandenen Arten-JSONs. Meer-/See-/Flussgewichte, Tiefen-/Biomfilter
und Herdengroessen sind konfigurierbar, ohne gespeicherte Tiere umzuwuerfeln.
Fehlende Ziel-Sections werden abgewartet. Das erweitert keine Habitattypen,
ersetzt keine Navigation und fuehrt keine neue Populationsverwaltung ein.

Sonnen-/Mondschatten sind auf hoechstens eine Werferhoehe projizierte Laenge
begrenzt. Der Uebergang bleibt glatt; sichtbare Sonne/Mond und Fernschatten
bleiben erhalten. Wasserfallseiten neben Teilbloecken werden nur an deren
wirklichen festen Boxen verdeckt, nicht pauschal am gesamten Nachbarwuerfel.
Formanschluesse erhalten gezielte Wassergeometrieinvalidierung; normale
Fluidticks behalten ihren bisherigen Halo und ihre Budgets.
Vollstaendiges Teilblock-Waterlogging ist damit nicht implementiert.

Item-Vorschauen behalten laufende Screenshot-Readbacks bis zum Callback,
auch bei verzoegertem GPU-Ergebnis. Hohe Wasserfaelle werden am unteren
Becken nicht mehr vom Hoehenfilter der normalen Wasserloops ausgeschlossen.
Alle fuenf Cargo-Pruefungen bestehen, einschliesslich 1703 Tests; native
Menue-, Audio-, Fluid- und kurze Drei-Seed-Kontrollen sind dokumentiert.

Codepfade, Pruefungen und ausdrueckliche Restaufgaben:
[Phase-101-Bericht](tools/benchmarks/results/technical-backlog-2026-10-22/README.md).
Keine neuen Dependency-, Protokoll-, Generator- oder Speicherformatversionen.

### Teilblock-Wasser Und Render-Lifecycle (Phase 102)

Slabs und Treppen koennen Wasser in ihren geometrisch freien Teilvolumina
enthalten. Zufuhr, Gravitation, gebuendelte Wasserpolygone und punktgenaue
Schwimm-/Unterwasserabfragen verwenden die bestehenden Kollisionsprofile.
Eine ersetzte explizite Quelle verliert ihre Source-Eigenschaft; nicht mehr
gespeistes Wasser zieht sich zurueck. FluidCell und alte Speicherformate
bleiben lesbar; beliebige importierte oder bewegliche Meshes erhalten damit
keinen allgemeinen Fluessigkeits-Solver.

Streaming verwaltet vertikale Job-Geschwister ueber einen Spaltenindex.
Die Kartenplanung begrenzt Kandidaten vor dem Sortieren und waehlt bei grossen
Archiven zwischen sichtbarer Rasterflaeche und Katalogiteration. Wasser-only-
Publikationen aktualisieren betroffene Kartenspalten ohne globale Terrain-
Invalidierung. Verlassene Sections verlieren Simulationsarbeit, nicht ihre
gespeicherten Wasserzellen.
Identische/veraltete bestaetigte Section-Deltas setzen keine unnoetige Change
Detection mehr. Tatsaechliche Importe senden SectionEditApplied fuer lokale
Kartenaktualisierung; bestehende Mesh-/Wasserepochen schuetzen echte Aenderungen.

Prop-Winddaten werden pro Template vorbereitet, bekannte Batchgroessen
vorreserviert und entfernte Props direkt gepackt. Geeignete Terrain-/Prop-
Indexbuffers verwenden U16 statt U32 bei identischen Dreiecken. Lichtoption
Off gilt nun auch fuer Fackeln; Cave-/Mist-Claims schuetzen gemeinsam genutzte
Nebel- und Lichtkomponenten bei Qualitaets-/Unterwasserwechseln.

Implementierung, Vergleichsbedingungen, Pruefungen und Grenzen:
[Phase-102-Bericht](tools/benchmarks/results/render-water-2026-10-22/README.md).
Alle fuenf Cargo-Pruefungen, 1734 Tests, der Headless-Server, der native
Teilblock-Wasserlauf und 31 Licht-/AA-/Unterwasseransichten sind bestanden.
Kurze native Messungen mit drei Seeds und Sichtweite 12/20/32 belegen
geringere Kartenplanungs-/Prop-Kompositionskosten, keine allgemeine FPS-
oder Speicherparitaet. Neue Langzeitlaeufe wurden nicht durchgefuehrt.

### Offenes Inventar Und UI-Runtime (Phase 103)

Die Inventarregression wurde im nativen 4K-/20er-Blick reproduziert:
32,79 FPS offen bei 91,78 FPS geschlossen. Der lokale Patch derselben
`tilt-ui-runtime 0.1.0` vermeidet ganze JSON-Root-Kopien pro Feld/Ausdruck,
serialisiert unveraenderte Shared-Stores nicht erneut und cached die
Flow-Sortiertiefe. Teramics Vorschau-Bildbruecke bindet neue/geaenderte Nodes
ueber einen begrenzten Katalogindex statt linearer Suche je Bild und Frame.
Seit Phase 134 liegen diese Runtime-Korrekturen im echten
[TiltUI-Repository](../../tilt-us/tilt-ui/crates/tilt-ui-runtime/README.md),
nicht mehr in einer Teramic-`vendor/`-Kopie. Die folgenden Messwerte beschreiben
weiterhin den damaligen Stand aus Phase 103.

Unveraenderte Szene/Qualitaet: offenes Inventar 32,79 -> 68,38 FPS bei 4K/20,
43,22 -> 90,99 FPS bei 2880x1620/12; kurzer Geh-/Rueckweg 101,43 -> 116,08 FPS.
Echter integrierter Server, Welt-HUD, gleiche Slot-/Bindinganzahl und weiterhin
eine aktive Weltkamera. Kein allgemeiner FPS-/RAM-Gewinn und kein neuer
Langzeitbeleg; Inventar-CPU-Kosten und einzelne Spitzen bleiben messbar.
Messrahmen, gueltige Regen-/Schnee-Nachherproben, ungueltige Schnee-
Inventarversuche und native
Creative-/Survival-Funktionspruefungen stehen im
[Phase-103-Bericht](tools/benchmarks/results/inventory-fps-2026-10-22/README.md).

### Buffs Und Debuffs (Phase 104 / F01)

13 serverseitige Effekte verwenden validierte Einzeldateien unter
[`assets/terra/effects`](assets/terra/effects/README.md): Poison, Freeze,
Bleeding, Rotten, Slow, Speed, Strength, Water Affinity, On Fire,
Fire Resistance, Freeze Resistance, Regeneration und Shield Power.
Dauer, Staerke, Stapelregel und Modifikatoren sind datengetrieben; hoechstens
ein zeitlicher Slot je Art, keine permanent wachsende Effektliste.

Quallenkoerperkontakt erneuert 10 Sekunden Gift. Ofenrauch zeigt Gift nur
waehrend der Exposition, behaelt seinen bisherigen 2-HP/s-Schaden und loescht
beim Verlassen kein unabhaengiges Quallengift. Erfolgreiche Schlafteilnehmer
erhalten 5 Sekunden Regeneration; Abbruch und Nichtschlaefer nicht.
Auch beim Warten auf andere Schlaefer laufen Gift, Atmung und Effektzeiten.
Weitere Effekte sind nutzbar, erhalten damit aber keine erfundenen Wetter-,
Waffen- oder Nahrungsausloeser.

`/effect give teramic::speed 30 1` und `/effect clear [teramic::speed]`
benoetigen Environment-Berechtigung und einen lebenden Survival-Spieler.
Die Dedicated-Konsole verwendet `as <Spieler> effect give ...`.
Der Handshake prueft den kanonischen Effektkatalog; Client und Server muessen
dieselben validierten Definitionen besitzen. Protokoll 56 ist inkompatibel
zu alten Binaries. Spielerdateien migrieren dagegen ausdruecklich nach PLY5;
Timer/Schild bleiben erhalten, Rauchkontakt wird nach Laden neu ermittelt.

Das bestehende HUD zeigt Restzeit, Staerke, Status-Icons und zusammengefasste
weitere Effekte. Gift-/Eisstrukturen liegen direkt auf Herz und HP-Kanal im
bestehenden UI-Material, ohne neuen Fullscreen-Pass. Shield Power zeigt einen
echten Absorptionsvorrat, keine vorgetaeuschte Ruestung. Regeneration verwendet
zwei pinke Herzen und vier wiederverwendete, innerhalb einer Sekunde
aufsteigende/ausblendende Herzsprites. Kleine Fenster verwenden kompakte Cards.
Bildherkunft und Generierungsprompt: [Status-Atlas](src-ui/assets/status-effects-atlas.txt).

Native Kurzpruefung: `target/debug/terra-client --smoke-test-effects`.
Status, Messungen und Grenzen:
[Phase-104-Bericht](tools/benchmarks/results/status-effects-2026-10-22/README.md).

### Ruestungen Und Ausruestung (Phase 105 / F02)

Fuenf Sets mit je Helm, Brustpanzer, Handschuhen, Hose und Stiefeln sowie vier
Accessoires verwenden neun echte, servervalidierte Plaetze. Einzeldefinitionen,
Rezepte, Schutz- und Werteboni:
[Armor And Equipment](assets/terra/items/ARMOR.md).
Klick, Tausch und Shift-Transfer verwenden dieselbe atomare Inventarrevision;
ein Wechsel heilt nicht. Die bisherige Keep-Inventory-Regel bleibt erhalten.
PLY6 liest PLY5 und aeltere Spielerdateien explizit; bestehende Welt-/Block-IDs
und Containerformate bleiben unveraendert. Protokoll 57 prueft beide
Effekt-/Ausruestungskataloge, Client und Server gemeinsam aktualisieren.

Gemeinsame gebuendelte Meshes folgen den animierten Koerpergelenken und werden
auch fuer 3D-Itembilder/Drops verwendet. Das Portraet erneuert sich bei einem
Wechsel mit wiederverwendeten Renderzielen. Die silberne Ruestungsleiste zeigt
Schutz, nicht Haltbarkeit; Shield Power bleibt separat. HP und Stamina beziehen
sich auf ihre tatsaechlichen Maxima. Nur der erste Effekt der Kartenreihe liegt
auf Herz/HP und aktiviert gegebenenfalls die Regenerationsherzen; andere Effekte
wirken weiter, ohne sich optisch darauf zu mischen.

Native Kurzpruefung: `target/debug/terra-client --smoke-test-equipment`.
Nachweise und Grenzen:
[Phase-105-Bericht](tools/benchmarks/results/equipment-2026-10-22/README.md).

### Capes Und Crusher (Phasen 106/107 / F03)

Die Ruestungsanzeige nutzt den schmalen Stamina-Rahmen mit eingelassenem,
ausgefuelltem Schild und silbernem Schutzkanal. Capes ergaenzen den zehnten
Ausruestungsplatz: Leather, Iron, Gold, Diamond und Ruby beginnen weiss und
lassen sich mit einem Dye faerben, ohne ihre Stufe zu aendern. Stoff,
Materialbeschlaege, Werteboni, Replikation und gespeicherte Farbe verwenden
die bestehende Ausruestungsarchitektur.
Der Stoff liegt ausschliesslich am Ruecken; zwei verbundene, geglaettete
Segmente reagieren auf Laufgeschwindigkeit und den vorhandenen Wetterwind.
Fremde Atlas-Rahmenpixel oberhalb der Ruestung werden maskiert; Shield Power
erscheint als schmales blaues Inlay, ohne einen zweiten Rahmen ueber dem HUD.
Gift tropft zeitversetzt vom Herz, weiterhin nur als erster Effekt der Reihe.

Der horizontale Zweiblock-Crusher verarbeitet Blumen zu Farbstoff und Mineralien
zu sechs Staubarten. Zwei private Inventarslots, atomare Transfers, vier Sekunden
Verarbeitung, Fortschritt und Too-Many-Items-Rezepte sind integriert. Einzelne
Rezepte liegen in `assets/crushing/`; der Generator fuer das gemeinsame
3D-Modell ist `tools/assets/build_crusher.mjs`. Seit Phase 107 muss eine
Handkurbel (Block 100) rechts am Zahnrad montiert werden. Inventar befuellen,
schliessen und an der Kurbel Platzieren/Benutzen gedrueckt halten; Loslassen
stoppt den Antrieb. Shift behaelt angrenzendes Platzieren bei. Kurbeln und
Zerkleinern haben getrennte, budgetierte Klanganteile; Spaene fallen zum
Auswurf. Die Verarbeitung pausiert ungeladen, nutzt begrenzte Arbeitsbudgets
und erzeugt keine Terrain-Rebuilds. Aktiver Antrieb wird nicht gespeichert.
Das Spielerinventar nutzt mehr Fensterhoehe und kompakte Hoehenregeln.
Slot-Tooltips reagieren direkt auf Hover; leere Slots bleiben stumm. Picking-
Position, Monitor-DPI und UI-Skalierung werden getrennt behandelt, ohne
gegenseitige Layoutkorrekturen der beiden Positionierungspfade im Stillstand.

Protokoll 60, PLY7 mit expliziter PLY6-Migration und RGA1 mit RGA0-Leser.
Bestehende Block-/Welt-IDs, Generator und Abwaertskompatibilitaet bleiben erhalten.
Details, native Kurzpruefungen und Grenzen:
[Phase-106-Bericht](tools/benchmarks/results/capes-crusher-2026-10-22/README.md).
Aktuelle Korrekturen und Pruefungen:
[Phase-107-Bericht](tools/benchmarks/results/manual-crusher-2026-10-22/README.md).

### Materialausbau (Phase 108)

Die Farbstoffpalette umfasst jetzt 13 Farben: Red, Yellow, Blue, Orange, Green,
Purple, White, Black, Lime, Pink, Gold, Silver und Light Blue. Alle Farben gelten
fuer Betten und Capes. Bisherige Farb-Tags und Bett-Ausrichtungen bleiben erhalten;
die sechs angehaengten Farben erweitern die fuenf Cape-Stufen auf 65 Varianten.
Neue Misch-/Pigmentrezepte und alle sechs Materialstufen fuer Schwert, Axt und
Spitzhacke sind einzelne JSON-Dateien und im Rezeptbrowser sichtbar.
[Rezepte und Materialstufen](assets/terra/items/DYES_AND_TOOLS.md).

Die Werkzeuge nutzen ein gemeinsames, facettiertes Modell fuer Vorschau und
Welt. Ausgeruestete Schwerter sind 1,5 m, Aexte und Spitzhacken 1,25 m lang;
Inventarbilder passen dieselbe Geometrie in den Slot ein, lose Drops sind
bewusst verkleinert. Die 69 bisher abgerundeten Blockdefinitionen nutzen gerade
Kanten. Oberflaechendetails und optionale Mod-Bevels bleiben davon unabhaengig.
Mixer und Kinetik-System sind unter F25/F26 geplant, nicht bereits implementiert.
Pruefungen, native Bilder und Hoerprobe:
[Phase-108-Bericht](tools/benchmarks/results/materials-crusher-2026-10-23/README.md).

### Ernte Und Anbau (Phase 109 / F04)

Weizen, Mais und Salat verwenden das bestehende Prop-System mit acht sichtbaren
Wachstumsstadien und gemeinsam gebuendelter 3D-Geometrie. Samen sind durch das
Abernten von wildem Gras, Blumen und kleinen Pflanzen erhaeltlich. Mit der
vorhandenen Platzieren-/Benutzen-Taste auf eine freie Gras-/Erdflaeche saeen;
die Autoritaet bereitet dabei Ackerboden (neue Block-ID 101) vor. Ein Feld
belegt eine Blockflaeche, Mais/Weizen brauchen zwei freie Bloecke darueber.
Unreife Pflanzen geben einen Samen zurueck; reife Pflanzen geben Ertrag und
mehrere Samen. Neue Aussaat erfolgt mit denselben normalen Inventarstapeln.

Wasser innerhalb von vier Bloecken und exponierter Regen befeuchten Pflanzen;
ohne Nachschub trocknet der Boden. Tageslicht und weiche SeasonSample-Gewichte
bestimmen das Wachstum. Ohne Licht, bei ausgetrocknetem Boden und im Winter
wachsen diese drei Feldfruechte nicht. Die Basiszeiten bei guten Bedingungen
sind 600/780/480 Sekunden fuer Weizen/Mais/Salat, saisonale Raten kommen hinzu.
Mais, Salat und herstellbares Brot verwenden das vorhandene Essenssystem.
Rezepte und Gegenstaende erscheinen im Too-Many-Items-Browser.

Einzeldefinitionen stehen in `assets/terra/farming/`, `assets/terra/props/`,
`assets/terra/items/` und `assets/crafting/`. Autoritative Timer, Feuchtigkeit
und die beim Saeen festgelegte Wachstumsdauer werden gespeichert. Bestehende
Regionen lesen weiterhin RGA1 und aeltere Formate; neue Regionen verwenden RGA2.
PLY7, Weltformat 2, Generator 5 und bestehende Block-IDs bleiben erhalten.
Netzwerkprotokoll 61 erfordert zusammen aktualisierte Clients und Server.

Gefressene Feldfruechte beginnen unter derselben Pflanzen-ID erneut zu wachsen.
Wildes Futter kehrt nach kontrollierter Abklingzeit unter derselben ID zurueck.
Hoechstens 128 Feldfruechte und 128 Futter-Timer pro Sekunde werden verarbeitet;
ungeladene Pflanzen und pausierte Zeit wachsen nicht weiter. Fortschritt allein
erneuert keine Terrain-/Wasser-Meshes oder Prop-Batches. Fackellicht-Farmen und
Offline-Wachstum sind nicht Teil dieser Feldanbau-Fassung.

Kurze native Funktions-/Bildpruefung: `target/debug/terra-client --smoke-test-farming`.
Werkzeug-Griffwinkel: `target/debug/terra-client --smoke-test-hands`.

### Spinnen Und Pflanzenuebergaenge (Phase 111)

`teramic::small_spider`, `teramic::medium_spider` und `teramic::large_spider`
erganzen die gemeinsame Mob-Registry um unterschiedliche Koerpergroessen,
natuerliche Tag-/Nacht-/Tiefenregeln, Klettern, Abseilen und Gift. Zehn
verankerte Spinnennetz-Varianten bleiben normale auswaehlbare, abbaubare Props
mit gemeinsamer Section-Geometrie. Die Einzeldefinitionen liegen unter
`assets/terra/mobs`; die bestehende `/spawn`-Vervollstaendigung kennt die Arten.

Aussaat auf bereits vorhandenem Ackerboden bleibt nach Save/Reload gueltig.
Ueberhaengende Kulturpflanzen blockieren Nachbarfelder beim Pflanzen nicht.
Ackerboden zeigt den Wachstumsbalken der Kultur oder `Keine Samen erkannt`.
Sichtbare Stufen wechseln ueber 2,4 Sekunden mit Groesseninterpolation und
komplementaerer Rasterueberblendung, ohne transparente Einzelpflanzen.

Die Minimap korrigiert ihren lokalen Bodenbezug, ohne die FullMap beim
Verschieben zurueckzusetzen. Geschlossene Hoehlen behalten beim Tauchen ihre
dunkle Beleuchtung, erhalten kein direktes Sonnen-/Mondlicht und sind im
Hintergrund 35 Prozent dunkler. Fackeln bleiben lesbar; Daemmerungsuebergaenge
besitzen keine abrupte Helligkeitsluecke. Groessere Shader-Schwimmspuren und
ueberarbeitetes Wasser-Foley ergaenzen die Wasserbewegung.

Netzwerkprotokoll 63 verlangt gemeinsam aktualisierte Clients/Server.
Weltformat 2, Generator 5, PLY7 und RGA2 bleiben erhalten. Die angefragten
Rohr-/Ventil-, Wasserhahn- und 2x2-Quellen-Features stehen unter F27/F28/F29 in
TODO.md, nicht als schon implementierte Technik. Architektur, Bilder,
Pruefungen und Grenzen: [Phase 111](tools/benchmarks/results/spiders-farming-light-2026-10-23/README.md).

### Zaehmung, Effekte Und Interaktion (Phase 113)

Schafe, Kuehe, Schweine, Pferde und alle drei Spinnenarten lassen sich mit
artspezifischem Futter ueber die konfigurierte Platzieren-Taste zaehmen. Echtes
Futter wird serverseitig verbraucht; Vertrauen und Besitzer werden gespeichert,
repliziert und im Ziel-HUD angezeigt. Folgen und Leerhand-Befehle sind seit
Phase 114 vorerst deaktiviert und fuer eine spaetere Pet-UI vorgesehen. Alte
gespeicherte Follow-Werte bleiben lesbar, loesen aber keine Bewegung aus.
Gezaehmte Tiere bleiben
vom natuerlichen Spawnlimit ausgenommen. Reiten und Zucht sind weitere Aufgaben.

Hoehlenspinnen priorisieren einen verankerten Netzbau und suchen danach bei
Hunger innerhalb von 20 Metern um sich und ihr Netz nach Nahrung. Sie haben
50 Nahrung und keinen Durst. Entfernte Netze koennen neu gebaut werden;
unpassende Netzanker lassen die Spinne weitersuchen. Fuettern und Befehle
erhalten Wand-/Decken- und Seilbefestigungen.

`/effect teramic::nightvision 2 100` wirkt auf den eigenen Spieler;
`/effect @Name teramic::nightvision 2 200` auf einen berechtigten anderen
Spieler. Namen mit Leerzeichen werden als `@"Name Mit Leerzeichen"` angegeben.
Hilfe, Konsole, Vervollstaendigung und die bisherigen give/clear-Formen nutzen
dieselbe Registry. Night Vision erweitert die bestehenden Effekte, ohne deren
IDs zu verschieben.

Creative-Pick-Block erzeugt ein Item statt 128. Truhengeraeusche folgen
bestaetigten Oeffnungs-/Schliesswechseln im vorhandenen raeumlichen Audiomixer.
Geladene Block-Edits bekommen bevorzugte Einzel-Remesh-Jobs und priorisierte
Render-Publikation, normale Spaltenjobs geben zwischen Sections den Worker frei.
TiltUI buendelt die Zielaufloesung verschachtelter Slots; der Inventarkatalog
wird wiederverwendet statt bei jeder Oeffnung komplett aufgebaut. Eine einzelne
unsichtbar vorgewarmte Inventaroberflaeche bleibt pro Welt erhalten; geschlossen
sind ihre Bindings geparkt. Neue CSS-Slot-Scopes teilen sich eine Datenerfassung
je Style-Pass statt je Slot erneut die komplette UI zu durchsuchen.
Im kurzen nativen Vergleich bei 2880x1620/Sichtweite 12 sank die erste
Inventaroeffnung von 325 auf 65 ms, der laengste Oeffnungsframe von 166 auf
25 ms. Die dauerhafte offene UI-FPS blieb etwa gleich; im zusaetzlichen
4K-/Regenlauf besteht weiterhin eine kleinere messbare Oeffnungsspitze.

Client und Server gemeinsam auf Protokoll 65 aktualisieren. RGA3 erweitert die
vorhandenen Regionsdaten mit Vertrauen und Netzzuordnung; alte Regionen bleiben
lesbar. PLY7, Weltformat 2 und Generatorversion 5 bleiben erhalten. Umsetzung,
Futterliste und Pruefungen:
[Phase 113](tools/benchmarks/results/taming-effects-interaction-2026-10-23/README.md).

### Blockoberflaechen, Mob-Balken Und Netzkonstruktionen (Phase 118)

Grasoberseiten bilden wieder eine durchgehende Flaeche statt wiederholter
eingesenkter Rasterkacheln. Dezente weltverankerte Materialdetails ersetzen
das harte prozedurale Steinraster. **Graphics > Block Surface Shader** schaltet
den zusaetzlichen Oberflaechenshader aus; es verwendet weiterhin
`blocks.material_detail` in der vorhandenen `graphics.toml`. Texturen,
Wetter-/Schneezustaende und logische Bloecke bleiben erhalten, gemeinsame
Materialhandles werden wiederverwendet.

**Gameplay > Mob Health Bars** bietet `Looking At` (Standard), `All In View`,
`All Time` und `None`. Die ersten beiden beachten echte Terrain-/Modell-
Sichtverbindungen; `All Time` zeigt projizierte Balken auch durch Waende.
Freundliche Tiere/gezaehmte Mobs sind gruen, feindliche rot. Maximal 64 nahe
Balken innerhalb 64 m, acht fair rotierende Sichtpruefungen pro UI-Tick und
eine begrenzte Sichtablage halten die Arbeit kontrolliert.

Stalker-Details verwenden getrennte konstruktive Fugen statt koplanar
ueberlappender Flaechen. Spinnen bauen vier groessere raeumliche Netzvarianten
mit Fangflaechen und Halteseilen, etwa 7..20 m freie Spannweite. Bestehende
Prop-Datensaetze, Section-Batches, Besitzersektionen und Entfernungsdaten
bleiben die Grundlage. Echte Felsanker, freie Seilverlaeufe, geladene Nachbarn
und Durchgangsraum werden geprueft; auch Abbau/Gegenanker ueber Section-Grenzen.

Neue Welten verwenden Hoehlenstrukturrevision 7: Das bestehende breite
Hoehlennetz wird regional geclustert, mit groesseren massiven Zwischenzonen.
Ausgewaehlte Kammern werden nicht verkleinert und aktive Gaenge behalten ihre
Revision-6-Breite. Bestehende Saves bleiben bei Revision 0..6.

Blumen sterben nach ihrer stabilen saisonalen Schwelle in zehn echten Sekunden
ab: Braunfaerbung, Schrumpfen an der Wurzel und vollstaendiges Verschwinden.
Eine feste 256er-Zeittabelle statt Pflanzen-Entities steuert Shader, Picking
und Bienen. Kein saisonaler Mesh-Rebuild; Fruehlingsrueckkehr und bereits
dormante Reloads bleiben konsistent. Protokoll 69 erfordert gemeinsam
aktualisierte Clients/Server. Block-/Welt-IDs und Speicherformate bleiben gleich.
Pruefungen, Aufnahmen und Grenzen:
[Phase 118](tools/benchmarks/results/render-health-webs-2026-10-24/README.md).

### Pferde Reiten, Schnee Und Autosaves (Phase 117)

Eigene gezaehmte, erwachsene Pferde lassen sich mit der bestehenden
Platzier-/Tierinteraktion besteigen, sofern kein passendes Futter angeboten wird.
WASD steuert, Sprint beschleunigt, Springen verwendet die feste individuelle
Sprunghoehe, Shift steigt an einer freien, geladenen Bodenstelle ab.
Ein Pferd hat genau einen Reiter. Server und Vorhersage verwenden denselben
Voxel-/Pferdemotor; Pferdeausdauer ersetzt waehrenddessen den Stamina-Wert im HUD.
Sitzpose, Pferd und Kamera teilen dieselbe Darstellung. Aufsitzen ist kein
Follow-Befehl; die spaetere Pet-Bedienoberflaeche bleibt offen.

Schnee sammelt sich mit stabilen individuellen Blockraten statt gleichmaessig
pro Chunk. Erst die dritte echte Schneeschicht verbirgt Bodenprops reversibel.
Regulaere Welten verwenden nur lokale Schneezustaende, keine globale weisse
Wetterauflage. Ausschliesslich die explizite lokale Saison-Galerie darf diese
Kosmetik per `CosmeticSnowPreview` anfordern; Server/Replicas ignorieren sie.
Herbstblumen blenden nach stabilen Pflanzenraengen gestaffelt aus und kehren
Richtung Fruehling zurueck. Bereits frueher als entfernt gespeicherte Props
werden nicht pauschal wiederhergestellt: alte Wetterentfernung und echte
Spielerentfernung sind dort nicht zu unterscheiden.

Neue Welten verwenden Hoehlenstrukturrevision 6 mit breiteren bestehenden
Verbindungstunneln. Bestehende Saves behalten ihre gespeicherte Revision 0..5
und ihre Spielerbauten. Generatorversion, Block-IDs und Welt-IDs bleiben gleich.
Der Stalker-Atem verwendet bearbeitetes CC0-Atem-Foley statt der bisherigen
Rausch-/Sinussynthese. Natuerliche Hoehlen-Stalker und kinematische
Spinnen-Kantenwechsel erfuellen jetzt dieselben Speicherinvarianten wie die
uebrige Population, ohne die Spielstandvalidierung abzuschalten.

Client und Dedicated Server gemeinsam auf Protokoll 68 aktualisieren.
Der aktuelle Reiter-/Ausdauerstand ist Sitzungszustand; Besitz, Fell,
Kapazitaet, Sprungvermoegen und die letzte autoritative Tierposition bleiben
ueber die vorhandenen Weltdaten erhalten.
Umsetzung, kurze native Aufnahmen, Regressionen und Grenzen:
[Phase 117](tools/benchmarks/results/riding-snow-autosave-2026-10-24/README.md).

### Stalker, Kletterwege Und Streaming (Phase 116)

Spinnen verwenden echte kontakt-/kollisionsgepruefte Wege ueber Boeden, gestufte
Waende und Decken. Der Oberflaechen-Routencache ist begrenzt; Zielwechsel und
blockierte Wege loesen keine unbeschraenkten Suchschleifen aus. Abseilen bleibt
mit der vorhandenen Seildarstellung verbunden. Pet-Folgen bleibt deaktiviert.

Jedes Tier besitzt ein stabiles, beim Spawn aus seiner persistenten Identitaet
bestimmtes Geschlecht. F06 paart nur erwachsene Maennchen und Weibchen derselben
Art und desselben Besitzers; Nachwuchs erhaelt wieder eine unabhaengige Auswahl.
Alte Tiere und Familien bleiben lesbar, keine Tier-ID wird ersetzt.

`/spawn teramic::stalker` ergaenzt den seltenen, ausschliesslich unterirdischen
Stalker. Seine begrenzte Navigation verwendet reale geladene Hoehlengaenge;
Beobachten, Verfolgen, Lichtabwehr und Wegziehen werden serverseitig entschieden.
Der Griff verwendet die vorhandene kollisionsgepruefte Spielerbewegung, keine
Flugerlaubnis oder Teleports. Rinden-/Moosmodell, bewegliche Arme/Haende,
Augen und vier situationsabhaengige Geraeusche ergaenzen den vorhandenen Mobpfad.

Die gemessene Terrain-Queue berechnet Sortierschluessel jetzt einmal je Eintrag.
Wetter-/Kuestensnapshots werden lokal invalidiert und fuer unveraenderte oder
leere Geometrie nicht erneut erstellt. Niederschlag folgt dem Boden-/Meerklima
statt der Kamera-Flughoehe; Fruehlingskuesten sind maritim temperiert, waehrend
Winter, Highlands und ausdrueckliche Schnee-Kommandos erhalten bleiben.

Client und Dedicated Server gemeinsam auf Protokoll 67 aktualisieren.
Nativer 120-s-Flugvergleich auf RX 6950 XT: bei 4K/Regen/Sichtweite 20
39,18 -> 71,33 FPS, p95 43,87 -> 25,16 ms. Der 1080p-Clear-Vergleich bei
Sichtweite 12 bleibt mit 123,61 -> 121,73 FPS auf aehnlichem Niveau.
Keine reduzierte Grafikqualitaet, aber auch keine behauptete Langzeitabnahme.
Messwerte, Screenshots, Regressionen und Grenzen:
[Phase 116](tools/benchmarks/results/stalker-climbing-streaming-2026-10-24/README.md).

### Nachwuchs, Slabs Und Spinnen (Phase 115)

Die orange Baukontur ist heller und zwei physische Pixel breit. Ein behaltenes
Kantenmesh wird im WGSL-Vertexshader zu schmalen Kantenstreifen erweitert;
die Deckkraft pulsiert ohne Geometrie-Rebuild. Vollbloecke bleiben ohne Vorschau.

Slabs stehen beim Treffer auf Boden-/Deckenmitte hochkant; die aeusseren 15
Prozent bleiben horizontal. Unterschiedliche Materialien lassen sich nach
anvisierter Haelfte einzeln abbauen. Resthaelfte, Abbauanimation, Drop und
Hover-HUD stimmen damit ueberein. Identische Paare behalten gemeinsamen Abbau.

Wilde und gezaehmte Spinnen bauen auch auf unebenen Hoehlenflaechen Netze und
suchen anschliessend Nahrung. Dauerhafte geschuetzte Tagesruhe ist korrigiert.
Gezaehmte Spinnen handeln autonom ohne Spielerziele; Pets folgen weiterhin nicht.

F06 ergaenzt Nachwuchs fuer gezaehmte Schafe, Kuehe, Schweine, Pferde und
Spinnen. Zwei gesunde erwachsene Tiere derselben Art und desselben Besitzers
in hoechstens 5 m Abstand jeweils einmal zusaetzlich mit geeignetem F05-Futter
fuettern. Das Paar bekommt bei sicherem Platz ein Jungtier und 300 Sekunden
Abklingzeit. Nachwuchs waechst weich ueber 1200 aktive Wachstumssekunden;
Groesse, Animation, Zielvolumen und Unterstuetzung passen sich an. Das Ziel-HUD
zeigt Wachstum und Zuchtzustand. Eltern, Besitzer, Uhren und eine nicht
wiederverwendbare Geburtsnummer bleiben gespeichert und repliziert.

Client/Server gemeinsam auf Protokoll 66 aktualisieren. Neue Familienregionen
verwenden RGA4, Welten mit Geburtszaehler IDX7. RGA3/IDX6, die bisherige
Creature-Struktur, PLY7, Weltformat 2 und Generatorversion 5 bleiben lesbar
beziehungsweise unveraendert. Regeln, Grenzen und Pruefungen:
[Phase 115](tools/benchmarks/results/babies-slabs-spiders-2026-10-23/README.md).

### Platzierungskonturen Und Pet-Befehle (Phase 114)

Nur Bauten mit Platzierungsausrichtung zeigen eine Vorschau, einschliesslich
Treppen, Slabs, Moebeln, Rohren und Wandfackeln. Die Kontur besteht aus echten
Modellkanten ohne gefuellte Flaechen und ohne Triangulierungsdiagonalen. Ein
einziges wiederverwendetes Linienmesh bleibt im Cache; nur die Deckkraft der
orangefarbenen Kontur pulsiert weich. Normale Vollbloecke behalten ihre
Platzierungslogik, erhalten aber keine Vorschau, auch nicht im Debugmodus.

Gezaehmte Tiere warten vorerst ohne Folge-/Warteumschaltung per leerer Hand.
Vertrauen, Besitzer und gespeicherte Weltstaende bleiben erhalten. Das Ziel-HUD
zeigt keinen aktuell nicht bedienbaren Begleitermodus; F33 beschreibt die
spaetere Befehls-UI. Protokoll 65 und Speicherformate bleiben unveraendert.
Umsetzung, Regressionen und native Bildpruefungen:
[Phase 114](tools/benchmarks/results/placement-outlines-pets-2026-10-23/README.md).

### Platzierung, Hoehlenschalen Und Tiernavigation (Phase 112)

Die Acker-/Kultur-HUD zeigt Bodenfeuchte und Wasserbedarf aus dem passenden
autoritativen Pflanzenzustand. Eine Kollision der alten Pflanzen-ID zwischen
Art und X-Koordinate ist behoben; alte gespeicherte IDs bleiben erhalten.

Grosse Hoehlen laden ihren realen Dach-/Bodenbereich aus vorbereiteten
Spaltendaten mit. Neue Weltvertraege nutzen Hoehlenrevision 5 fuer organischere
Raumkonturen, Felsvorspruenge und kleine Auslaeufe im vorhandenen Wasserfluss.
Alte Revisionen 0 bis 4 werden nicht migriert. Hoehlen-Hintergrundhelligkeit
steigt gegenueber Phase 111 um 12,5 Prozent, Fackel-Streulicht um 30 Prozent
ohne grelleren Nahbereich. Mineralsegmente sind blockig und pruefen ihre echte
Tragkette statt beliebiger benachbarter Abbaupositionen.

Tierbewegung prueft den durchlaufenen Raum, Hindernisse, Formblock-Untergrund
und Wassereintritt. Spinnen besitzen mehr Entscheidungen und sind bei Jagd
und Flucht physisch 60 Prozent schneller. Ihre Baender sind dunkelbraun.
F1 + E ergaenzt F3 + E fuer Absicht, Ziel und vorhandene Pfadpunkte des
anvisierten Tiers. Die Daten werden gezielt angefordert und begrenzt
repliziert, nicht fuer jeden Mob permanent gesendet.

Ausrichtbare Bauten haben eine echte Modell-Platzierungssilhouette aus dem
gemeinsamen Server-/Client-Resolver mit einem einzigen wiederverwendeten
Mesh. Truhen bekommen weiche Deckelanimationen anhand berechtigter Betrachter,
auch Doppeltruhen und fuer andere Spieler. Crusher und Vogelscheuche erhalten
echtes Laternenlicht im unveraenderten lokalen Lichtbudget.

Protokoll 64 erfordert gemeinsam aktualisierte Clients/Server; PLY7/RGA2,
Weltformat 2 und Generator 5 bleiben erhalten. Nachweise und Grenzen:
[Phase 112](tools/benchmarks/results/building-farming-caves-2026-10-23/README.md).

### Farming-Werkzeuge Und HUD (Phase 110)

Giessekannen, Hacken und Sensen gibt es in Holz, Eisen, Gold, Diamant und Rubin.
Die konfigurierte Platzieren-/Benutzen-Taste bereitet freien Gras-/Erdboden vor,
fuellt Kannen an echtem Wasser auf, bewaessert Pflanzen oder erntet reife Kulturen
mit der Sense. Holz/Eisen/Gold ernten 3x3, Diamant/Rubin 5x5 Felder auf gleicher
Hoehe. Reichweite, Sicht, Inventarquelle und Drop-Kapazitaet werden serverseitig
geprueft; unreife Kulturen bleiben stehen. Sensen schneiden sichtbar seitlich,
Kannen kippen beim Benutzen. Die Arbeitskante zeigt in die Welt statt quer zur
Ego-Kamera.

Kannen halten je Stufe 8/12/16/20/24 Einheiten; eine Bewaesserung setzt die
gespeicherte Feuchtigkeit auf voll und verbraucht eine Einheit. Der Vorrat
gehoert derzeit zur Sitzung und muss nach neuem Verbinden aufgefuellt werden.
15 einzelne Werkzeugdefinitionen und 16 Rezepte verwenden die vorhandenen
Kataloge und Too Many Items. Eine dekorative Vogelscheuche mit Hut, Schal,
Stroh und Laterne ist platzierbar; Vogel-KI und Laternenlicht sind noch nicht
implementiert. Ackerboden besitzt eigene 512px-Top-/Seitentexturen und drei
75-mm-Furchen, ohne neue logische Blockgroesse.

Das Ziel-HUD zeigt echten Wachstumsfortschritt als Prozentbalken. Einmal pro
Sekunde sendet die Autoritaet maximal 256 nahe Pflanzenwerte. Der Jahreszeiten-
zeiger ist eine gefuellte silberne Pfeilspitze mit dunklem Rand. Strg + Klick
auf die Creative-Muelltonne leert persoenliche Inventarinhalte einschliesslich
Cursor, Ausruestung und Crafting-Reservierungen, nicht Weltcontainer.
CreatureStates werden auch vor dem abgeschlossenen Weltbeitritt konsumiert,
statt in Lightyears Queue unbehandelt zu bleiben. Protokoll 62 erfordert
zusammen aktualisierte Clients und Server; Speicherformate bleiben PLY7/RGA2.
Details, native Bilder und Pruefungen:
[Phase-110-Bericht](tools/benchmarks/results/farming-tools-2026-10-23/README.md).

### Mob-Bewegung, Reiten Und Streaming (Phase 119)

`X` steigt vom Reitobjekt ab; `Shift` sprintet mit dem Pferd. Pferd und Reiter
benutzen eine gemeinsame Sprungphase und denselben Sattel-Bezugspunkt.
Beschleunigung/Bremsen sind zeitbasiert, entfernte Reitposen interpolieren ihre
wirklich angezeigte Rotation weiter. `F1 + H` zeigt reale Mob-Kollisionsboxen
und die Blickrichtung, ohne das Textoverlay einzublenden.

Landtiere pruefen reale Teilkontakte beim Absteigen und lassen sich aus
bestehenden Kollisionen nur auf einen nachweislich freien lokalen Weg befreien.
Spinnen pruefen beim Klettern ihre gedrehten Koerperausmasse. Die natuerliche
Hoehlen-Spawnchance steigt relativ um 10 Prozent fuer Stalker und 25 Prozent
fuer Medium/Large Spiders; die bestehenden Populationslimits bleiben bestehen.

**Gameplay > Mob Health Bars > Edit Bars** oeffnet vier Sichtmodi und
Farbwaehler fuer freundliche/feindliche Balken sowie deren Schrift. Live-Mobs
auf Mini-/FullMap warten nicht mehr auf Detail-Kartenkacheln. Bekannte Decken
trennen weiterhin Hoehlenstockwerke; unbekannte Sections sind keine Decke.

Eine begrenzte deduplizierte Prop-Auswahl ersetzt globale LOD-Durchlaeufe ueber
leere Sections. Echte Edits werden vorgezogen und neue Pflanzen auch bei
gleichzeitigen Wasser-Publikationen aufgenommen. Reine LOD-Auftraege verwenden
gueltige Dach-Exposure-Daten erneut. Ranken brauchen eine lueckenlose reale
Deckenverbindung, Bodenpflanzen tragfaehigen Boden; voruebergehend ungeladene
Nachbarn verursachen keine dauerhafte Pflanzenentfernung.

Messbedingungen, kurze native Pruefungen, Regressionen und Grenzen:
[Phase-119-Bericht](tools/benchmarks/results/mob-movement-streaming-2026-10-24/README.md).

### Pets, Ganzbaumfaellen Und Spielerorte (Phase 120)

**F33:** Sneak + Platzieren auf dem eigenen gezaehmten Tier oeffnet ein
dunkel-goldenes Radialmenue mit Modellportraet, Herzen und Follow/Rest/None.
Die bestehenden Belegungen sind umbelegbar (Standard Shift + B). Follow nutzt
die vorhandene Navigation zu einem erreichbaren anwesenden Besitzer innerhalb
24 m, ohne Teleport. Rest wartet mit weiterlaufender Physik, None erlaubt das
normale autonome Verhalten. Besitzer, lebendes Tier, Reichweite, geladener
Sichtweg und Reitzustand werden auf dem Server geprueft. Alte Follow-Felder
werden nicht aktiviert; explizite Auftraege bleiben ueber RGA5 gespeichert.

**F08:** Beim Survival-Abbau eines natuerlichen Stammfusses mit Sneak dauert der
gesamte Baum 0,5 s je Holzblock. Exakte Generator-Strukturzuordnung schuetzt
Spielerbauten und Nachbarbaeume. Holz, gestapelte Drops und verzoegerter
Laubzerfall werden budgetiert verarbeitet. Noetige obere Stammsections werden
ueber das vorhandene Streaming nachgeladen. Einzelblock-/Creative-Abbau bleibt
unveraendert; der regulaere Weltabschluss beendet angenommene Faellarbeiten.

Wasserfall-Ecken verwenden gemeinsame versetzte Kanten. Echte Fluessigkeit
behaelt neben Slabs/Treppen dieselben Oberflaechenkanten, auch diagonal ueber
vier Chunks; Wasser und feste Restgeometrie werden gegenseitig verdeckt.
Simulation, Update-Budgets, Materialien und Shader bleiben erhalten.

Spielerorte werden pro UUID und Welt mit Kapselmitte, Blickrichtung und erlaubtem
Flugzustand gespeichert. Ortsupdates sind Metadaten und invalidieren kein Terrain.
Beim Laden werden echte Blockformen und geladener Kollisionsraum geprueft;
begrenzte Korrekturen duerfen keine neuen Hindernisse durchqueren. Alte Saves
ohne PLY8-Ortsdaten verwenden weiter den normalen sicheren Spawn. Welt-/Block-IDs,
Generator- und Welt-Saveversion bleiben gleich; Clients und Server benoetigen
gemeinsam Protokoll 70.

Pruefungen, Aufnahmen und Grenzen:
[Phase-120-Bericht](tools/benchmarks/results/pets-trees-water-location-2026-10-24/README.md).

### Phase 121: Baumfall, Mob-KI, Trefferboxen Und Bienen

Abgeschlossener Sneak-Ganzbaumabbau loest eine gebuendelte Fall-/Bruchanimation
mit nachfallender Krone aus. Die eigentlichen Blockaenderungen und Drops bleiben
autoritative, begrenzte Arbeiten; kein kosmetischer Baumblock bekommt einen
eigenen Collider oder eine eigene Entity. Spielerersatzbloecke und Streaming-
Wechsel besitzen explizite Schutz-/Aufraeumpfade.

Mob-Treffer verwenden anatomische gedrehte Teilvolumen statt quadratischer
Ganzkoerperhuellen. Korrigiert sind individuelle Jungtier-Navigation, Spinnen-
Orientierungswechsel, physischer Pferde-Hufkontakt und Stalker-Treppen-/Griff-
Zustaende. Nichtendliche Ziele und unbeschraenkte lokale Suchsweeps werden
abgewiesen; F1+H trennt Trefferteile und konservativen Navigationsraum.

Natuerliche Wasserfaelle verwenden echte Block-Fluessigkeitszellen statt
unabhaengiger Fallplatten, auch bei kleinen abrupten Fallkanten. Geraeusche folgen
wirklich fallenden geladenen Zellen und sind leiser. Neue Welten verwenden
Cave-Strukturrevision 8 mit breiteren Gaengen; gespeicherte Revisionen bleiben
erhalten.

F12 ergaenzt Honigentnahme am echten Nest, Bestaeubung durch Nektarbesuche und
ressourcenfinanzierten Bienennachwuchs ueber das vorhandene F06-System. Reifung,
Nestkapazitaet, Familien, Inventare und bestehende Speicherlayouts bleiben
massgeblich. Client und Server verwenden gemeinsam Protokoll 71.

Pruefungen, Bedienung, Aufnahmen und technische Grenzen:
[Phase-121-Bericht](tools/benchmarks/results/tree-ai-bees-water-2026-10-24/README.md).

### Phase 122: Doerfer, Spinnennester Und Pet-Menue

Neue Welten erhalten deterministische Doerfer in drei Stilen (Wueste,
Plains/Forest und Highlands), Ruinen und Minen. Die vorhandene Section-
Generierung verwendet begrenzte regionale Plaene mit echten Objektzustaenden,
Truheninhalten und erntbaren Feldern. Fehlendes `structures_revision` in alten
Weltmanifesten bedeutet weiterhin 0: keine nachtraeglichen Gebaeude in alten
Spielerwelten. Dorfbewohner und Berufe bleiben der gesonderte Punkt F10.

Neue Baubloecke sind `teramic::path`, `teramic::wool_block`, `teramic::wool_stairs`
und `teramic::wool_slab`, einschliesslich Rezepten, Kollision und gemischten
Slab-Haelften. Vorhandene Block-IDs und Mischslab-Codes bleiben erhalten.
Client und Server benoetigen gemeinsam Protokoll 72.

Ganzbaumabbau zaehlt alle zugeordneten natuerlichen Holzsegmente mit 0,1 s je
Block. Spinnen bauen zuerst eine grosse Netz-Konstruktion, warten 30 bis 90
aktive Simulationssekunden und wechseln dann zwischen Jagd/Patrouille und
Rueckkehr. F9 zeigt vertikale Chunk-Spalten und horizontale Section-Grenzen.
Das Pet-Radialmenue besitzt den dunkel-goldenen Referenzaufbau mit realem
Tierportraet und einer kompakten Fensteransicht.

Implementierung, reproduzierbare Standortsuche und Pruefergebnisse:
[Phase-122-Bericht](tools/benchmarks/results/villages-pets-spiders-2026-10-24/README.md).

### Phase 128: Cherry Forest, Mob-Bewegung Und Lichtauswahl

F34 erweitert die vorhandenen Klima-, Baum-, Prop-, Wasser- und Kartensysteme.
Ein kontinuierliches seed-basiertes Klimafeld erzeugt Sakura-Regionen statt
zufaelliger Einzelchunk-Biome. Verzweigte breite Kronen verwenden die neuen
Blatt-IDs 132 (pink) und 133 (weiss), gemeinsame alpha-geclippte Blattgeometrie
und neutrale Bluetentexturen. Wind, Schatten und saisonaler Blattverlust bleiben
erhalten. Pinke/weisse Blumen, pflueckbare pinke Blumen und deren Crusher-Rezept
sowie dezente rosa Wasser-/Kartenfarben gehoeren dazu.

Schmale Baeche in Cherry Forest, Forest und Highlands erweitern die bestehende
gecachte Flusspfad-Generierung. Wechselnde Breiten, flache Becken, Gefaellestufen
und passende Uferschnitte verwenden dieselben Wasserzellen und denselben
Fluss-Renderer. Kein zweites Wasser-/Prop-System. `/locate biome cherry_forest`
verwendet die gemeinsame sichtbare Biomkennung. Diese meldet Ocean nur ueber
einem echten offenen Meereswasserstand, nicht auf trockenem Grasland.

Ecology 1 aktiviert die kleinen Baeche auch bei `rivers = false` (keine grossen
Flusspfade); `water = false` deaktiviert beide. Gespeicherte Ecology-0-Welten
erhalten keine neuen Baeche.

Neue Welten speichern `ecology_revision = 1`. Ein fehlendes Feld in alten
Manifesten bedeutet 0; auch Dedicated Server uebernehmen die Revision aus dem
Weltvertrag. Alte Welten werden nicht nachtraeglich mit Sakura und Baechen
umgeneriert. Generatorversion, vorhandene Block-IDs und Spieler-Edits bleiben
erhalten. Fuer die neue Landschaft eine neue Welt erstellen; Client und Server
gemeinsam aktualisieren.

Die gemeinsame Wegpruefung validiert auch die Koerperausrichtung am Anfang
einer Kante. Bewohner geben dauerhaft unerreichbare Ziele auf, koennen
kollisionsgeprueft seitlich aus Moebeln/Tueren herausgehen und ueber echte
Schwerkraft von niedrigen Kanten heruntersteigen. Keine Befreiungs-Teleports.
Ein Test fuehrt alle 17 Bewohner eines tatsaechlich generierten Dorfes durch
40 simulierte Sekunden mit echten Hausformen und Tueroeffnungen.

Mob-Wiedergabe verwendet hoechstens acht Serverzeit-Snapshots, begrenzte
Extrapolation und weiche spaete Korrekturen. Mehrere ankommende Voll-Snapshots
eines Frames werden zusammengefasst. Fehlende Pakete erzeugen keinen endlosen
Gehzyklus; die dargestellte Ortsveraenderung steuert die Laufanimation.

Fackeln geben ab vier Metern 50 Prozent mehr Umgebungslicht ab, ohne den
Nahkern heller zu machen oder die Anzahl dynamischer Lichter zu erhoehen.
Die begrenzte Lichtauswahl bevorzugt sichtbare Weglichter vor verdeckten
Innenraumfackeln; nahe Ecklichter bleiben zugelassen. Strukturrevision 5 setzt
bis zu 20 unterstuetzte Fackelpfosten neben Dorfwegen, ausserhalb von Tueren,
Haeusern und begehbaren Wegspuren. Aeltere gespeicherte Strukturrevisionen
bleiben unveraendert. Dachtreppen und Slabs
schliessen auch bei nur zwei gegenueberliegenden Dachseiten am Rohr an, bei
weiterhin offenem Rauchquerschnitt und erhaltener Materialmischung.

Native Aufnahmen, Tests, Grenzen und umsetzbare weitere FPS-Vorschlaege:
[Phase-128-Bericht](tools/benchmarks/results/cherry-ai-light-2026-10-25/README.md).

### Phase 134: Hoehlenausblick, FPS-Messung Und Dependencies

Min-/Max-FPS beginnen erst mit vollstaendig nach den ersten zehn realen
Sekunden des Weltbeitritts liegenden Frames. Auch ein langer Frame ueber die
Warmup-Grenze wird ausgeschlossen; Weltwechsel setzen die Werte zurueck.
Entladene Sections bereinigen die Mesh-Warteschlange einmal je Batch statt
einmal je Section. Binding-/Shadow-Marker nutzen SparseSet-Speicherung;
Schatten-Roots bleiben erhalten, damit ihre Query-Reihenfolge nicht durch
wiederholte Tabellenwechsel schwankt.

Hoehlen dunkeln die globale Sonne und den durch Eingaenge sichtbaren Himmel
nicht mehr ab. Nebel und lokale Hoehlenaufhellung lassen den sonnenbeleuchteten
Aussenraum erkennbar, ohne geschlossene Hoehlen pauschal taghell zu machen.
Bevy bleibt unveraenderte Registry-Abhaengigkeit; TiltUI-Fixes liegen im
separaten Quellrepository. 2534 Workspace- und 322 TiltUI-Runtime-Tests,
Formatierung, Check, striktes Clippy und Workspace-Build bestanden. Die native
Portalpruefung zeigt sichtbares Tageslicht von innen bei weiterhin dunklen
geschlossenen Hoehlen. Regenfluege bei Sichtweite 12/20 zeigen keinen
belastbaren allgemeinen FPS-Gewinn; keine Langzeitabnahme, A05 bleibt offen.
[Umsetzung und Nachweise](tools/benchmarks/results/cave-fps-no-vendor-2026-10-26/README.md).

### Phase 139: Verbundene Seen, Lange Fluesse Und Relief

Neue Welten verwenden `landscape_revision = 3` und aktivierte Hauptfluesse.
Gespeicherte Landschaftsrevisionen 0/1/2 und deren Flussoption bleiben erhalten;
keine stille Regeneration vorhandener Welten. Eine explizite Fluss-Deaktivierung
in einem neuen Revision-3-Template wird respektiert. Seen und die kleinen
Oekologie-Baeche sind weiterhin eigenstaendig von dieser Hauptflussoption.

Die bestehende Hydrologie erzeugt groessere, unregelmaessige Seen und lange
regionale Flusskurse mit gemeinsamen Pegeln, Seeanschluessen und geprueften
Meeresmuendungen. Kleine Baeche bleiben erhalten, sofern ihr gesamter Kurs
nicht mit einem regionalen Gewaesser kollidiert. Wasserzellen, Stroemung,
Wasserfall-Auslaesse, Section-Meshing und Fluessigkeits-Scheduler werden
weiterverwendet; keine zweite Wasser-Engine.

Ebenen und Plateaus bewahren mehr lokales Relief. Senken werden weich statt
auf eine konstante Hoehe begrenzt. Hoehleneingaenge pruefen tragende Decken;
explizite Durchgaenge bleiben auch beim Kreuzen von Kammerformationen offen.

Streaming-Interest, Chunk-Animationsabfragen und Wasser-Uploads vermeiden
wiederholte Mengenbildung, Query-Erzeugung und unnoetige Vollscans. Ruhige
Meeresflaechen teilen ihre Gitterecken auch bei aktivierten Fluessen. Der
Channel-Mesher berechnet keine Oberflaechenecken fuer die falsche Hoehen-Section.
Budgetierte wiederverwendbare Cloudgruppen ersetzen Vollhimmel-Reuploads;
shader-sichere Bounds vermeiden Offscreen-Arbeit. Grosse stabile Prop-Batches
werden im Worker aufgeteilt. Dynamische Wasserdaten laufen ueber einen
gemeinsamen GPU-Puffer statt Material-Aenderungen fuer alle Wassertiles.

Native kurze Regen-/Wasser-/Schnee-Laeufe zeigen bessere AVG/p99-Werte gegen
zeitnahe Kontrollen, aber keine durchgaengige 32-FPS-Untergrenze: Ein
Regen-Wiederholungslauf enthaelt weiterhin einen einzelnen 73-ms-Ausreisser;
die neue Schnee-Szene erreicht mindestens 27,25 FPS. Alle Messungen,
Wiederholungen und Einschraenkungen sind dokumentiert, keine Langzeitabnahme.

[Implementierung, Messungen und Validierung](tools/benchmarks/results/hydrology-relief-performance-2026-10-26/README.md).

### Phase 166: Start-Icons Und Wassertechnik

Normale grafische Starts laden den endlichen Itemkatalog, TiltUI-Symbole,
UI-Bilder und Schriften vor dem Weltbeitritt. Der Fortschritt steht im
vorhandenen Menue; ein frueher Start-/Joinwunsch wartet auf die fertigen Caches.
Danach werden diese Bild-Handles wiederverwendet. Frei gemischte RGB-Varianten
bleiben auf den vorhandenen begrenzten Bedarfscache beschraenkt. Ein fehlendes
Asset wird als Startfehler gemeldet, nicht durch endloses Laden versteckt.

F27/F28 verwenden die bestehenden Wasserzellen, den Fluessigkeits-Scheduler und
die Kinetik: Holz-/Stein-/Eisenrohre, absperrbare Ventile, Wasserhahn, kinetische
Pumpe und modularer Speicher. Rohre verbinden sich raeumlich wie Kaminsegmente;
die Darstellung entfernt unbenoetigte Arme und Beckeninnenwaende. Rechteckige
Becken reichen von 1x1 bis 5x5 Grundflaeche und bis zehn Bloecke Hoehe, auch 1x2.
Ein Modul speichert acht echte Fluessigkeitseinheiten. Hoehendifferenz liefert
Schwerkraftdruck; zum Hochpumpen ist ein laufender mechanischer Antrieb noetig.
Ventile sperren den Nachschub, fehlende Chunks pausieren die betroffene Leitung.
Ein offener versorgter Hahn gibt begrenzt echtes Wasser an Becken oder den
vorhandenen Zellsolver ab. Es gibt keine zweite Terrain-Wassersimulation.

Rechtsklick ohne Gefaess schaltet Ventil/Hahn. Rohre oder andere Anschluesse in
der Hand haben Platzierungsvorrang; mit Schleichen bleibt Nachbarplatzierung
moeglich. Einen leeren Eimer, eine Glasflasche oder Giessekanne auf eine
sichtbare Quelle oder einen versorgten Hahn richten und Benutzen gedrueckt
halten. Der Wasserstand steigt sichtbar; Loslassen, Slotwechsel, Wegsehen oder
eine offene UI unterbrechen. Erst der Server uebertraegt das echte Wasser und
prueft dabei Inventar, Sichtlinie und Versorgung erneut. Am Hahn stammen die
Einheiten aus dem endlichen Speicher; eine grosse Kanne kann mehrere gepruefte
Teilfuellungen benoetigen. Ein voller Eimer bleibt ueber die vorhandene Aktion
platzierbar. Die gehaltene Giessekanne kippt mit sichtbarem Wasserstrahl beim
Giesen bepflanzter Beete. Die bisherige Kannenreserve bleibt sitzungslokal,
nicht als zusaetzliche Metadaten je einzelner Kanne gespeichert.

Neue Block-IDs 163..169 werden angehaengt, bestehende IDs bleiben gleich.
RG12 speichert Leitungsinhalte und Speicherformen um die unveraenderte
RG11-Huelle; alte Formate bleiben lesbar. Client und Server benoetigen gemeinsam
Protokoll 93. Rezepte und Inventaricons laufen durch die vorhandenen Registries.
Detailbedienung und Asset-Vertraege stehen im
[Blockhandbuch](assets/terra/blocks/README.md#water-pipes-faucets-and-reservoirs).

`TODO.md` enthaelt nur offene Aufgaben. Abgeschlossene Beschreibungen und
historische Messungen wurden nach `PATCH.md` verschoben, ohne die Nachweise
als neue Tests auszugeben. Die neuen Inventar-, Rezept-, Chemie-, Taschen-,
KI-, Befehls- und Debugwuensche stehen dort als F44 bis F52 offen.

Zwei getrennte kurze native Pruefmodi stehen bereit, ohne Langzeitlauf:
`TERRA_PREVIEW_WARMUP=1 TERRA_STARTUP_ONLY=1 target/debug/terra-client --smoke-test-menu`
prueft den vollstaendigen Cache vor Welt-/Serverstart;
`target/debug/terra-client --smoke-test-hydraulics` prueft die integrierte
Versorgung, echte Gefaessfuellung, Ventilsperre und Bewaesserung. Fuer den
Menue-Test setzt `TERRA_MENU_SCREENSHOTS`, fuer Wassertechnik `TERRA_CAPTURE_DIR`
das Screenshotverzeichnis. Teststaende und Rohlogs
liegen unter `tools/benchmarks/results/water-tech-startup-2026-10-31/`.

Die finale Pruefung besteht mit 3511 Workspace-Tests, null Fehlern und acht
explizit ignorierten Spezialtests sowie Format, Check, strengem Clippy und
Workspace-Build. Der native Wassertechnik-Test bestaetigt neun nichtleere
Aufnahmen und echte Rechtsklick-Transaktionen in 19.51 Sekunden. Vor dem
Weltstart sind 454 Inventar-Icons und 1611 UI-Symbole fertig; der separate
Headless-Test generiert 45 Sections. Keine neuen FPS-Messungen oder
Langzeitabnahme. Details, Aufnahmen und Einschraenkungen:
[Phase-166-Pruefbericht](tools/benchmarks/results/water-tech-startup-2026-10-31/README.md).


### Phase 168: Aktuelle Wasserbedienung Und Asset-Ladescreen

Vor dem Hauptmenue wartet ein separater Ladescreen auf Modelle, Texturen,
Inventar-/UI-Icons, Schriften und die vorhandenen Audio-Caches. Fehler werden
angezeigt; das Hauptmenue ist bis zum abgeschlossenen Preload nicht bedienbar.

An Wasserquellen fuellen sich Eimer und Kanne per kurzem Rechtsklick. Laufende
Wasser-Meshupdates verursachen dabei keinen falschen Revisionskonflikt. Boden
unter Wasser bleibt mit dem normalen Abbaubefehl erreichbar. Uferwellen bleiben
an Kontaktkanten verankert, und Beckenraender haben keine doppelten Flaechen.

Haehne zeigen zu/1/2/3: LMB eine Stufe zu, RMB eine Stufe auf, Shift+LMB abbauen.
Gefaesse behalten am Hahn ihre Halte-Fuellaktion. Der Ausfluss kann ein separates
Becken durch eine freie Luftstrecke erreichen. Regen fuellt offene Becken langsam;
Daecher sperren die Sammlung. Client und Server verwenden gemeinsam Protokoll 94.

## Beckenwasser, Quellenbildung Und Befuellstation (Phase 172)

Endliches Beckenwasser verwendet denselben Shader und Materialhandle wie
Fluesse und Seen. Gespeicherte Mengen bestimmen Oberflaeche, Schwimmen,
Unterwasseransicht und Atempruefung. Alle vier Holzarten bleiben beim Bauen,
Abbauen und Speichern erhalten; das normale TMI-Becken verwendet Fichte.

Beim Ansaugen bleibt zusammenhaengendes Wasser ab 27 nassen Zellen unveraendert.
Auch eine offene Grenze zu ungeladenem Gebiet wird nicht als abgeschlossenes
kleines Becken behandelt. Nur ein nachweislich isolierter Wasserkoerper mit
weniger als 27 nassen Zellen wird geleert. Gespeicherte Becken- und Rohrinhalte
bleiben endlich. Wasser um eingetauchte Rohre behaelt den umgebenden Pegel;
Blasen erscheinen am tatsaechlich arbeitenden Ansaugmund, bleiben unter Wasser
und ausserhalb des Flanschs. Eine zweite flache Ripple-Flaeche ist entfernt.

F29: Zwei volle diagonale Quellen erzeugen in einem vollstaendig gestuetzten,
geschlossenen 2x2-Becken die beiden uebrigen Quellen. Geschlossene flache
Terrainbecken bis 64 Innenzellen nivellieren auch endliches Wasser konservativ
und kommen mit null Stroemung ohne dauernde Wasser-Mesh-Aenderungen zur Ruhe.
Ein gemeinsames Pruefbudget begrenzt die Arbeit; unvollstaendig bekannte Becken
warten. Nach dem endlichen Abpumpen bilden entquellte Becken keine neuen Quellen.

Der Hold Block (170) steht auf Bandhoehe zwischen zwei Foerderbaendern und
uebernimmt ihre passende Achse und Laufrichtung. Darueber bleibt ein Luftblock
frei; erst zwei Zellen ueber dem Hold sitzt der ganze blockgrosse Fluid Infuser
(171). Seitliche oder obere Rohre versorgen ihn ohne eigenen Kinetikantrieb.
Ungeeignete und volle Items fahren mit dem normalen engen Bandabstand gemeinsam
weiter. Erst ein fuellbares Gefaess fordert einen Einzelplatz an: Vorgaenger
raeumen das Deck, Sperren heben sich, das Gefaess haelt zum Fuellen und faehrt
danach physisch weiter. Ein blockierter Luftspalt unterbricht die Befuellung
ohne Wasserverbrauch. Rueckstau behaelt den Gegenstand; es gibt keinen Teleport.
Der Infuser folgt der [Blockvorlage](docs/references/fluid-infuser-block.png)
mit Holzgehaeuse, Eisenrahmen, Messingbeschlaegen, Rohrflansch und Metallduese.

Leere Flaschen, Eimer und teilgefuellte Giesskannen werden automatisch befuellt.
Der Inhalt einer Kanne gehoert zum einzelnen gespeicherten Item und bleibt beim
Transport und Aufheben erhalten. Das Rohrsystem fuehrt Wasser; weitere
Fluessigkeitstypen koennen den expliziten Konvertierungsvertrag erweitern.

Der rahmenlose Start-Ladescreen zeigt `Laden` ueber dem Balken, den Zaehler
mittig darin und den aktuell geladenen, bei Bedarf gekuerzten Assetpfad darunter.
Nach dem Preload blendet er in 650 ms zum Hauptmenue ueber; die Menueeingabe
bleibt waehrend des Uebergangs gesperrt. Pumpen und Haehne verwenden den
vorhandenen raeumlichen Geraetemix. Neue Clients und Server verwenden gemeinsam
Protokoll 97. Details stehen im
[Blockhandbuch](assets/terra/blocks/README.md#water-pipes-faucets-and-reservoirs)
und in [PATCH.md](PATCH.md#phase-172-dichter-banddurchlauf-infuser-abstand-und-ruhende-quellen).


## Rezeptbuch, Glasrohre Und Unterwasserbau (Phase 173)

Die Rezeptansicht verwendet einen JEI-aehnlichen Aufbau im vorhandenen gruenen
Teramic-Stil: echte Stations-Tabs, eine zentrale Zutaten-/Ergebniskarte,
Prozesspfeil, lesbare Mengen und direkt anwaehlbare Alternativen mit Seitenwahl.
Der Materialbedarf bleibt sichtbar; kleine Fenster verkleinern die Slots und
scrollen den Inhalt. R oeffnet auch aus einem Rezept heraus das Rezept einer
Zutat oder des Ergebnisses. Suchfelder, absolute Item-Tooltips und bestehende
autoritative Stations-/Plus-Aktionen bleiben erhalten. Das Blaettern und
Kategoriewechseln erzeugt keine Items und veraendert keine reservierten Zutaten.
RGB-Rezepte behalten ihre exakte Farbe.
Die Seitenwahl zaehlt Alternativen innerhalb der gewaehlten Station;
beispielsweise sind Ofen und Lagerfeuer getrennte Tabs.

Normale Tunnel haben ausschliesslich ihren Random-Transportschalter. Filter
gehoeren zu den ausdruecklich gefilterten Tunnel-/Funnel-Varianten. Veraltete
Filterdaten normaler Ports werden beim Laden entfernt. Der Bauray schaut unter
Wasser durch die Quellzelle am Auge bis zur tatsaechlich anvisierten Oberflaeche;
Gefaessfuellung und Unterwasserabbau bleiben eigene Interaktionen.

Glasrohr (172) ist transparent, herstellbar, im TMI und mit allen vorhandenen
Rohrarten verbunden. Bestaetigter Leitungsinhalt ist in Geraden, Boegen und
Verzweigungen sichtbar. Ruhende leere Rohre zeigen kein Wasser; bestaetigter
Durchfluss bleibt als dezente Innenbewegung sichtbar, auch wenn der Inhalt im
selben Tick weiterfliesst. Gespeicherte Mengen werden dadurch nicht erhoeht.
Verbundene Rohrmuendungen haben keine doppelten Stirnringe. Rohre behalten beim Einsetzen
und Materialwechsel die vorhandene aeussere Wassermenge einschliesslich einer
Quellmarkierung. Ihr Abbau zu Luft gibt dieses Wasser wieder frei. Ein bloss
platziertes, unbenutztes Rohr entzieht dem Gewaesser kein Wasser. Die natuerliche
Aufnahme braucht eine laufende Pumpensaugroute oder einen erreichbaren Abnehmer
mit ausreichender Foerderhoehe. Endlicher Durchfluss bleibt endlich, und echte
Pumpentnahme leert kleine isolierte
Gewaesser weiterhin ohne Regeneration. Die sichtbare Wasseroberflaeche wird an
den runden Rohrprofilen und den tatsaechlichen Facetten der Verzweigungen
beschnitten. Eckige Kollisionsboxen erzeugen keine quadratischen Wasserausschnitte.
Client und Server verwenden gemeinsam Protokoll 98.

## Absperrhahn Und Sonnenlicht (Phase 174)

Absperrhaehne lassen sich an Boden, Decke und Seitenflaechen entlang aller drei
Rohrachsen setzen. Vorschau, Serverplatzierung, Anschluesse und Modell verwenden
dieselbe Ausrichtung. Der Absperrhahn bleibt ein Auf-/Zu-Ventil. Nach dem
Schliessen kann der endliche Leitungsinhalt auf der Abnehmerseite weiterhin
zum offenen Abnehmer ablaufen; gesperrte Wege und fehlende Foerderhoehe bleiben
wirksam. Geschlossene Leitungen verlieren dadurch kein Wasser ins Nichts.

Glasrohre zeigen kleinere gespeicherte Mengen durch einen sinkenden,
schwerkraftgerechten Fuellstand. Rohr-, Pumpen- und Auslaufwasser verwenden
dasselbe biomeabhaengige Wassermaterial wie Seen und Becken. Eingeschlossenes
Wasser bleibt innerhalb der Rohrgeometrie und bekommt keinen Uferschaum.
Bestaetigter Durchfluss ohne gespeicherte Menge erscheint als duenne Stroemung.

Die direkte Sonnenenergie ist von 100000 auf 65000 Lux abgestimmt, damit helle
Grasflaechen ihre Zeichnung behalten. Kamerabelichtung, Tonemapping, Grastexturen,
Nachtlicht und die vorhandene Hoehlen-/Fackelabstimmung bleiben erhalten.

## Gleichmaessiger Durchfluss Durch Offene Ventile (Phase 175)

Ein offener Absperrhahn hat in horizontalen und vertikalen Leitungen denselben
Durchsatz und dieselbe Fuellfolge wie das entsprechende gerade Rohr. Verfuegbare
Quellversorgung wird nicht mehr von einem hoeher gelegenen Restwasserspeicher
verdraengt. Gespeichertes Wasser in einer aktiven Saugleitung folgt dem Weg
zur Pumpe, statt zwischen oberen und unteren Rohren hin und her zu wechseln.

Die Durchflussanzeige beruecksichtigt auch echte senkrechte Transfers und
Wasser, das im selben Schritt bereits weitergegeben oder verbraucht wurde.
Sie veraendert keine Mengen. Reiner Transit wird als schmaler, durchgehender
Wasserstrom entlang des Rohrverlaufs gezeichnet; gespeicherte Mengen behalten
ihren tatsaechlichen Fuellstand. Nach dem Stoppen verschwindet nur die
Transitdarstellung, vorhandenes Wasser und der endliche Auslauf bleiben erhalten.

## Wandfackeln Und Rohr-Oberflaechen (Phase 176)

Normale Fackeln koennen wieder an allen vier Wandseiten platziert werden.
Vorschau und Server benutzen denselben Montagekontakt und dieselben
Supportregeln. Eine ungueltige Stuetze entfernt die Fackel mit genau einem Drop.
Bodenmontage bleibt erhalten.

Rohrtexturen folgen dem geraden beziehungsweise gebogenen Rohrverlauf. Passende
Tangenten erhalten die Normalmaps auf Aussen- und Innenwaenden; kleine
Messingringe liegen vollstaendig auf den geraden Anschlussarmen.

Wasser wird an Ventilen und Pumpen entlang ihrer sichtbaren Koerper statt
entlang grober Kollisionsrechtecke ausgeschnitten. Die Wasserprofile passen
auch zu den versetzten Rohr-Stuetzringen. Gemeinsame Wasserecken an
Chunkgrenzen erhalten identische Wellengewichte und Stroemungsdaten.
Rohrkontakte bleiben verankert, weiter entfernte Wellen bleiben aktiv.

Der Hydraulik-Smoke prueft vier echte Wandplatzierungen, die Boegen aller vier
Rohrmaterialien sowie eingetauchte Rohre, Ventil und Pumpe. Der optionale
`TERRA_WATER_CONTACT_SMOKE=1` erweitert `--smoke-test-daylight` um zwei
Nahansichten im natuerlichen Oasengewaesser. Build, Clippy und 3686 Tests sowie
beide nativen Laeufe mit insgesamt 68 Pflichtaufnahmen bestanden.
[Pruefbericht und Aufnahmen](tools/benchmarks/results/torch-pipe-surfaces-2026-11-02/README.md).

## Rohrdurchfuehrungen Und Wasser In Teilbloecken (Phase 177)

Holz-, Stein-, Eisen- und Glasrohre nutzen bei einer tatsaechlichen Wand- oder
Dachumfassung denselben Capture-/Compose-Weg wie Ofenrohre. Das umgebende
Blockmaterial schliesst entlang des runden Rohrprofils; freie Rohrnetze erhalten
keine erfundenen Waende. Boegen, Abzweigungen und Sectiongrenzen bleiben erhalten.

Wasser belegt den freien Aussenraum der vorhandenen Teilblockformen, darunter
Zaunpfosten und verbundene Querstreben. Quellverbundene Treppen-/Zaunluecken
uebernehmen den angrenzenden Wasserstand. Gedeckte nasse Taschen senken den
freien Aussenspiegel nicht mehr ab. Slabs halten Wasser bis zur festen Kante;
oberhalb dieser Kante bleibt echtes Ueberlaufen moeglich. Eigene Behaelter bleiben
vom aeusseren Wasser getrennt.

Der Fluid Infuser zeichnet einen durchgehenden runden Strahl vom Auslass zum
gehaltenen Gefaess. Er verwendet das vorhandene Wasser mit eigener Strahldicke
und bewegten Glanzlichtern. Der bestaetigte Fuellvorgang beruecksichtigt Wasser
im gesamten angeschlossenen Netz; ein leerer lokaler Puffer blendet den Strahl
nicht mehr faelschlich aus.

Die optionale Umgebung `TERRA_PARTIAL_WATER_SMOKE=1` erweitert
`--smoke-test-waterfalls` um eine eigene kurze Szene fuer Slabbecken,
Treppen-/Zaunluecken, untergetauchte Teilmodelle und Rohrdurchfuehrungen.
Build, Clippy, 3712 Tests sowie drei native Laeufe mit 42 Pflichtaufnahmen
bestanden. [Pruefbericht und Bilder](tools/benchmarks/results/partial-water-pipe-surrounds-2026-11-02/README.md).

## Geschlossene Wasserraender (Phase 178)

Slabs, Treppen und Zaeune begrenzen Becken seitlich; Wasser fuellt den Innenraum
und passende Randluecken. Geschlossene Slabraender begrenzen den Pegel auf ihre
feste Oberkante. Beim Rohrsetzen bleibt vorhandenes Naturwasser erhalten und
wird an den Client uebertragen. Wassergefuellete Rohre werden nicht durch eine
abgeleitete Terrainumfassung zu scheinbaren Vollbloecken.

Build, Clippy und 3716 Tests bestehen. Zwei native Spielpruefungen mit
41 Pflichtaufnahmen bestaetigen die geschlossenen Raender und den Wassererhalt.
[Pruefbericht und Bilder](tools/benchmarks/results/closed-banks-waterlogged-pipes-2026-11-02/README.md).

## Innere Zaunseite Und Ueberflutete Fackeln (Phase 179)

Wasser endet innerhalb einer Zaunzelle an der nassen Seite und fuellt die
inneren Ecken. Bereits untergetauchte Teilbloecke behalten ihr Wasser; Rohre
auf der Wasseroberflaeche verdecken diese nicht mit einer Terrainumfassung.
Fackeln lassen sich nicht in Wasser setzen und werden bei spaeterem
Wasserkontakt mit genau einem Drop entfernt. Der Wasserzustand bleibt erhalten.
[Pruefbericht und Bilder](tools/benchmarks/results/fence-inner-water-partial-blocks-torches-2026-11-02/README.md).

3721 Tests, Build, Clippy und die beiden nativen Laeufe mit 43 Pflichtaufnahmen
bestanden. Die Zaun-Draufsicht und die Ansicht direkt an der Beckenwand sind
im Pruefbericht verlinkt.

## Kupferinventar Und Rohrwasser (Phase 180)

Player Inventory und Crafting Recipe verwenden den gemeinsamen Kupferrahmen,
braune eingelassene Slots, goldene Auswahlen und die Anordnung der gelieferten
Vorlagen. Das Spielerinventar hat vier echte Rucksackreihen und neun Hotbarplaetze
(45 insgesamt); alle zehn vorhandenen Ausruestungsplaetze bleiben bedienbar.
Rezeptbuch, Itemsuche, Kategorien, Blaettern und alternative Rezepte arbeiten mit
den bestehenden Rezept- und Itemdaten. Zutaten und Ausgaben der Alternativen
fordern ebenfalls ihre echten Vorschau-Icons an. Reines Blaettern im Rezeptbuch
erzeugt auch in Creative keine Gegenstaende.

Der Terrainrenderer darf die im Voxelmesher ausgelassenen Flaechen von
`Fluid Water Block` neben einem Teilmodell nicht als Wasserwuerfel rekonstruieren.
Diese doppelten Flaechen verursachten den sichtbaren Luftkasten und verdeckten das
Glasrohr. Die Korrektur ist an einer isolierten Kopie der gespeicherten Testwelt
mit gleicher Unterwasserkamera vor und nach der Aenderung geprueft. Dekoratives
`Solid Water` behaelt beim Ersetzen durch ein Rohr ebenfalls seine Wassermenge;
seine internen Kontaktflaechen werden an vollstaendig nassen Modellen ausgeblendet.

Vorhandene Inventardateien bleiben lesbar. Netzwerkprotokoll **99** transportiert
die erweiterten Inventar-, Reservierungs- und Container-Drag-Daten; Client und
Server muessen aus demselben Stand gebaut werden.
[Pruefbericht, Referenzen und native Aufnahmen](tools/benchmarks/results/copper-inventory-underwater-pipes-2026-11-02/README.md).

## Kompaktes Inventar Und Rezept-Popups (Phase 181)

Das Buch am Inventar-Crafting klappt den Itembrowser ein und aus. Seine
Kategorie-Tabs stehen links. R ueber einem Katalogitem oeffnet dessen Rezept. Rezepte erscheinen als Popup ueber dem weiterhin
sichtbaren, abgedunkelten Inventar. E und ESC schliessen zuerst das Popup und
anschliessend das Inventar; X-Schaltflaechen entfallen. Die Ofenansicht zeigt
Material, Brennstoff, Ofenmodell und Ergebnis sowie registrierte Schmelzzeiten
und Brennstoffalternativen. Die Brennstoffauswahl ist eine Vorschau. Holzrezepte
benennen ihre beliebigen Holzplanken ausdruecklich.
[Aufnahmen und Pruefbericht](tools/benchmarks/results/inventory-modal-furnace-2026-11-02/README.md).

## Quadratische Slots Und Referenz-UIs (Phase 182)

Inventar-Slots sind quadratisch; Pagination ist rechtsbuendig und an den
Seitenenden begrenzt. Rezept-Popups zeigen keinen JEI-Itembrowser. Bei Rezepten
mit beliebigen Holzplanken sind Oak, Spruce, Birch und Cherry einzeln als
Vorschau waehlbar. Kesselrezepte zeigen echte Zutaten, Wasser, Ruehrwerkzeug,
Kesselmodell und Ergebnis. Ihr Fortschrittsbalken folgt der autoritativen
Kesselarbeit. Neue Hotbar und Startmenue verwenden den Kupfer-/Blatt-/Kristallstil
der Referenz; Grafikassets werden vor dem Menue vorgeladen.
[Pruefbericht und Grafik-Prompts](tools/benchmarks/results/interface-pagination-square-slots-2026-11-02/README.md).

## Inventar Und Menues (Phase 183)

18 Rucksackplaetze plus neun Hotbarplaetze, eine groessere Character-/Crafting-
Ansicht und gleich grosse quadratische Itembrowser-Slots. Korrigierter Hotbar-
Ausschnitt ohne dunkle Hintergrundplatte. Pause im Hauptmenue-Stil; Settings
mit linken Kategorien und die Singleplayer-Weltliste im Kupferdesign der Vorlagen.
Client und Server verwenden Protokoll 100. Alte zusaetzliche Inventarreihen
werden beim Beitritt ohne Itemverlust uebernommen.
[Vorlagen, native Aufnahmen und Pruefbericht](tools/benchmarks/results/interface-menus-27-slots-2026-11-02/README.md).

## Slotdetails Und Kesselreferenz (Phase 184)

Crafting-Zellen entsprechen den Inventarzellen, JEI-Zellen sind 25 Prozent
kleiner und die Tonne steht unter den groesseren Kategorien links neben JEI.
Holzrezepte zeigen vier eigene Alternativkarten. Kesselrezepte verwenden die
neue Referenzillustration mit echten Rezeptdaten. Ladebildschirme und Welt-
Popups folgen dem Kupferstil; Menuebuttons besitzen mehr Rahmenabstand.
[Native Aufnahmen, Grafik-Prompt und Pruefung](tools/benchmarks/results/copper-ui-details-cauldron-2026-11-03/README.md).

## Geraete Und Diagnose (Phase 185)

Geraete, Container, Werkbank und Logistikfilter verwenden den Kupferstil mit
quadratischen Slots und gestapelten Arbeitsbereichen bei schmalen Fenstern.
Crafting- und Ofenrezepte folgen den aktuellen Vorlagen; die Rezeptansicht
bleibt als Popup ohne Itembrowser. F1 zeigt rahmenlose Diagnose-Textspalten
mit echten Spiel-, Welt- und Leistungsdaten. Blockkarte, verbleibende
Bedienelemente und Multiplayer sind ebenfalls farblich angepasst.
[Native Aufnahmen, Ofengrafik-Prompt und Pruefung](tools/benchmarks/results/all-surfaces-recipes-debug-2026-11-03/README.md).

## Feste Inventar-Layouts (Phase 188)

Spieler, Container, Geraete, Werkbank und Itembrowser verwenden 28 x 28 grosse
Slots, Ergebnisse 32 x 32 (logische Pixel). Passende Rahmen und Arbeitsbereiche
ohne Inhalts-Scrollbar; angeschlossene Truhen stehen links. Die Tonne sitzt
links unten am Itembrowser, der HUD-Rahmen passt zu den Hotbarslots.
FPS 0 heisst Unlimited. Auch bei Borderless bleibt die Aufloesung auswaehlbar;
Dropdowns verwenden einen kupferfarbenen Pfeil.
[Native Aufnahmen und Pruefbericht](tools/benchmarks/results/fixed-inventory-layout-2026-11-03/README.md).

## Inventargroesse Nach Aufloesung (Phase 189)

Inventare und Hotbar skalieren gemeinsam mit ihren Rahmen und Beschriftungen
passend zur Aufloesung. In Full HD sind normale Slots bei 100 Prozent
UI-Massstab etwa 56 Pixel gross, in 1440p etwa 74 Pixel. Angeschlossene Truhen
und der Abstand zur Hotbar werden beim Einpassen beruecksichtigt.
[Native Aufnahmen und Pruefbericht](tools/benchmarks/results/responsive-inventory-2026-11-03/README.md).

Inventarbeschriftungen und Bedienicons sind in Phase 190 kompakter. Die
grossen Slots bleiben erhalten; die Hotbarreihe im Inventar zeigt keine
redundanten Tastenzahlen mehr.
[Aktuelle Aufnahmen](tools/benchmarks/results/inventory-typography-2026-11-03/README.md).

Phase 191 verkleinert die HUD-Hotbar um 30 Prozent, vergroessert
Inventarzellen um 20 Prozent und senkt die Player-/Crafting-Hoehe um
20 Prozent. Text-/Icondichte ist fuer grosse Aufloesungen gedrosselt.
[4K-Aufnahmen und Kollisionspruefung](tools/benchmarks/results/inventory-density-collision-2026-11-04/README.md).
## Claims Und Diplomatie (Phase 195)

Kingdoms besitzen persistente Spieler-/Bot-Claims mit verbundenen Gebieten,
Reichweite und getrennten Kontingenten fuer geschuetztes und offenes Land.
Die grosse Karte zeigt ueber **Diplomatie** die Gebiete, Grenzen, Legende,
Gruender und den ausfuehrenden Akteur; Auswahl und Claim-Aktionen werden
live aktualisiert. F55/F56 sind abgeschlossen.
[Bedienung, native Aufnahmen und Bildkorrekturen](tools/benchmarks/results/claims-diplomacy-castle-2026-11-06/README.md).

## Gleisbau Und Zugmontage (Phase 210)

Gleise lassen sich aus einem Startstueck bis zum anvisierten Ziel strecken.
Der Baubereich reicht 32 Meter; ein tragender Block unter dem Modul reicht aus.
Bewegungsbestaetigungen entsprechen den wirklich simulierten Schritten, damit
Baubursts und schnelle Bewegung keine Schritte der Clientvorhersage verlieren.

F63 ergaenzt Fahrgestell, Drehgestell, Rahmen, Kupplung, Fuehrerstand, Sitz,
Laderaum und Montagestelle. Rechtsklick auf die Montagestelle oeffnet Montage,
Demontage und Verbundverwaltung. Ladung und Sitzplaetze bleiben mit dem
gespeicherten Fahrzeug verbunden. Protokoll 116; ID13 liest weiterhin alte Welten.
[Bauanleitung, Grenzen und CPU-Pruefbericht](tools/benchmarks/results/train-assembly-movement-2026-11-08/README.md).

Phase 211 korrigiert den Ladefehler im Zug-Stylesheet und den wachsenden
Eingaberueckstand bei nur serverseitigen Simulationspausen. Gleise lassen sich
durch die eigene Figur platzieren; andere Spieler bleiben geschuetzt.
[Korrekturen und CPU-Abnahme](tools/benchmarks/results/rail-input-recovery-2026-11-08/README.md).

Phase 212 erweitert die Montagestelle um bedienbare Typauswahl und eine optionale
gruene Bauvorlage direkt am Gleis. Nach dem Schliessen bleibt sie als Bauhilfe
sichtbar. Auch schwebende Bloecke koennen an vorhandene Blockseiten gesetzt werden.
F64 erlaubt einen freien Aufbau aus registrierten Bloecken mit Formen, Farben,
Verbindungen und gespeicherten Inventar-/Maschinenzustaenden. Montierte Maschinen
pausieren ohne Weltanschluesse. Kollisions- und
Fahrzeuggeometrie wird in begrenzten Hintergrundjobs berechnet.
Protokoll 117; der neue Zugindex ID14 uebernimmt alte ID13-Spielstaende.
[Aktuelle Bauanleitung und Bedienung](docs/train-building.md).


Phase 213 macht die Zuege mit Brennstoffmotor, Fuehrerstand, Fahrhebel und
Geschwindigkeitsanzeige fahrbar. Ganze Fahrgestell-/Rahmenbloecke und groessere
Raeder erleichtern den freien Aufbau; die Vorlage erlaubt 3 bis 15 Meter
Drehgestellabstand. Ladung darf vor oder hinter dem Fahrer sitzen. Die Montage
stellt den angehaltenen Zug an seiner aktuellen Position wieder als editierbare
Bloecke her, einschliesslich Ladung, Ausrichtungen und uebriger Brenndauer.

F54 ergaenzt den vollstaendigen BR52-Baukasten mit 76 neuen Teilen und Rezepten,
Metall-/Gitterformen, oeffnenden Fuehrerhausfenstern/-tueren und synchronisierten
Raedern, Gestaengen und Hebeln. Echte Dampfwasserversorgung und Kesseldruck bleiben
unter F53 offen; die entsprechenden Aufbauteile sind gekennzeichnet.
Kompakte Bewegungspakete verschieben vorhandene Fahrzeugmeshes und Collider,
ohne sie bei jedem Fahrschritt neu zu bauen. Protokoll 118 und Zugindex ID15
uebernehmen die gespeicherte Fahrt und lesen ID13-/ID14-Zuege weiterhin.
[Bauanleitung und Steuerung](docs/train-building.md).


Phase 215 leitet die Zugachse aus den Drehgestellen ab. Die verborgene Drehung
von Fahrgestellwuerfeln verhindert die Montage nicht mehr; gespeicherte Formen
und Blockzustaende bleiben erhalten. Beide realen Achskontakte pruefen einen
passenden geladenen Gleisweg. Ausrichtungs-, Abstands- und echte Gleisfehler
haben getrennte Meldungen. Protokoll 119 uebertraegt die neuen Fehlergruende.
Die normale Lokvorlage ergaenzt einen optionalen Kessel-/Fuehrerhausaufbau und
richtet sich nach vorhandenen Teilen. Beim Standardabstand bleiben nach dem
Grundgeruest 26 gruene Aufbauplaetze sichtbar. Die Vorlage bleibt bis zur
bestaetigten Montage erhalten, auch wenn das Fenster vorher geschlossen wird.

Phase 216 korrigiert den Zugriff auf den Laderaum vom Fuehrerstand aus und
Klicks auf Slotbilder und Stapelzahlen. Inventarstapel lassen sich mit einem
Klick verschieben; bereits gehaltene Stapel sind sichtbar und koennen in leere
Motor-/Lagerplaetze oder ins Spielerinventar gelegt werden. Private Inhalte
bleiben bei Fahrt und Brennstoffverbrauch offen. Veraltete Inventarstaende
werden vor einer einmaligen sicheren Wiederholung des Klicks neu angefordert.


Phase 218 glaettet bestaetigte Zugbewegung fuer Wagen, Laufwerk und Sitze mit
identischem Darstellungszeitpunkt je Frame. Die Fahrerkamera vermeidet eine
zweite Physikinterpolation. Wiederverwendete Geometriehuellen und begrenzte
Kollisionsvorbereitung reduzieren Arbeit beim Bauen. Neue gezeichnete Kurven
benoetigen mindestens 16 Meter horizontalen Radius; alte Gleisdaten bleiben
lesbar. Client und Server verwenden Protokoll 120.
[Pruefbericht mit nativer Baumessung](tools/benchmarks/results/train-motion-build-curves-2026-11-08/README.md).


Phase 219 ergaenzt den materialbasierten automatischen Vorlagenbau, gespeicherte
Zugnamen und beschriftete Nummernschilder sowie Scheinwerfer mit echtem
Lichtkegel und Wagen-Deckenleuchten. Die Montageansicht besitzt geordnete
Abstaende und ein eigenes Namensfeld. Kessel erkennen drei Bloecke breite
Zugrahmen und verwenden denselben 3x3-Querschnitt fuer Darstellung und Kollision.
Lok, Brennraum und Schienenlauf erhalten eine begrenzte entfernungsgerechte
Klangmischung. Prellboecke stoppen die ganze Fahrzeughuelle. Importierte
Gleisbelegung bleibt unabhaengig vom grossen Geometriecache erhalten;
kalte Kurven werden vor dem Import im Hintergrund vorbereitet.
Protokoll 121; Zugindex ID16 uebernimmt ID13-/ID14-/ID15-Zuege.
[Aktuelle Zugbedienung](docs/train-building.md).
[Pruefbericht und native Montageaufnahmen](tools/benchmarks/results/train-workshop-stops-lights-2026-11-09/README.md).
