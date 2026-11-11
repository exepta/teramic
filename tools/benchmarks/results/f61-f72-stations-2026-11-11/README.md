# F61, Berufsstationen, Komponentenordner und F72

Pruefung vom 11.11.2026 mit Rust 1.97.1 und den vorhandenen
Bevy-0.20-/Avian-0.8-/Lightyear-0.31-Abhaengigkeiten. Versionslinien blieben
unveraendert. TiltUIs bestehendes Fluent-Feature ist aktiviert.

## Umfang

- F61: physisches Anlaufen, natuerliche/gepflanzte Baumzuordnung, echte Axt
  aus dem Depot, Arbeitszeit, individuelle gespeicherte Restbenutzungen,
  begrenzte Faellbatches, reale Drops und Anlieferung sowie artgleiche
  Nachpflanzung. Breite Staemme erhalten eine erreichbare aeussere Arbeitsflaeche.
- Neun Berufsstationen mit eigenen GLBs, JSONs und eindeutigen Rezepten,
  Material-IDs 297 bis 305. Reale private Depots, Kollision, Rotation, Abbau
  und Inhaltserhalt. Holzfaeller, Bergarbeiter und Landwirt nutzen passende
  Stationen; Sammler koennen alle Depots verwenden. Die uebrigen sechs Berufe
  werden nicht als fertig automatisiert ausgegeben.
- 45 Komponenten-/Page-Triplets in je einem eigenen Verzeichnis; rekursive
  TiltUI-Erkennung, aktive Tests/Skripte/Assetpfade angepasst. Charakterbutton
  unten rechts im Hauptmenue.
- 3230 deutsche/englische Katalogeintraege, Fluent-Fallback/Pluralparameter,
  dynamische Textprojektion, Profilpraeferenzen, Sprachwahl in Gameplay,
  lokalisierte Inhalts-/Rezeptsuche und Schutz von Namen, Nutzereingaben und IDs.
- F78/F79/F80 sind als offene Aufgaben ergaenzt: neutrale Nomaden, produktive
  Doerfer/Bot-Doerfer und einmalige Kingdom-Startbevoelkerung mit 3 Schwert-,
  2 Bogen-, 3 Lanzenkaempfern und 10 Arbeitslosen. Diese Zukunftsaufgaben wurden
  nicht stillschweigend als bereits implementiert markiert.

## Automatisierte Validierung

```bash
cargo fmt --all --check
cargo check --locked --offline --workspace -j 4
cargo clippy --locked --offline --workspace --all-targets -j 4 -- -D warnings
cargo test --locked --offline --workspace --lib --bins --no-fail-fast -j 4 -- --test-threads=4 --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
cargo test --locked --offline --workspace --doc -j 4
cargo build --locked --offline -j 4 -p terra-client -p terra-server --bins --example profession_station_gallery
```

4684 Library-/Bin-Tests bestanden, 0 fehlgeschlagen, 10 bestehende Tests
ignoriert. Ein Test ist wegen der bereits fehlenden Originaldatei
`FIVERR/VoxelGrassBlock glb file.glb` ausgefiltert. Die Doctest-Phase bestand;
aktuell existieren keine ausfuehrbaren Doctests. Lokale UDP-Tests liefen mit
erlaubtem Netzwerkzugriff. Formatierung, Workspace-Check, striktes Clippy sowie
Client-/Server-Build bestanden.

Die Tests pruefen unter anderem alle neun GLBs mit dem echten Bevy-GltfLoader,
die Eindeutigkeit der Stationsrezepte unter allen eingebauten Rezepten,
WornTool-Ernteklasse/Abbauzeit/PvP-Schaden und gemeinsame Vorschaucaches,
vollstaendige Baum-Ertragskonservation und Nachpflanzung, Teilauftraege,
ID21-Migration nach ID22, alle Fluent-Nachrichten und Parameter, Plurale,
Sprachwechsel, Profilspeicherung und Schutz von benutzergenerierten Daten.

Gefundene und korrigierte Fehler: drei mehrdeutige shapeless Rezepte, fehlende
physische Anlaufposition fuer breite Staemme, einzeln pendelnde Dropanlieferung,
zwei ungueltige Testvorlagen und veraltete Material-/Template-Erwartungen.
Die Naturbaum-Vorlage verwendet nun wie die echte Generation Air-only-Stempelung
und erhaelt den Boden; kompletter Ertrag und artgleiche Nachpflanzung bleiben
verpflichtend. Ein widerspruechlicher Pruefprofil-Cache wurde neu erzeugt;
der anschliessende unabhaengige Workspace-Check bestand.

Bedienung und Grenzen: [NPC-Arbeit](../../../../docs/npc-work.md),
[Sprachen](../../../../docs/localization.md).

## Native Abnahme

Client und Dedicated Server wurden gebaut. Der Server-Smoke-Test lief ohne
DISPLAY/WAYLAND_DISPLAY mit 27 geladenen Sections erfolgreich bis zum
geordneten Ende. Die vorhandene Spielwelt wurde nicht geaendert.

Der Sprachtest lief in separaten Testkonfigurationen unter
`target/f61-f72-native/language-02/`: deutsches Hauptmenue, echte TiltUI-Handler
fuer Sprache/Uebernehmen, gespeicherte UUID-Profilpraeferenz, englische
Gameplay-Einstellungen und englisches Hauptmenue. Alle vier Aufnahmen wurden
erzeugt; die deutschen/englischen Einstellungen wurden visuell beurteilt.
Die Bounds-Pruefung bestaetigt den Charakterbutton unten rechts im Hauptmenue.
[Deutsch](gameplay-de.png), [English](gameplay-en.png).

Die isolierte [GLB-Galerie](profession-stations.png) wurde auf der vorhandenen
AMD Radeon RX 6950 XT per Vulkan gerendert. Sie wartet auf neun geladene und
instanziierte WorldAssetRoots und speichert vor AppExit. Fuer diese isolierte
Bevy-App mussten die glTF-Komponententypen explizit fuer Reflexion registriert
werden; der anschliessende Lauf war erfolgreich. Die Aufnahme zeigt echte
GLB-Geometrie. Die menschliche stilistische Referenzabnahme bleibt A01.

```bash
cargo run --locked --offline -p terra-client --example profession_station_gallery -- /tmp/profession-stations.png
```

Die produktiven Holzfaeller-Aktionen wurden mit der realen begrenzten
Weltsimulation und gespeicherten Inventaren automatisiert geprueft; die native
Galerie behauptet keinen zusaetzlichen grafischen Arbeitsablauf-Nachweis.
