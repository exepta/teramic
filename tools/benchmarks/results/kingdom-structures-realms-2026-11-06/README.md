# Siedlungen, Reichsfarben und neue Kingdom-Strukturen (197)

Die Diplomatiekarte zeigt explizite Chunk-Claims aller Spielerreiche sowie die
Gebiete bereits entdeckter NPC-Dörfer, Kingdoms und Außenposten in ihren
Reichsfarben. Siedlungsdaten bleiben neben den Kartendaten gespeichert und
überstehen das Entladen der Chunks. Die Anzeige plant keine unbekannten Regionen.

Die zentrale Besitzauflösung gilt für Karte, Fassadenbanner, Dachflaggen,
Soldaten-Wappenröcke und Eintrittsanzeige: Ein direkter Claim gilt am jeweiligen
Chunk; sonst bestimmt der Besitzer des Siedlungszentrums die Zugehörigkeit;
ansonsten gilt das ursprüngliche Bot-Kingdom. Farb-/Wappenänderungen aktualisieren
die Anzeige. Freigabe oder Auflösung stellt die ursprüngliche Zugehörigkeit wieder her.

Rechtsklick entfernt einzelne Chunks aus der noch unbestätigten Auswahl; mit
gehaltener rechter Maustaste lässt sie sich ausradieren. Bestehender Besitz wird
dabei nicht freigegeben. **Apply** bestätigt die verbleibende Auswahl, **Cancel**
verwirft sie. Die hellen Chunk-Umrandungen sind entfernt; farbige Flächen bleiben.
Das getrennte, freiwillig aktivierte Kartenraster bleibt verfügbar.

## Neue Strukturen

Die Vorlagen sind unter [Winter-Kingdom](../../../../docs/references/kingdom-winter.png),
[temperiertem Kingdom](../../../../docs/references/kingdom-temperate.png) und
[Outpost](../../../../docs/references/kingdom-outpost.png) abgelegt.

- **Temperiertes Kingdom:** Runde Außenmauer mit Wachtürmen und Tor, innere
  Burganlage mit vierstöckigem Bergfried und Ecktürmen, Kirche, zwanzig
  Wohnhäuser, Kasernen, Schmiede, Markt, Felder und Windmühle. Wohnhäuser besitzen
  rote oder braune Dächer; Türme und zentrale Gebäude blaue Dächer.
- **Winter-Kingdom:** Gestaffelte Höfe mit großer Treppenachse, höherer Burg,
  schneebedeckten roten Dächern und eigener Höhenverteilung. Türen, Treppen und
  Unterkünfte verwenden dieselben wirklichen Block-/Objektsysteme.
- **Outpost:** Holzpalisade, steinerner Wachturm und Tor, Mannschaftshaus,
  Zelte, Stall, Feuerstelle und kleines Feld. Grasflächen und Wege verbinden die
  Gebäude. Große Wappenbanner und Dachflaggen zeigen das zugehörige Reich.

Die Pläne enthalten echte Fenster, Türen, Betten, befüllte Kisten und
Arbeitsstationen. Der Gründungsmarker liegt im Bergfried beziehungsweise im
Wachturm; sein Chunk ist das Zentrum für die Übernahme der Siedlungsfarben.
Große Banner sitzen an geprüften freien Außenflächen und vorhandenen Dachmasten.
Bestehende Dörfer erhalten abgeleitete Banner ohne Änderungen ihrer Terrainblöcke.

## Bevölkerung

| Struktur | Zivilisten bei Erzeugung | Soldaten bei Erzeugung |
| --- | ---: | ---: |
| Kingdom, beide Varianten | 50–80 | 26–64 |
| Outpost | 0 | 4–10 |

Jeder Bewohner hat ein wirkliches Bett und einen eindeutigen persistenten Platz.
Soldaten besitzen Schwert-, Speer- oder Bogenrollen mit unterscheidbaren Modellen,
Wachschichten, Patrouillen und Wappenröcken. Tote Bewohner bleiben gespeichert;
Streaming oder erneutes Laden würfelt sie nicht neu. Die Generierungsbereiche
werden nicht als laufende Populationsgrenzen verwendet. Die Bewohnerkennung
unterstützt weitere Plätze, ohne die bisherigen Kennungen zu verändern.
Dies ist die Erzeugungsbasis; ein zusätzliches Rekrutierungs-/Wachstumsmenü ist
nicht Teil dieser Änderung.

## Welt- und Speicherverträglichkeit

Neue Welten verwenden **Strukturrevision 6**. Gespeicherte ältere Revisionen
behalten ihre Generierung und bisherigen Größenlimits. Die neuen Baupläne
ersetzen somit keine bestehenden Bauten in alten Welten. Dorf-Wappen und
Diplomatie-Metadaten funktionieren auch für bereits vorhandene Dörfer.
Speicherformate bleiben IDX9/PLY7; die optionalen Kartengebiete nutzen eine
getrennte Datei `settlements.dom`. Client und Server verwenden Protokoll **105**.

Zum Suchen in einer neuen Welt:

```text
/locate struct teramic::kingdom
/locate struct teramic::outpost
```

## Reproduzierbare Ansichten

Native Szenen verwenden Seed **42042**, Terrainrevision **1**, Strukturrevision
**6**, unveränderte echte Terrain-/Objektgenerierung und folgende Regionen:

| Szene | Region | Zivilisten | Soldaten |
| --- | --- | ---: | ---: |
| kingdom-temperate | [-1, -1] | 53 | 28 |
| kingdom-winter | [0, -1] | 51 | 38 |
| outpost | [1, 4] | 0 | 9 |

Beispiel mit isolierter Testkonfiguration und einem frei gewählten Ausgabeordner:

```bash
TERRA_BIOME_SCENE=kingdom-temperate \
TERRA_STRUCTURE_REGION='[-1,-1]' \
TERRA_CAPTURE_DIR=/absolute/capture/path \
target/debug/terra-client --smoke-test-biomes
```

Die Bauaufnahmen verwenden in ihrer privaten Konfiguration ausgeschalteten Nebel
und eine weiter reichende Übersichtskamera, damit die gesamte reale Burg sichtbar
ist. Die regulären Spieleinstellungen werden dadurch nicht verändert. Das sind
Geometrie-/Darstellungsprüfungen, keine Leistungsbenchmarks. Die Kartengalerie nutzt
eine vorübergehende Clientprojektion; Serverautorität wird getrennt mit echtem UDP
geprüft. Es werden keine normalen Spielstände oder OS-Eingaben verwendet.

## Native Aufnahmen

| Szene | Übersicht | Nähe |
| --- | --- | --- |
| Temperiertes Kingdom | [Übersicht](native/kingdom-temperate/kingdom-temperate-overview.png) | [Burg und Dorfwappen](native/kingdom-temperate/kingdom-temperate-close.png) |
| Winter-Kingdom | [Übersicht](native/kingdom-winter/kingdom-winter-overview.png) | [Terrassen und Burg](native/kingdom-winter/kingdom-winter-close.png) |
| Outpost | [Übersicht](native/outpost/outpost-overview.png) | [Tor und Banner](native/outpost/outpost-close.png) |

Die fünf Kartenaufnahmen zeigen [fremde Claims und das NPC-Gebiet](native/claims/diplomacy-claims.png),
[aktualisierte Reichsfarben](native/claims/diplomacy-live-color.png),
das [kompakte Layout](native/claims/diplomacy-compact.png), die
[Flächenauswahl](native/claims/diplomacy-rectangle-preview.png) und das
[Entfernen eines Chunks per Rechtsklick](native/claims/diplomacy-rightclick-unselect.png).

Das rohe Kartenprotokoll enthält beim Fensterwechsel eine Wayland-Meldung zur
Cursorposition und nach dem erfolgreichen Abschluss eine geschlossene
Screenshot-Warteschlange. Alle fünf Aufnahmen wurden zuvor gespeichert; die
Galerie bestätigte ihre Prüfungen und beendete sich mit Status 0.

## Prüfung

**3.697 Workspace-Library-Tests bestanden, acht ignoriert.** Die Prüfungen decken
die beiden Maximalbesetzungen mit jeweils 80 Zivilisten und 64 Soldaten, wirkliche
Bettplätze, vier begehbare Bergfriedgeschosse, Terrassenzugänge und Torwege auf
realem Höhlengelände ab. Militärrollen, erweiterte Bewohnerkennungen und
Todeszustände werden über echte UDP-Verbindungen und Save/Reload geprüft.
Kartenprüfungen umfassen fremde Claims, NPC-Gebiete, Live-Farbänderungen,
Siedlungsübernahme und Rechtsklick ohne Änderung bestehender Besitzdaten.
Workspace-Build und Clippy aller Targets mit `-D warnings` sind erfolgreich;
Format, Diff und alle 60 UI-Stylesheets sind geprüft. Die drei nativen Bau-Szenen
und die Kartengalerie liefern zusammen elf Aufnahmen.

```bash
cargo test --workspace --lib --locked -j2 -- --test-threads=8
cargo clippy --workspace --all-targets --locked -j2 -- -D warnings
cargo build --workspace --locked -j2
cargo fmt --all -- --check
git diff --check
```

Rohprotokolle, Prüfsummen der Quellen und Bildabmessungen stehen in
[verification.json](verification.json) und im [Protokollordner](logs/).
