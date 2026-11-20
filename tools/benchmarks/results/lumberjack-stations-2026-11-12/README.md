# Holzfaeller-Stationen, Laubbeute Und Dynamische Buttons

Abnahme vom 12.11.2026. Bedienung:
[Holzfaeller, Stationen und Setzlinge](../../../../docs/lumberjack-stations-and-drops.md).

## Gepruefte Funktionen

Neue Holzfaellerauftraege zeigen den fehlenden Berufstisch an, ohne beim
Speichern den NPC vorzeitig umzuziehen. Der NPC sucht eine echte Holzfaeller-Station
im Arbeitsgebiet oder beim Lager und muss sie physisch erreichen. Erst danach
nimmt er den Beruf an. Entfernen/Unload stoppt die Arbeit und erhaelt Inventar
und Teilbaum. Die bestehende Erntekette verwendet reale Wege, Aexte, Holz-/Laubdrops,
Lager und Setzlinge fuer die Nachpflanzung. Beide Geschlechtermodelle bleiben
an das dauerhafte Geschlecht der NPC-ID gebunden; sichtbare Avatare erneuern
sich beim replizierten Berufswechsel.

Alle normalen Laubpfade verwenden denselben stabilen 20-%-Wurf: direkter Abbau,
Gleisraeumung, verzoegertes Kronenaufraeumen und NPC-Arbeit. Zugeordnete Baeume
behalten ihre tatsaechliche Art. Nicht zugeordnete Blaetter verwenden die
Materialart, gemeinsames gruenes Material Eiche. Laub gibt seinen Block nicht
zusaetzlich zurueck; der garantierte Setzling aus Holz entfaellt.

Button-Inhalte und -Breiten bleiben horizontal und textabhaengig. Die native
Geometriepruefung kontrolliert die tatsaechlichen Caption-Grenzen, NoWrap,
nebeneinanderliegende Picker sowie ihre unterschiedlichen Inhaltsbreiten.
Der Stationshinweis steht auch direkt neben Speichern, wenn die Kopfzeile
bereits aus dem Scrollbereich verschwunden ist.

## Tests

4763 Workspace-Library-/Binary-Tests bestanden, 0 fehlgeschlagen. Zehn bestehende
Tests bleiben ignoriert. Ausschliesslich
`original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images`
ist wegen der bereits fehlenden externen Datei
`FIVERR/VoxelGrassBlock glb file.glb` ausgefiltert.

Neue Regressionen pruefen Stationsanforderung und falsche Tischtypen,
physischen Besuch vor Berufswechsel, reale Ernte/Nachpflanzung,
Inventar-/Teilbaumerhalt bei Verlust und Reload, beide Geschlechtermodelle,
Erneuerung bereits sichtbarer Avatare/Werkzeuge sowie Artzuordnung,
Kapazitaet und Einmaligkeit der Laubdrops. Nach einem im ersten Lauf gefundenen
Verteilungsfehler wurde der Produktionshash mit Avalanche-Mischung korrigiert;
die beiden urspruenglichen Testfaelle bleiben unveraendert. Zusaetzlich prueft
eine neue Regression negative Reihen und kleine Kronenbereiche. Die festen
Stichproben ergeben 2017/10000 im Raster und 2031/10000 auf einer negativen Reihe.

[Testsummen](tests.txt) und [maschinenlesbare Ergebnisse](verification.json).

## Native Abnahme

Ein isolierter Client mit wirklichem lokalem UDP-Server gruendet ein Kingdom,
oeffnet die Arbeitskraft per Rechtsklick und waehlt Gebiet/Lager ueber reale
Raycasts. Die bisherigen Auswahl-/Abbruch-/Zuweisungs-/Pausenpruefungen bleiben
erhalten. Anschliessend wird derselben Arbeitskraft ein Holzfaellerauftrag
erteilt: Der Server meldet `MissingStation`, die alte Berufsvariante bleibt
erhalten. Ein echter Blockedit platziert die Holzfaeller-Station bei [8,12,25].
Nach dem Stationsbesuch repliziert der Server den neuen Beruf; der sichtbare
Avatar wird mit derselben NPC-ID und unveraendert weiblichem Geschlecht erneuert.

Neun NPC-/Spielaufnahmen bei 1600x900 und 960x640 sowie vier weitere
DE/EN-Hauptmenue-/Settingsaufnahmen bei 960x640 wurden erstellt und die
Sprachpraeferenz gespeichert. Die Eingaben verwenden framegesteuerte
Gameplay-Tasten/Maustasten und die registrierten TiltUI-Handler. Weltedits,
Inventare, NPCs und Bestaetigungen stammen aus dem echten Serverpfad.
Die Ernte-/Nachpflanzkette pruefen die automatisierten Welt-Tests mit realen
Voxeln, Bewegung, Werkzeugverschleiss, Inventaren und gespeicherten Zustaenden.

- [Buttons mit Textbreite](npc-direct-de-wide.png)
- [Sichtbare Stationsanforderung](npc-lumberjack-missing-station-compact.png)
- [Replizierter Holzfaeller und unveraendertes weibliches Geschlecht](npc-lumberjack-profession-compact.png)
- [Settings-Aktionszeile auf Deutsch](gameplay-de.png)
- [NPC-/Stationsprotokoll](native-npc.txt), [Sprachgalerie](native-language.txt)
- [Headless-Protokoll](headless.txt)

Alle Aufnahmen liegen lokal unter `target/npc-work-direct/run.UUuJv5/captures/`
und `target/ui-language/run.ng7k5c_6/captures/`. Profil, Welt und Konfigurationen
sind isoliert. GPU: AMD Radeon RX 6950 XT, Vulkan/RADV.

## Wiederholen

```sh
cargo fmt --all --check
cargo check --locked --offline --workspace -j 4
cargo test --locked --offline --workspace --lib --bins --no-fail-fast -j 4 -- --test-threads=4 --skip original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
cargo clippy --locked --offline --workspace --all-targets -j 4 -- -D warnings
cargo test --locked --offline --workspace --doc -j 4
cargo build --locked --offline -j 4 -p terra-client -p terra-server --bins
bash tools/benchmarks/run-npc-direct.sh
env -u DISPLAY -u WAYLAND_DISPLAY target/debug/terra-server --smoke-test
```

Den nativen Lauf nach dem Build auf einem Grafikdisplay starten. Der Dedicated
Server benoetigt kein Display. Beide Seiten verwenden Protokoll138; es gibt
keine neuen Speicherfelder. Vorrat muss real vorhanden sein. Die begrenzte
Stationssuche arbeitet in geladenen Bereichen; ein Tisch direkt beim Lager
wird schneller entdeckt als am Ende eines maximalen Arbeitsgebiets.
