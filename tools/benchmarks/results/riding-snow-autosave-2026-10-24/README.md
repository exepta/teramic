# Pferde Reiten, Schnee Und Autosaves (Phase 117)

## Umfang

- F07: Reiten eigener gezaehmter erwachsener Pferde mit WASD, Sprint,
  individuellen echten Spruengen und sicherem Absteigen mit Shift.
- Trockenes aufgenommenes Stalker-Atem-Foley statt der bisherigen
  Rausch-/Sinussynthese.
- Breitere bestehende Hoehlenconnectoren fuer neue Welten.
- Blockweise gestaffelte Schneeakkumulation, reversible Prop-Verdeckung ab
  Schneeschicht 3 und gestaffeltes Ausblenden von Herbstblumen.
- Korrekte Speicherinvarianten fuer natuerliche Hoehlen-Stalker und
  kinematische Tierbewegungen.
- Cherry Forest, Spirit Forest, natuerlichere Hoehlenkammern, Schmetterlinge,
  Spirit Gloom und Skelettkrieger sind als F34-F39 eingetragen. Diese neuen
  Biome/Mobs sind ausdruecklich noch nicht implementiert.

## Reiten

Die normale Platzier-/Tierinteraktion besteigt ein eigenes Pferd, wenn kein
bevorzugtes Futter angeboten wird. Futter behaelt seine Zaehm-/Zuchtfunktion.
Wilde, fremde, junge und tote Pferde werden abgewiesen. Ein Pferd kann genau
einen Reiter tragen. Reichweite, Sichtverbindung und Kopffreiheit prueft der
Server ueber die vorhandenen Interaktions-/Voxelregeln.

`terra_world::creatures::riding` erweitert den bestehenden Pferdemotor, keine
zweite Physik: echte Gravitation, Sweeps, Treppen-/Slab-Unterstuetzung und
Schwimmen. Server und Client-Replay verwenden dieselbe Funktion. Sprint kostet
4 Ausdauer/s, ein Sprung 8; normales Reiten regeneriert 5/s. Die beim Spawn
festgelegten 65..200 Kapazitaet und 1,1..2,25 m Sprunghoehe bleiben unveraendert.
Die bestehende Stamina-Anzeige zeigt waehrend des Reitens Pferdeausdauer.

Shift sucht seitlich/hinter dem Pferd einen geladenen, gestuetzten und ueber
einen freien Spielerpfad erreichbaren Ausstieg. Fehlt ein sicherer Ort, bleibt
der Spieler aufgesessen. Teleport, Tod, Disconnect oder Besitzverlust loesen
den Laufzeitzustand. Pet-Folgen wird nicht aktiviert.

Sitzpose, Pferd, First-Person-Kamera und entfernte Avatare teilen dieselbe
Darstellungsposition. Die vorhandenen gegliederten Spieler-Meshes verwenden
gebeugte Knie und eine Reitpose statt der Gehbewegung.

## Autosave-Ursachen

Ein Hoehlen-Stalker kann korrekt spawnen, obwohl kein Oberflaechenhabitat
geladen bzw. als erkundet markiert wurde. Die bisherige Populationsvalidierung
forderte trotzdem Oberflaechenhistorie. Sie erkennt jetzt die exakt validierte
natuerliche Hoehlen-ID, ohne Oberflaechenzellen faelschlich zu markieren und
dadurch spaetere Herden zu unterdruecken.

Zusaetzlich konnten kinematische Spinnen-Wand-/Deckenwechsel und Uferaufstiege
ihren Koerper-Pivot bzw. Step-Up als reale Vertikalgeschwindigkeit speichern.
Bei 60 Hz ueberschritt das die gueltige Geschwindigkeitsgrenze. Die Produzenten
speichern jetzt echte tangentiale Bewegung bzw. Bodenunterstuetzung; normale
Gravitation bleibt erhalten. Die Save-Validierung wird nicht abgeschaltet und
ungueltige Daten werden nicht pauschal durch Abschneiden kaschiert.

## Schnee Und Saisonale Props

Die bestehende autoritative Schneeakkumulation verwendet stabile lokale
Raten von 0,55..1,45 aus Seed und globaler Blockposition. Dadurch werden anfangs
einzelne Bloecke bedeckt und nicht komplette Chunkflaechen gleichzeitig.
Negative Koordinaten, Reload und Generierungsreihenfolge aendern den Rang nicht.

Natuerliche Wetterkappen benutzen den bestehenden Editpfad ohne die sonst
korrekte dauerhafte Prop-Entfernung beim Bauen auszuloesen. Bodengebundene Props
bleiben bei Schicht 1/2 sichtbar und selektierbar; ab Schicht 3 werden sie
reversibel verborgen. Nach Schmelzen erscheinen sie wieder. Spielerbauten
entfernen ueberdeckte Props weiterhin normal.

Blumen verwenden einen stabilen individuellen Rang und sanfte saisonale
Coverage. Rendering, Tiefen-/Schattenpass, Picking und Bienen-Nektarsuche stimmen
ueberein. Der Rang liegt im bestehenden Vertex-Alpha-Kanal. Der gemeinsame
Weather-Buffer hat sechs Vec4 (96 Byte), keine neue Entity pro Pflanze und
keinen saisonbedingten Terrain-Rebuild.

Globale shaderbasierte Schneeaufhellung ist im normalen Spiel abgeschaltet.
Nur `CosmeticSnowPreview` in einer expliziten lokalen Galerie erlaubt die alte
kosmetische Vorschau. Server/Replica-Welten koennen sie auch mit Marker nicht
aktivieren. Physische lokale Schneekappen bleiben davon unabhaengig.

## Hoehlen Und Audio

Neue Welten verwenden Hoehlenstrukturrevision 6. Der vorhandene
Connector-Breitenparameter wird mit 1,4 multipliziert. Seed, Netzverlauf und
Biome-Sampling bleiben gemeinsam; keine zweite Hoehlenengine. Bestehende
Revisionen 0..5 behalten ihre exakten bisherigen Felder und gespeicherten Edits.
Der Faktor ist ein Feldparameter, keine Behauptung eines ueberall exakt 40 %
groesseren geometrischen Tunneldurchmessers.

`assets/audio/animals/stalker-airway.wav` ist ein bearbeiteter trockener
Atem-/Airway-Ausschnitt einer bereits lizenzierten CC0-Aufnahme von Scorcher24.
Die vorhandene Ladeaufgabe erzeugt asymmetrisches Ein-/Ausatmen mit Pausen.
Keine neue menschliche Sprecheraufnahme; subjektive In-Game-Hoerabnahme steht
aus. Herkunft und reproduzierbare Aufbereitung stehen in
`assets/audio/CREDITS.md` und `tools/assets/prepare-stalker-breath.sh`.

## Kurze Native Pruefungen

Hardware: AMD Radeon RX 6950 XT, RADV/Mesa 26.2.4, Vulkan, 1920x1080.
Keine 10-/15-/30-Minuten-Spieltests und keine behaupteten FPS-Messwerte.

`--smoke-test-riding` startet eine isolierte Survival-Testwelt mit echtem
integriertem UDP-Server. Der erfolgreiche Lauf dauerte rund 8,7 Sekunden:
Aufsteigen, Sitzabgleich unter 2 cm, echte Vorwaerts-/Sprint-/Sprungeingaben,
serverseitige Bewegung/Ausdauer, Absteigen und abschliessendes Speichern.
Gemessene Sprunghoehe: Prediction 2,0159 m, Autoritaet 2,0135 m.
Vier Aufnahmen wurden tatsaechlich visuell kontrolliert:

- [Sitzpose](native/riding-seated-third-person.png)
- [First Person](native/riding-seated-first-person.png)
- [Sprint/Sprung](native/riding-sprint-jump.png)
- [Abgestiegen](native/riding-dismounted.png)

Die Saison-Galerie erfasst sieben GPU-gepruefte Jahreszeit-/Schneevorschauen
ueber unveraenderten Terrain-Meshes. Sie ist eine explizite lokale
Shader-Galerie, kein Nachweis fuer physische Schneeakkumulation; letztere
pruefen die autoritativen Regressionstests.

Der Dedicated-Server-Smoke startete ohne Fenster/Renderer, generierte 27
Sections und beendete sich erfolgreich. Die beiden nativen Clientlaeufe sowie
der Serverlauf meldeten keine Warnung, keinen Fehler und keine Panic.

Reproduzieren, vom Repository-Root aus; `target/riding-check-new` darf noch
nicht existieren. Die Testwelt ist getrennt von normalen Spielstaenden:

```bash
env TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/riding-snow-autosave-2026-10-24/configs" \
    TERRA_MENU_CONFIG_DIR="$PWD/target/riding-check-config" \
    TERRA_RIDING_SMOKE_DIR="$PWD/target/riding-check-new" \
    TERRA_CAPTURE_DIR="$PWD/target/riding-check-captures" \
    WGPU_BACKEND=vulkan \
    target/debug/terra-client --smoke-test-riding --gender=f
```

`--smoke-test-seasons` startet stattdessen die lokale Shader-Galerie.
`target/debug/terra-server --no-save --smoke-test` prueft den headless Server.

## Tests

Alle folgenden Befehle bestanden auf dem abschliessenden Stand, mit nur einem
gleichzeitig aktiven Cargo-Buildjob:

```bash
cargo fmt --all --check
CARGO_BUILD_JOBS=1 cargo check --locked --workspace --all-targets
CARGO_BUILD_JOBS=1 cargo test --locked --workspace --no-fail-fast
CARGO_BUILD_JOBS=1 cargo clippy --locked --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=1 cargo build --locked --workspace
git diff --check
```

Workspace-Suite: **2162 bestanden, 0 fehlgeschlagen, 3 ignoriert** in 23
Test-/Doc-Test-Suiten. Native Reitprobe, finale Saison-Galerie und headless
Serverprobe beendeten sich jeweils mit Exitcode 0. Keine Langzeittests.

Die abschliessenden Logs liegen neben diesem Bericht. Zentrale Regressionen:

- Natuerlicher Hoehlen-Stalker ohne Oberflaechenhistorie: Populationsvalidierung
  sowie Speichern/Laden ohne faelschlich beanspruchte Oberflaechenzellen.
- Echte 60-Hz-Spinnen-Pivot-/Uferwechsel mit weiterhin gueltigem Gameplay.
- Reitmotor deterministisch, diagonale Eingaben normalisiert, Kopffreiheit,
  reale Sprunghoehen, fehlende Stuetzen, echte Slabs und flaches Wasser.
- Besitz/Alter/Leben, kein doppeltes Aufsitzen und kein Ausdauer-Reset durch
  wiederholtes Ab-/Aufsteigen.
- Zwei echte UDP-Clients pruefen Aufsteigen, Besitzverweigerung, Bewegung,
  Sprung, entfernten Spielerzustand, Ausdauer, Absteigen und Save-Gueltigkeit.
- Neue breitere Connectoren fuer drei Seeds, negative Naehte und identische
  alte Revisionen 0..5; gespeicherte Bauten bleiben erhalten.
- Gestaffelte Schneeraten, acht reale Schichten, unveraenderte Prop-IDs und
  keine Entfernung vor Schicht 3; reversibles Schmelzen und Herbst-/Winterfade.
- Ein Weather-Buffer mit synchronem 96-Byte-Layout; expliziter Preview-Gate
  bleibt fuer alle Server-/Replica-Zustaende gesperrt.

## Kompatibilitaet Und Grenzen

- Protokoll 68: Client und Dedicated Server zusammen aktualisieren.
- Keine Dependency-Upgrades, neuen Block-/Tier-/Welt-IDs oder Saveformate.
- Aktueller Reiter und seine Rest-Ausdauer sind Sitzungszustand. Besitzer,
  Fell, feste Kapazitaet/Sprunghoehe und letzte autoritative Tierposition werden
  ueber die vorhandenen Weltdaten gespeichert.
- Pferdehunger/-durst werden autoritativ fortgeschrieben und gespeichert,
  nicht exakt im kompakten Mounted-Snapshot vorhergesagt.
- Alte dauerhaft entfernte Props werden nicht blind wiederhergestellt:
  historische Wetterentfernung und echte Spielerentfernung sind ununterscheidbar.
- Alte Welten werden nicht still auf breitere Hoehlengenerierung umgestellt.
- Keine allgemeine FPS- oder Langzeitstabilitaetszusage aus diesen Funktionstests.
