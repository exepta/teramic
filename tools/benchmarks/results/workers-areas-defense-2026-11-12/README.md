# Arbeitswege, Rezepte, Gebiete und Verteidigung

Phase 239, Abnahme vom 12.11.2026. [Bedienung und Grenzen](../../../../docs/workers-areas-and-pvp.md).

## Automatisierte Abnahme

**4.843 Tests bestanden: 4.839 Library-/Binary-Tests und vier Integrationstests.**
Zehn bestehende Tests bleiben ignoriert. Nur
`original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images`
ist wegen der fehlenden externen Datei `FIVERR/VoxelGrassBlock glb file.glb`
ausgefiltert. Die Dokumentationstest-Ziele bestehen; derzeit enthalten sie keine
ausfuehrbaren Beispiele. Formatierung, Workspace-Check, Clippy aller Targets mit
`-D warnings` und getrennte Client-/Server-Builds bestehen.

Die 1.350 Welt-Tests pruefen unter anderem:

- Reale Treppenumwege bei gleicher X/Z-Position auf verschiedenen Etagen,
  sichere Erholung an einer hohen Terrassenkante und zentimetergenaue Standpunkte.
- Sofortige erste Suche fuer ein neues Arbeitsziel trotz geerbter Suchpause;
  unveraenderte Wiederholpause fuer dieselbe zwischengespeicherte Route.
- Bestaetigte Wandblockade am urspruenglichen 150-Tick-Pruefpunkt und weitere
  90 durchgehend blockierte Ticks. Nach Entfernen der Wand folgen echter Abbau
  und Materiallieferung. Aufgezeichnete Baubloecke bleiben geschuetzt.
- Pflanzen auf anfangs baumlosem Boden fuer alle fuenf Baumarten, den echten
  Fichtenzyklus Pflanzen/Wachsen/Faellen/Nachpflanzen, genaue Vorrats-/Dropbilanz
  sowie Unterbrechung, Speichern/Laden und fehlende Rechte/Chunks.
- Vier vollstaendige Dorfwohnhaeuser in allen Ausrichtungen, echte Materialkosten,
  Geruestabbau/Ruecklieferung und die vorgegebenen 30/15/10/5 Minuten aktiver Zeit.
- Kampfpause ohne Verlust von Werkzeugen, Auftraegen, Materialien oder Baufortschritt.

Die Netzwerktests pruefen echte Verteidigung, Sichtlinien, Reaktions-/Schlagabstand,
Ruestung/Spawn-Schutz, Ende der Verfolgung und die Ausnahme fuer den wirklichen
eigenen Koenig. Besitzer ohne Koenigstitel und fremde Koenige sind nicht ausgenommen.
Der neue Zwei-Client-UDP-Test verwendet geparstes `server.toml` mit `pvp=false/true`
und jeweils widersprechender gespeicherter Weltregel. Er prueft beide Welcomes,
Regelupdates und ein wirkliches Angriffspaket. Ein nachfolgender geordneter
Held-Item-Marker bestaetigt dessen Empfang auch bei abgewiesenem Schaden.
Serverzustand, Opferstatistik und entfernte Spielerdarstellung stimmen ueberein;
die gespeicherte Weltregel wird nicht umgeschrieben.

- [Workspace-Tests](tests.txt), [gesamte Welt-Suite](world-navigation-final.txt)
- [Echter UDP-PvP-Test](pvp-udp.txt), [Doctest-Ziele](doc-tests.txt)
- [Formatierung](fmt.txt), [Check](check.txt), [Clippy](clippy.txt)
- [DE/EN-Katalog: 3.336 Meldungen, keine offenen Uebersetzungen](localization.txt)

## Assets und Hausvorlage

487 endliche Rezepte liegen unter `assets/crafting/`: 447 normale Rezepte und
40 getrennte Ofen-/Brecher-/Braurezepte. Rekursive Loader, eingebettete Definitionen
und Asset-Generatoren verwenden diese Pfade. IDs und vorhandene Rezeptinhalte
bleiben erhalten. [Ordner und Schemas](../../../../assets/crafting/README.md).

Neue Hausauftraege lesen tatsaechlich die gepruefte JSON-Datei
`assets/buildings/templates/village/residential.json`: 390 atomare Schritte,
392 Zellen und 17 Materialarten. Unter anderem werden 66 Strukturstein mit ID43
benoetigt, kein gewoehnlicher Stein mit ID3. Bestehende gespeicherte Auftraege
behalten ihre Plaene. Der visuelle Editor bleibt F82.
[Vorlage und Export](../../../../assets/buildings/README.md).

## Native Grafikpruefung

Der finale Client auf AMD Radeon RX 6950 XT / Vulkan / RADV besteht beide isolierten
Grafiklaeufe mit wirklichem lokalem UDP-Server. Profile, Welt und Einstellungen
liegen ausschliesslich unter `target/`. Eingaben erfolgen ueber normale
Gameplay-Tasten, Raycasts und echte HTML-Handler; dies ist keine OS-Mausautomation.

Der Bauablauf prueft eine reale Truhe mit 128 Strukturstein und 128 Brettern,
gespeicherte Lagerauswahl, Build-Claim, einen wirklichen JSON-Hausauftrag,
Handwerker ohne Arbeitsgebiet, erste Nicht-Air-Blockplatzierung und bestaetigte
Pause. Die Bauzeiten werden nicht im kurzen Grafiklauf ausgesessen;
vollstaendige Bauten und alle vier Mannschaftszeiten prueft die CPU-Suite.

Die Bau-Testkonfiguration verwendet bewusst `fly_speed=4.0`; die normale
Spielkonfiguration bleibt bei 20. Bewegung erfolgt weiterhin physisch mit
echten Eingaben und Praezisionsmodus. Vor dem Menuewechsel muessen vorhergesagte
und bestaetigte Position passen sowie eine niedrige bestaetigte Geschwindigkeit
mit frischem ACK anliegen. Auswahlreichweite und Kollisionsregeln bleiben normal.

Sechs Aufnahmen pruefen Deutsch bei 1600x900 und Englisch bei 960x640 sowie echte
Caption-/Button-Grenzen. Der gespeicherte Lagerrahmen, die Legende und das
Schliessen-Symbol wurden auf der Aufnahme auch visuell geprueft. Die automatischen
Bereichspruefungen vergleichen bestaetigte Daten und HUD-Geometrie; sie sind kein
separater GPU-Pixeltest.

- [Bau-Tab, Deutsch](building-local-de-wide.png)
- [Auswahl-HUD](building-storage-selection-de-wide.png)
- [Gespeicherter 3D-Bereich und Legende](building-saved-areas-de-wide.png)
- [Hausauftrag, Englisch kompakt](building-job-en-compact.png)
- [Echte Blockplatzierung](building-progress-en-compact.png), [Pause](building-paused-en-compact.png)
- [Bau-Protokoll](native-building.txt), [Pruefdaten](native-building.json)

Der direkte NPC-Lauf besteht mit neun weiteren Aufnahmen: wirklicher Rechtsklick,
privates Inventar, Gebiets-/Lagerauswahl mit Abbruch ohne Weltedit, Zuweisung/Pause
und tatsaechlicher Holzfaeller-Stationsbesuch. Identitaet und Geschlecht bleiben
beim sichtbaren Berufswechsel erhalten.
[Dialog](npc-direct-de-wide.png), [fehlende Station](npc-lumberjack-missing-station-compact.png),
[Stationsbesuch](npc-lumberjack-profession-compact.png), [Protokoll](native-npc.txt),
[Pruefdaten aller neun Aufnahmen](native-npc.json).

Finale Laeufe: `target/building-local/run.6tVEBh/` und
`target/npc-work-direct/run.fJsUkB/`. Der separat gebaute Server startet ohne
DISPLAY/WAYLAND_DISPLAY und generiert 27 Sections. Sein Dependency-Baum enthaelt
keine Rendering-, Windowing- oder UI-Crates.
[Headless-Protokoll](headless.txt), [Server-Features](server-features.txt),
[Server-Build](server-build.txt), [Client-Build](client-build.txt).

## Wiederholen

```sh
cargo fmt --all --check
cargo check --locked --offline --workspace -j 4
cargo clippy --locked --offline --workspace --all-targets -j 4 -- -D warnings
cargo test --locked --offline --workspace --lib --bins --test headless --test textiles --test generated_sources --no-fail-fast -j 4 -- --test-threads=4 --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
cargo test --locked --offline --workspace --doc -j 4
cargo build --locked --offline -p terra-server --bin terra-server -j 4
cargo build --locked --offline -p terra-client --bin terra-client -j 4
env -u DISPLAY -u WAYLAND_DISPLAY target/debug/terra-server --smoke-test --no-save
bash tools/benchmarks/run-building-local.sh
bash tools/benchmarks/run-npc-direct.sh
```

Netzwerktests benoetigen lokale UDP-/TCP-Ports, die Grafiklaeufe ein funktionierendes
Display/GPU. In dieser Umgebung wurden sie entsprechend ausserhalb der
Netzwerksperre ausgefuehrt. Es gibt keine neuen Dependencies oder Versionswechsel.
Protokoll 139 und Speicherindex ID24 bleiben gleich. Entfernte Auftraege werden
weiter nur in geladenen Weltbereichen simuliert.
[Gesamtpruefung](verification.json), [Quellpruefsummen](source-sha256.json).
Die Nachweise der Phasen 235 bis 238 bleiben historische Pruefstaende.
