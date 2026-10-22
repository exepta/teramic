# Phase 43: Player Stats, Survival Und HUD

## Umsetzung

TODO-Etappe 1 erweitert die vorhandenen Spielerzaehler um serverautoritative
Gesundheit, Ausdauer, Hunger, Sauerstoff und Todeszaehler. Kein zweites Inventar,
kein neuer Spieler-Speicher, keine Terrain-/Generatoraenderung. Creative ist
ausgenommen. Gemeinsame begrenzte Regeln liegen in `terra-core::survival`,
Bewegungslimits in derselben Avian-`CharacterSimulation` fuer Authority und
Prediction/Replay. Schadensberechnung bleibt auf dem Server.

Sprint, Sprung und aktives Schwimmen verbrauchen Ausdauer und Nahrung.
Erschoepfung verwendet eine 25-Prozent-Freigabehysterese, sperrt aber niemals
Auftauchen oder Uferausstieg. Heilung setzt mindestens 75 Hunger und vier
schadensfreie Sekunden voraus. Fallhoehe oberhalb 3 m kostet 8 HP/m.
20 Sekunden Luft, danach 8 HP/s Ertrinkenschaden; an Luft Wiederauffuellen.
Ohne Nahrung 1 HP/s Schaden. Zahlen und Bedienung sind in der Haupt-README
unter **Player Stats Und Survival** dokumentiert.

Die bereits vorhandenen kleinen Pilz-Ressourcen sind die erste echte Nahrung.
R verwendet einen ausgewaehlten Hotbar-Pilz fuer 18 Hunger, frei rebindbar.
Servervalidierte Slotrevision und atomare Entnahme verhindern Replay-Duplikate.
Die Keule ist das Hungersymbol, kein vorgetaeuschtes neues Fleischitem.

Tod sperrt Bewegung/Interaktionen/Aufnahme. Ein servervalidierter Respawn
verwendet die vorhandene trockene Festland-Spawnpruefung, kollisionsbereites
Ziel, Chunk-Lade-/Teleport-Handshake und neue Bewegungsgeneration. Zwei
Sekunden Schutz; Inventar/Cursor/Zaehler bleiben erhalten. Reconnect heilt
nicht. Regelmaessige Vitalwerte markieren kein Terrain als geaendert.

## Speicherung Und Replikation

`PlayerStats` erweitert sich in denselben UUID-/Weltdateien. Neuer Spielerdaten-
Record `PLY2`, expliziter Decoder fuer alte `PLYR`-Records mit vollen neuen
Defaults. Inventare, Cursor und bisherige Zaehler bleiben erhalten. Regionen,
Weltformat 2, IDs, Seed und Generatorversion 5 bleiben gleich. `PLY2` ist keine
kompatible Datei fuer alte Programme; Backups wie ueblich bei gestopptem Host.

Servercommits verwenden `bypass_change_detection` und eine eigene Metadaten-
Revision im bestehenden Journal. So loest ein Staminatick keine Terrain-/
Wasser-/Collider-Invalidierung aus. Dedicated-Autosave erkennt die Revision
auch bei pausierter Weltuhr; Integrated verwendet seinen bestehenden
Hintergrund-/Shutdown-Save. Keine zwingende Renderabhaengigkeit im Server.

Protokoll 17 ergaenzt generationengebundene Stats/Respawn-Nachrichten sowie
Vitals in Bewegungssnapshots. Owner-Stats maximal 10 Hz bei Aenderung,
vorhandene Bewegungssnapshots fuer Prediction weiterhin 20 Hz bei 60-Hz-Server.
Nur erfolgreiche Simulationsschritte verbrauchen Zeit. UUIDs, Bagrevisionen und
Bewegungsgeneration verhindern Vermischung bei Reconnect/Respawn.

## TiltUI Und Art

HUD unten links, unabhaengig von Debug/F1: Herz/HP, Blitz/Ausdauer, Keule in
fuenf Bissstufen bis zum Knochen, optionale Atemanzeige. Dazu Schadensflash,
Erschoepfung und echter Respawn-Dialog mit Warte-/Fehlerzustand. Kein weiteres
UI-Framework, keine manuell erzeugten Ersatz-Controls. Das vorhandene TiltUI-
ImageNode-Bridge-Muster waehlt Rects aus einem gemeinsamen RGBA-Atlas.

Bitmapasset: `src-ui/assets/survival-atlas.png`, 1774x887 Pixel, echte Alpha-
Transparenz. Built-in `image_gen`, kein CLI/API-Fallback. Produktionsspezifikation
und Reproduktionsprompt: `src-ui/assets/survival-atlas.txt`. Der Atlas bleibt
im Repository und wird nicht aus einem privaten/temporaeren Pfad geladen.

## Tests

Alle fuenf Cargo-Pruefungen erfolgreich, Buildjobs auf zwei begrenzt:

```bash
cargo fmt --all --check
env CARGO_BUILD_JOBS=2 cargo check --workspace
env CARGO_BUILD_JOBS=2 cargo test --workspace
env CARGO_BUILD_JOBS=2 cargo clippy --workspace --all-targets -- -D warnings
env CARGO_BUILD_JOBS=2 cargo build --workspace
```

660 Tests bestanden, ein bereits vorhandener 180-s-UDP-Soak ignoriert.
Neue Tests pruefen begrenzte Regeln und ungueltige Zeitwerte, Sprint-/Jump-
Limits trotz Clientpaketen, Fallschaden/Tod nur einmal, Creative-Ausnahme,
reales Tauchen/Ertrinken an der Koerperposition, atomare Nahrung und Replay-
Ablehnung, getrennte Werte fuer zwei UDP-Clients, toten Reconnect, sicheren
Respawn und veraltete Generationen. Storage prueft echte alte Binaerrecords,
volle Vitaldaten nach Save/Reload, identische Inventare sowie dieselbe UUID
in zwei verschiedenen Welten. Dedicated prueft Autosave bei pausierter Uhr
ohne Terrain-Change-Flag. UI prueft den echten TiltUI-CSS-Parser, alle fuenf
Atlaszellen, F1-Unabhaengigkeit, Death-Gating und Mauseinfang nach Respawn.

Fruehe Testiterationen haben fehlende Client-Koerper in der Headless-
Teleportfixture und noch nicht eingetroffene Inventarsnapshots aufgedeckt;
Fixtures wurden auf den tatsaechlichen Lebenszyklus angepasst. Finaler
Workspace-Lauf einschliesslich dieser Tests erfolgreich.

## Native Pruefung

RX 6950 XT, RADV, Vulkan, Wayland. Zwei erfolgreiche native Durchlaeufe mit
echtem Integrated Server und isolierten frischen Welt-/Konfigurationspfaden:
1280x800 logische Pixel (1920x1200 Capture bei 150 Prozent DPI) und 800x600
(1200x900 Capture). Kein CSS-/Shaderfehler. Gepruefter Ablauf:

1. Gespeicherte Anfangswerte und fuenf Pilze werden tatsaechlich geladen.
2. Fuenf echte Inventaraktionen fuellen Hunger schrittweise; alle Bissstufen
   werden aufgenommen und der Stack schrumpft bis null.
3. Reale Lauf-/Sprintinputs verbrauchen Ausdauer bis zur Erschoepfung;
   danach Erholung und wieder freier Sprint.
4. Autorisierter Testflug mit anschliessendem wirklichem Fall fuehrt zu Tod.
5. Der gerenderte TiltUI-Button loest den echten Respawn-Handshake aus;
   alle Vitalwerte voll, neue Generation, gleiche Inventardaten und Mausfang.
6. Finaler Host-Save/Shutdown erfolgreich.

Der erste native Versuch blieb wegen der allgemeinen Smoke-Test-Maussperre
beim Laufabschnitt stehen. Der Survival-Smoke darf nun gezielt den echten
CaptureRequest-Pfad benutzen; normales Gameplay bleibt unveraendert. Die
anschliessenden beiden Durchlaeufe waren erfolgreich. Dieser erste Fehlversuch
ist im archivierten Log getrennt erkennbar.

Aufnahmen von Bissstufen, Erschoepfung, Tod und Respawn wurden geoeffnet. Das
HUD ist unten links lesbar, Hotbar/Respawn-Button werden nicht ueberdeckt.
Native Sauerstoff-/Unterwasseransichten wurden in dieser Etappe nicht separat
aufgenommen; Atem-/Ertrinkenlogik ist ueber reale UDP-/Wasserphysiktests geprueft.

```bash
env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra-survival-menu-fresh \
  TERRA_SURVIVAL_SMOKE_DIR=/tmp/terra-survival-world-fresh \
  TERRA_CAPTURE_DIR=/tmp/terra-survival-captures \
  target/debug/terra-client --smoke-test-survival

env -u DISPLAY -u WAYLAND_DISPLAY \
  target/debug/terra-server --smoke-test --no-save --bind 127.0.0.1:0
```

`TERRA_SURVIVAL_SMOKE_DIR` muss frisch sein. Benutzerwelten werden nicht
veraendert. Logs liegen neben diesem Bericht, Bilder unter `captures/` und
`captures-compact/`.

## Grenzen Und Kosten

Keine neue FPS-/p95-/p99-/RAM-/VRAM-Messung und keine behauptete Performance-
Paritaet. Die nativen Laeufe sind Funktionstests, kein Streamingbenchmark.
Der neue unkomprimierte RGBA-Atlas belegt rechnerisch rund 6 MiB Texeldaten
ohne Mips/Allocator-Overhead. Es gibt keine UI-Entity pro Bissstufe und keine
Render-/Audioabhengigkeit im Dedicated Server. Stats werden nicht zusammen
mit allen Drops pro Physiktick neu gesendet.

Weitere Nahrungsmittel, Fleisch/Tierdrops und Kochen folgen mit Tier-KI und
Crafting. Kein PvP-/Ruestungs-/Statuskrankheitssystem. Inventarverlust beim
Tod ist bewusst noch keine Spielregel. Die allgemeine Sprachwahl fehlt
weiterhin; Texte folgen der bisherigen englischen Gameplay-UI. Reale
Internet-/Mehrrechner-Tests und mehrstuendige Lastabnahme bleiben offen.
