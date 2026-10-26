# Pets, Ganzbaumfaellen, Wasserfaelle Und Spielerorte

Stand: 2026-10-24, Phase 120. Die folgenden Ergebnisse stammen aus ausgefuehrten
Tests; dieser Bericht behauptet keine Langzeit-FPS-Messung.

## Bedienung Und Architektur

- F33: Mit Sneak + Platzieren auf dem eigenen gezaehmten Tier oeffnet sich ein
  dunkel-goldenes Radialmenue mit Follow, Rest und None. Follow nutzt die
  bestehende Navigation, Rest bleibt stehen, None erlaubt autonomes Verhalten.
  Besitzer, Reichweite, Sichtlinie und gueltiger Tierzustand werden auf dem Server
  geprueft. Alte Follow-Felder aktivieren keinen Befehl; nur explizite Pet-Auftraege
  werden als solche gespeichert. Das Portraet verwendet das vorhandene Tiermodell.
- F08: Survival-Abbau mit Sneak am natuerlichen Stammfuss braucht 0,5 Sekunden
  je zugeordnetem Holzblock. Die Zuordnung stammt aus den vorhandenen vorbereiteten
  Baumprimitiven, nicht aus einem Holz-Floodfill. Spieler-Overrides bleiben erhalten.
  Holz und nachgelagerter Laubzerfall verwenden begrenzte Arbeitsbudgets. Der
  regulaere Weltabschluss beendet angenommene Arbeiten vor dem finalen Save.
- Wasser: Natuerliche Wasserfallkanten erhalten eine gemeinsame Eckversetzung;
  echte Fluessigkeitszellen verwenden auch neben Teilbloecken gemeinsame
  Oberflaechenkanten. Nur tatsaechlich wassergefuellte Slabs/Treppen werden auf ihre
  freie Restgeometrie zugeschnitten. Verdeckte Innenflaechen werden entfernt.
  Die bestehende Fluessigkeitssimulation und ihre Update-Budgets bleiben erhalten.
- Spielerort: Der Server erfasst die Kapselmitte, Blickrichtung und den erlaubten
  Flugzustand pro Spieler-UUID im bestehenden Weltjournal. Metadaten-Updates loesen
  keine Terraininvalidierung aus. Vor dem Einstieg wird gegen dieselben geformten
  Block-Kollisionsboxen wie bei der Bewegung geprueft. Hoehlen, Wasser und freie
  Luftpositionen werden nicht auf einen Oberflaechenspawn umgebogen. Bei lokalen
  Hindernissen wird eng benachbarter freier Raum versucht, sonst der sichere Spawn.

## Vertraege Und Grenzen

Protokoll 70 erfordert zusammen aktualisierte Clients und Server. PLY8 erweitert
den Spielerpayload um Ortsdaten; Spieler ohne Ortsdaten bleiben PLY7, alle bisher
unterstuetzten Spielerpayloads bleiben lesbar. RGA5 speichert explizite Pet-Auftraege
auf dem bestehenden regionalen Speicherpfad. Welt-/Block-IDs, Generatorversion,
Welt-Saveformat, Abhaengigkeiten und die existierende Wasser-Engine bleiben erhalten.

Ganzbaumfaellen ist auf 4096 Holz- und 16384 Laubzellen je Baum, acht aktive
Arbeiten sowie globale 32 Holz-/64 Laub-Edits je Arbeitsschritt begrenzt. Spielerbauten
werden nicht als Baum erkannt. Normales Einzelblock-Abbauen bleibt erhalten.
Der Laufzeitauftrag selbst ist nicht crash-resumierbar; bereits bestaetigte Edits
und Drops sind normale persistente Weltzustaende. Bei regulaerem Verlassen wird
der Auftrag beendet. Ein voller Drop-Speicher wird als Fehler gemeldet statt
Ressourcen still zu verwerfen.

## Validierung

Alle folgenden Befehle wurden erfolgreich ausgefuehrt. Cargo lief mit einem
Build-Job, ohne parallele schwere Builds:

```bash
cargo fmt --all --check
CARGO_BUILD_JOBS=1 cargo check --locked --workspace --all-targets
CARGO_BUILD_JOBS=1 cargo test --locked --workspace --no-fail-fast
CARGO_BUILD_JOBS=1 cargo clippy --locked --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=1 cargo build --locked --workspace
CARGO_BUILD_JOBS=1 cargo test --locked -p terra-storage --lib -- --test-threads=1
target/debug/terra-server --no-save --smoke-test
```

- [Workspace-Tests](tests.log): 2261 bestanden, keine Fehler, drei bestehende
  opt-in Tests ignoriert. Abgedeckt sind unter anderem autoritative Pet-Befehle,
  Besitzer-/Sichtpruefung, Menue-Eingabe, Ganzbaumabbau, Section-Grenzen,
  Spieler-Overrides, Drops, Wassergeometrie sowie Spielerorte und alte Saves.
- [Separater Speicherlauf](storage-pure.log): 86 Tests bestanden, auch ohne die
  durch den gesamten Workspace vereinigten Bevy-Features.
- [Check](check.log), [Clippy](clippy.log), [Build](build.log) und
  [Formatierung](fmt.log) erfolgreich.
- [Headless-Server](headless-server.log): erfolgreicher kurzer Lauf mit 27 Sections.

### Native Clientpruefung

Beide kurzen Laeufe verwendeten Vulkan auf der AMD Radeon RX 6950 XT, isolierte
Konfigurationen und Test-Speicherverzeichnisse. Die erzeugten Bilder wurden auch
visuell angesehen. In diesen beiden Clientlogs und im Headless-Log stehen keine
Warnungen, Fehler, Panics oder Slab-Allocator-Fehler.

- [Pet-Test](native-pets.log), etwa sechs Sekunden: echtes TiltUI, reale
  UDP-Befehle, Besitzerpruefung, Legacy-Migration, Follow/Rest/None, Modellportraet
  und Escape-Verhalten. Explizites Follow bewegte das Tier um 1,109 m. Drei Bilder:
  [Rest](native/pets/pet-rest.png), [Follow](native/pets/pet-follow.png),
  [None](native/pets/pet-none.png).
- [Wasserfall-Test](native-water.log), etwa 20 Sekunden: echter 21 m hoher
  Fluessigkeitsfluss ueber X-/Y-Section-Grenzen, mit Slab, Treppe, Zwischenbecken
  und unterem Becken. Nach Schliessen des Zulaufs sank der gemessene nachgelagerte
  Wasserbestand von 959 auf 477; die Quelle blieb unveraendert. Zwei Bilder:
  [Uebersicht](native/water/waterfall-stepped-overview.png),
  [Teilblock-Kante](native/water/waterfall-partial-step-close.png).

Reproduzieren, vom Repository-Verzeichnis aus:

```bash
export TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/pets-trees-water-location-2026-10-24/configs"
export TERRA_MENU_CONFIG_DIR="$PWD/target/phase120-test-menu"
export WGPU_BACKEND=vulkan
# Fuer jeden Pet-Test ein neues, noch nicht vorhandenes Save-Verzeichnis verwenden.
TERRA_PET_SMOKE_DIR="$PWD/target/phase120-pets-fresh-save" \
  TERRA_CAPTURE_DIR="$PWD/target/phase120-pet-captures" \
  target/debug/terra-client --smoke-test-pet-commands
TERRA_CAPTURE_DIR="$PWD/target/phase120-water-captures" \
  target/debug/terra-client --smoke-test-waterfalls
```

### Aussagegrenzen

Der Wasserfall ist eine reproduzierbare konstruierte Testszene, nicht die exakte
urspruengliche Hoehle aus dem Nutzerbild. Ein separater visueller Ganzbaumtest und
ein manueller nativer Welt-Neustart mit Positionsvergleich wurden nicht
durchgefuehrt. Ortsdaten sind durch Speicher-Roundtrips, echte UDP-Wiederherstellung
zweier Spieler, Controller-Tests und Autosave-Tests geprueft. Der native Pet-Lauf
hat zusaetzlich eine echte integrierte Testwelt beim Verlassen gespeichert.

Der Netzwerk-Ganzbaumtest prueft die echte erforderliche Dauer und kuerzt danach
nur in seiner Test-Fixture den Wartezeitpunkt ab; im Spiel gibt es keinen solchen
Skip. Keine 10-, 15- oder 30-Minuten-Laeufe und keine FPS-Vorher-/Nachher-Messung
wurden durchgefuehrt. Fruehere Diagnoseversuche in `tree-network*.log` sind keine
abschliessenden Testergebnisse; massgeblich ist der erfolgreiche Workspace-Lauf.
