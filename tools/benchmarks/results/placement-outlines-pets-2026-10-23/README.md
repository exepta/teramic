# Platzierungskonturen Und Pausierte Pet-Befehle

Phase 114 korrigiert das Verhalten nach der Zaehmung und die Platzierungsvorschau.
Protokoll 65, Block-IDs, PLY7/RGA3 und Generatorversion 5 bleiben unveraendert.

## Pets

- Futterverbrauch, Vertrauen, Besitzer und Speichern/Neuladen bleiben erhalten.
- Neue gezaehmte Tiere starten wartend. Leere Hand gibt keinen Bewegungsbefehl.
- Auch ein frueher gespeichertes `Follow` wird nicht ausgefuehrt. Der gespeicherte
  Enum-Wert bleibt lesbar; replizierte Ansichten verwenden den inaktiven Wartemodus.
- Besitzer werden nicht angegriffen. Untergrund, Wasser, Spinnenbefestigung und
  laufende Sprungphysik bleiben aktiv; es gibt keinen Teleport zum Besitzer.
- Das Ziel-HUD zeigt Vertrauen und Besitzer, keinen nicht bedienbaren Begleitermodus.
- Die spaetere Pet-Befehlsoberflaeche steht als F33 in TODO.md.

## Platzierung

Nur Bauten mit vorhandener Ausrichtung erhalten eine Vorschau: Moebel,
Treppen/Slabs/Zaunelemente, Rohre, Tueren/Fenster, Betten, Maschinen und
Wand-/Bodenfackeln. Stein, Dirt, Gras und andere normale Vollbloecke behalten
ihren bisherigen autoritativen Platzierungsplan, aber keine Render-Vorschau.
F1+B umgeht diese Einschraenkung nicht.

Die Form entsteht weiterhin aus demselben Modell-/Platzierungsresolver wie die
echte Platzierung, einschliesslich Misch-Slabs und Mehrblockmoebeln. Ein
`LineList`-Mesh enthaelt ausschliesslich Kanten, keine gefuellten Dreiecke.
Gemeinsame Kanten werden ueber Positionsschluessel mit 0,00001-Block-Praezision
erkannt; koplanare Triangulierungsdiagonalen werden verworfen. Echte Modellkanten
bleiben. Das unindizierte Linienmesh speichert zwei Endpunkte je Kante.

Eine einzige wiederverwendete Mesh- und Materialinstanz reicht fuer die Kontur.
Orange sRGB `(1.0, 0.48, 0.08)` pulsiert mit 1,6 Sekunden Periodendauer und
Deckkraft 0,4 bis 0,95. Pro Pulsframe aendert sich nur die Materialfarbe, nicht
die Geometrie. Keine Entity je Kante, kein gefuellter Ersatzwuerfel bei noch
fehlenden Modelldaten und keine zusaetzlichen Schatten.

## Gezielte Regressionen

- Normale Vollbloecke bleiben auch mit altem ausrichtbarem Plan und Debug unsichtbar.
- Slab-/Treppen-/Moebelformen und negative Koordinaten verwenden echte Modellkanten.
- Koplanare Dreiecke erzeugen keinen diagonalen Draht; Mesh-Indizes bleiben gueltig.
- Pulsierung behaelt Farbe, Entity und Mesh-Handle; 200 Testframes vermehren keine Assets.
- Alte gespeicherte Folgewerte, echte Zwei-Client-Befehle, Ziel-HUD und erneutes
  Laden koennen das deaktivierte Folgen nicht reaktivieren.

## Native Pruefungen

Kurze reproduzierbare Client-Fixtures auf RX 6950 XT (RADV/Mesa 26.2.4,
Vulkan), mit isolierter Konfiguration und ohne private Weltstaende:

| Szene | Ergebnis |
|---|---|
| Normaler Vollblock | Keine Vorschau; auch F1+B und kleineres Fenster/FOV umgehen die Einschraenkung nicht. Echte Abbau-/Setzaktion erfolgreich. |
| Slab | 12 orange Modellkanten; Pulsdeckkraft 0,661 bis 0,860 mit unveraendertem Mesh-Handle. |
| Treppe | 34 orange Modellkanten; Pulsdeckkraft 0,408 bis 0,538 mit unveraendertem Mesh-Handle. |
| Werkbank | 753 Modellkanten ueber die reale Zweiblockform; Pulsdeckkraft 0,629 bis 0,840 mit unveraendertem Mesh-Handle. |
| Pets | Vier echte UDP-Fuetterungen, Vertrauen 0/25/50/75/100 und Weizen 8 auf 4. Neue und zuvor mit Follow gespeicherte Kuh bleiben trotz Besitzerbewegung stehen; beide Positionsabweichungen 0,0 m. Leere-Hand-Aktionen bleiben wirkungslos. |

Vollblock, Slab, Treppe, Werkbank und Pet-HUD wurden anhand der aufgenommenen
Framebufferbilder tatsaechlich angesehen. In den projizierten Modellbereichen
liegen 1283 orange Pixel beim Slab und 2806 bei der Treppe. Keine gefuellten
Vorschauflaechen; das Pet-HUD zeigt Vertrauen/Besitzer ohne Befehlszeile.

- [Vollblock ohne Vorschau](native/plain/building-normal-block-no-outline.png)
- [Slab-Kontur](native/slab/furniture-82-placement.png)
- [Treppenkontur](native/stairs/furniture-74-placement.png)
- [Werkbankkontur](native/workbench/furniture-28-placement.png)
- [Pet nach Besitzerbewegung](native/pets/taming-owner-moved-no-follow.png)

Die Fixtures sind kurz; es wurden keine 10-/15-/30-Minuten-Tests angesetzt.
Die Ressourcenkonstanz der Pulsierung wurde ueber 200 Regressionstestframes
geprueft, nicht als allgemeiner In-Game-Performancebenchmark behauptet.

## Abschliessende Validierung

Alle Aufrufe mit vorhandener Cargo.lock und einem Cargo-Buildjob:

| Pruefung | Ergebnis |
|---|---|
| `cargo fmt --all --check` | Erfolgreich |
| `cargo check --locked --workspace` | Erfolgreich |
| `cargo test --locked --workspace --no-fail-fast` | 2061 Tests erfolgreich, keine Fehler |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Erfolgreich |
| `cargo build --locked --workspace` | Client und Dedicated Server erfolgreich |
| `terra-server --smoke-test --no-save` | Headless erfolgreich |
| Native Vollblock-/Slab-/Treppen-/Werkbank-/Pet-Fixtures | Jeweils Exit 0 |
| Finale Build-/Test-/Native-Logs | Keine Warnungen, Fehler oder Panics |

Logs liegen in [logs](logs/), Bilder in [native](native/). Die Pet-Fixture
verwendete den frischen isolierten Save `target/phase114/taming4`; alte Welten
wurden nicht veraendert. Die vorhandene Zwei-UDP-Client-Regression deckt auch
Besitzerwechselversuche und alte gespeicherte Follow-Werte ab.

Beim Testen wurden zusaetzlich reine Fixture-Probleme korrigiert: ungueltiger
Ruhetimer, eine unzulaessige Gleichzeitigkeitserwartung fuer getrennte Inventar-/
Tiernachrichten und ein Shutdown waehrend einer Hintergrund-Screenshotaufnahme.
Die Fixture wartet nun begrenzt auf beide echten Zustandsbestaetigungen sowie
zwei aufeinanderfolgende Frames ohne offene Screenshotaufnahme; weder
Futterverbrauch noch Bewegungstoleranzen wurden gelockert.

## Bewusste Grenzen

Keine neue Pet-UI oder Bewegungsbefehle in dieser Phase. Die bestehende
Folgemodus-Serialisierung bleibt nur fuer Kompatibilitaet lesbar. Die Kontur
zeigt Modellkanten, keine gefuellte Flaeche und keine neue Kollisionsgeometrie.
Nicht jede moegliche Ausrichtung wurde erneut nativ fotografiert; die
vorhandenen Geometrie-/Resolver-Regressionen bleiben Teil des Workspace-Tests.
