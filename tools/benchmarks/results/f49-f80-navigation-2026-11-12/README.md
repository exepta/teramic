# F49 / F80 – Abnahme Vom 12.11.2026

F49 erweitert die lokale physische Navigation, F80 die atomare Kingdom-Gruendung
um genau 18 persistente Start-NPCs. Keine neuen Rust-Abhaengigkeiten.
Bedienung und Grenzen: [Navigation und Gruendung](../../../../docs/navigation-and-founding.md).

## Ergebnisse

| Pruefung | Ergebnis |
| --- | --- |
| `cargo fmt --all --check` | Erfolgreich |
| `cargo check --locked --offline --workspace -j 4` | Erfolgreich |
| Workspace-Library-/Binary-Tests | 4717 bestanden, 0 fehlgeschlagen, 10 bestehende ignoriert, 1 ausgefiltert |
| `cargo test --locked --offline --workspace --doc -j 4` | Erfolgreich, keine Doctest-Faelle vorhanden |
| `cargo clippy --locked --offline --workspace --all-targets -j 4 -- -D warnings` | Erfolgreich |
| Client-/Server-Binaerbuild | Erfolgreich |
| Dedicated Server ohne DISPLAY/Wayland | Erfolgreich, 27 Sections bereit |
| Nativer Client mit integriertem UDP-Server und echten OS-Eingaben | Erfolgreich |

Ausgenommen bleibt ausschliesslich
`original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images`:
Die bereits zuvor fehlende externe Datei `FIVERR/VoxelGrassBlock glb file.glb`
liegt nicht im Repository. Es wurde kein Ersatzasset erfunden und keine weitere
Pruefung deaktiviert.

## Inhalt Der Regressionstests

Navigation: veraenderte gecachte Wegmitte, schmale Durchgaenge, gleiche X/Z auf
verschiedenen Etagen, reale Treppenbewegung, Deckenfreiheit, Claim-Umwege,
Suchbudget und Retryfristen. Ein abgelaufener, weiterhin gueltiger Weg fordert
keine Suche pro Tick an. Die bestehenden Baeren- und Spinnenpruefungen bleiben
erhalten.

Arbeit: echte Holztueranforderung und bestaetigte Passage mit Ruecklieferung,
geschlossene/unvollstaendige/Eisentueren, fehlender Bodenkontakt, Fracht bei
Claim-Umwegen sowie Stop/Wiederaufnahme nach Section-Unload/Reload. Die
Serverbruecke prueft Besitzer, aktiven Auftrag, Entfernung und alle Tuerzellen.

Gruendung: exakte Rollen und Besitzer, keine Teilmutation bei ungueltigen
Plaetzen oder nur 17 freien Kreaturenplaetzen, unveraenderte Inventare,
Revisions-/ID-Ueberlauf, negative und weit entfernte Koordinaten, Wasserbloecke
und echte Fluessigkeiten, gebaute Startboeden mit erreichbarem Naturboden,
fremde Claims, ID23-Roundtrip und eingefrorene ID22-Migration. Tod, Entfernen,
Aufloesung und erneute Gruendung vergeben alte IDs nicht erneut. Echte
UDP-Anfragen pruefen Replikation aller 18 NPCs, Sichtbarkeit der zehn
Arbeitslosen in der Arbeitsverwaltung und Replay-Schutz.

## Nativer Ablauf

Der vorhandene KDE/uinput-Helfer bediente das wirkliche Clientfenster:
Bergfried per bestaetigtem Edit setzen, ueber die normale Interaktion oeffnen,
Namen eingeben, **Gruenden** und **Speichern** klicken, aktiven Bergfriedabbau
serverseitig ablehnen, bestaetigten Save anfordern, Anwendung beenden und mit
demselben isolierten Profil/Save neu starten. Nach dem Reload stimmen alle
18 IDs, die Rollen 3/2/3/10 und der Gruender mit dem gespeicherten Beleg ueberein.
Danach wurden Aufloesung, freigegebener Blockabbau und erneutes Speichern geprueft.

- [Erster Start](native-create.txt)
- [Reload und Aufloesung](native-reload.txt)
- [Persistierte Start-IDs und Rollen](kingdom-starters.json)
- [Headless-Server](headless.txt)
- [Maschinenlesbare Pruefergebnisse](verification.json)

Die sechs geprueften 1600x900-Aufnahmen liegen lokal unter
`target/kingdom-starters/run.59jhPo/captures/`. Der native Test wird nach
abgeschlossener Kompilierung ausgefuehrt, damit ein kalter Grafikstart nicht
mit einem Vollbuild um Ressourcen konkurriert. Mausbewegung und Klick werden
in getrennten Bevy-Frames ausgefuehrt. Dafuer wurde der bestehende OS-Helfer
verwendet.

## Wiederholen

```sh
cargo test --locked --offline --workspace --lib --bins --no-fail-fast -j 4 -- --test-threads=4 --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
cargo test --locked --offline --workspace --doc -j 4
cargo build --locked --offline -j 4 -p terra-client -p terra-server --bins
bash tools/benchmarks/run-kingdom-starters.sh
```

Der native Test legt neue Konfigurations-, Profil- und Weltordner unter
`target/kingdom-starters/` an. Spielerdaten werden nicht verwendet.
Client und Server benoetigen gemeinsam Protokoll136. Die Gruendungsbelege liegen
in Index ID23; alte Kingdoms erhalten beim Laden keine nachtraeglichen NPCs.
Globale Transportrouten, neutrale Nomaden und produktive generierte Doerfer
bleiben die getrennten offenen Aufgaben F69/F78/F79.
