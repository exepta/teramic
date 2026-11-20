# NPC-Arbeit, Patrouillen Und Kingdom-UI: Abnahme Vom 12.11.2026

Direkte Arbeitszuweisung, private NPC-Inventaransicht, Build-Patrouillen,
sichtbare entfernte Dorflichtquellen und Kingdom-Zugriff mit K.
Bedienung und Grenzen: [NPC-Interaktion und Kingdom](../../../../docs/npc-interaction-and-kingdom.md).

## Ergebnisse

| Pruefung | Ergebnis |
| --- | --- |
| `cargo fmt --all --check` | Erfolgreich |
| `cargo check --locked --offline --workspace -j 4` | Erfolgreich |
| Workspace-Library-/Binary-Tests | 4750 bestanden, 0 fehlgeschlagen, 10 bestehende ignoriert, 1 ausgefiltert |
| `cargo clippy --locked --offline --workspace --all-targets -j 4 -- -D warnings` | Erfolgreich |
| `cargo test --locked --offline --workspace --doc -j 4` | Erfolgreich, keine Doctest-Faelle vorhanden |
| Client-/Server-Binaerbuild | Erfolgreich |
| Dedicated Server ohne DISPLAY/Wayland | Erfolgreich, 27 Sections bereit |
| Nativer NPC-/Kingdom-Test mit lokalem UDP-Server | Erfolgreich, sieben DE/EN-Aufnahmen in zwei Fenstergroessen |
| Native Hauptmenue-/Gameplay-Sprachgalerie | Erfolgreich, vier 960x640-Aufnahmen, gespeicherte Sprachwahl |
| Fluent-Kataloge | 3247 zweisprachige Eintraege, 309 Muster, 0 ausstehend |

Ausgenommen bleibt ausschliesslich
`original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images`:
Die bereits zuvor fehlende externe Datei `FIVERR/VoxelGrassBlock glb file.glb`
liegt nicht im Repository. Es wurde keine weitere Pruefung deaktiviert.

## Gepruefte Faelle

- Lichtentdeckung hinter vielen leeren Himmels-Sections, begrenzte deterministische
  Quellenauswahl, Edit-/Unload-Entzug, Tiefentestmaterial, weiche Distanzuebergaenge
  und fern-zu-nah-Reihenfolge transparenter Lichtpunkte.
- Alle drei eigenen Soldatenrollen patrouillieren ohne Bewohnerstelle und Bett;
  Gebietsclaims, fremde/freigegebene Claims, explizite Befehle, Unload und
  Aufloesung stoppen die automatische Bewegung. Tueren, Crowd-Separation und
  erzwungene Entpenetration nach Blockaenderungen erhalten die Build-Grenzen.
- Private Inventare bleiben an Besitzer-/Claimrechte, acht Meter Reichweite,
  Spielerzustand und geladene NPC-Zellen gebunden. Keine Inventare fremder NPCs
  oder aus der allgemeinen Arbeitsliste. Ein direkt gewaehlter NPC bleibt
  trotz 128 bereits vorhandener Listeneintraege erreichbar.
- Echte UDP-Anfragen verwalten den eigenen ungeladenen Bergfried aus 128 Metern
  Entfernung. Fremder Bergfried und nachtraeglich freigegebenes Aufenthaltsgebiet
  werden abgewiesen. Tastenmigration erhaelt bestehende eigene K-Belegungen.
- Lange Beschriftungen erhalten Zeichenumbruch; Text-/Control-Geometrie,
  editierbare Werte und explizite einzeilige Beschriftungen bleiben erhalten.

## Nativer Ablauf

Der Client setzt in einer isolierten flachen Welt einen echten Bergfried und
gruendet serverseitig ein Kingdom. Die Kingdom-Taste oeffnet dessen bestaetigte
Verwaltung. Eine reale zivile F80-Startarbeitskraft wird anvisiert und durch
Rechtsklick geoeffnet; ein zusaetzlicher Test-NPC war nicht erforderlich.

Der Test prueft neun private Inventarplaetze, den HUD-Auswahlmodus, ESC-Abbruch
mit erhaltenem Entwurf sowie echte Raycast-Auswahl zweier Ecken und einer
serverseitig platzierten Truhe. Er vergleicht Edit-Bestaetigungen und Blockwerte,
damit die Auswahl keine Weltedits ausloest. Zuweisung und Pause werden vom
wirklichen Server bestaetigt.

Die Eingaben verwenden framegesteuerte Gameplay-Tasten/Maustasten und die
registrierten TiltUI-HTML-Handler; dies ist kein OS-Zeigertest. Es werden keine
NPCs, Inventarantworten oder Netzwerkbestaetigungen im Client erfunden.
Geometriepruefungen sichern Viewport, Rahmenabstand, Editorbreite, neun Slots,
Aktionsbreiten und Erreichbarkeit durch Scrollen. DE/EN werden bei 1600x900
und 960x640 aufgenommen. Die separate Sprachgalerie prueft reale Auswahl,
Apply, gespeicherte Profilpraeferenz und Hauptmenue-Rueckkehr bei 960x640.

- [NPC Deutsch, breit](npc-direct-de-wide.png)
- [NPC Deutsch, kleines Fenster mit sichtbarer Speicheraktion](npc-direct-de-compact.png)
- [Groesserer Kingdom-Dialog](kingdom-unbound.png)
- [Gameplay-Einstellungen Deutsch, kleines Fenster](gameplay-de.png)
- [NPC-Laufprotokoll](native-npc.txt), [Sprachgalerie](native-language.txt)
- [Headless-Protokoll](headless.txt), [Testsummen](tests.txt)
- [Maschinenlesbare Ergebnisse](verification.json)

Alle elf Galerieaufnahmen liegen lokal unter
`target/npc-work-direct/run.eieFbY/captures/` und
`target/ui-language/run.42vixi14/captures/`; der zusaetzliche Gruendungsdialog
liegt im ersten Ordner. Die Tests verwenden ausschliesslich eigene Profil-,
Konfigurations- und Weltordner. GPU: AMD Radeon RX 6950 XT, Vulkan/RADV.

## Wiederholen Und Grenzen

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

Den nativen Test nach dem Build mit verfuegbarem Grafikdisplay ausfuehren.
Inventare sind lesend; Versorgung erfolgt ueber Auftragslager. Fernlichter zeigen
geladene Quellen innerhalb eines Qualitaetsbudgets; der echte Nahlichtpool bleibt
begrenzt. Patrouillen arbeiten in geladenen eigenen Build-Claims. Zufriedenheit
und weitere Statistiken bleiben F81 in der TODO. Client und Server benoetigen
gemeinsam Protokoll137; das Speicherformat bleibt unveraendert.
