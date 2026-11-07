# Kingdom-Terrain, Türen und Welterstellung (198)

Die Korrekturen folgen den Bildern mit dem [ausgeschnittenen Feld](references/farm-problem.png)
und der [senkrechten Fundamentkante](references/foundation-problem.png).
Neue Generierungsgeometrie verwendet Strukturrevision **7**. Gespeicherte ältere
Revisionen behalten ihre Bauten; bestehende Spieleränderungen werden nicht
automatisch mit neuen Strukturplänen überschrieben.

## Verhalten

- Felder folgen dem Gelände, anstatt einen rechteckigen Schacht auszuschneiden.
  Aufgeschüttete Außenbereiche gehen mit begrünten Hängen in das Terrain über.
- Turmtüren sitzen zwischen wirklichen Steinpfosten; die Bergfriedöffnung passt
  zur Doppeltür. Die Achse geschlossener Türblätter bleibt an der Wand ausgerichtet.
- NPCs schließen selbst geöffnete Türen nach einer kurzen Verzögerung, sobald
  Spieler und Tiere den Durchgang verlassen haben. Beide Hälften werden zusammen
  geändert; fehlende Chunks werden abgewartet. Manuell geöffnete Türen behalten
  ihre Bedienung.
- Beim Verlassen eines Reichs erscheint **Neutral Territory**. Weitere neutrale
  Chunks starten dieselbe Meldung nicht erneut. Unabhängige Dörfer werden als
  neutral angezeigt.
- **Kingdoms** und **Outposts** sind in der Welterstellung einzeln abschaltbar.
  Die gespeicherten Schalter überstehen Neustarts und Netzwerkübertragung;
  alte Einstellungen übernehmen die bisherige Kopplung an den Dorf-Schalter.
- Die Biombilder werden aus echten Spielaufnahmen mit **1536 × 768 Pixeln**
  exportiert. Die vorherigen Vorlagen hatten nur **122 × 59 Pixel**. Auswahl und
  Vorschau verwenden für diese Fotos weiche Bildfilterung; Symbole behalten ihre
  eigene Darstellung. Auch die Felsenwüste erhält ein eigenes Bild.

Client und Server verwenden wegen der erweiterten Weltoptionen Protokoll **106**.
IDX9/PLY7 und die bestehenden TOML-Weltmanifeste bleiben lesbar.

## Aufnahmen und Quellen

| Struktur | Übersicht | Nähe |
| --- | --- | --- |
| Kingdom | [Burg und Gelände](native/kingdom-temperate/kingdom-temperate-overview.png) | [Hangfeld und Mauern](native/kingdom-temperate/kingdom-temperate-close.png) |
| Winter-Kingdom | [Übersicht](native/kingdom-winter/kingdom-winter-overview.png) | [Höfe und Böschungen](native/kingdom-winter/kingdom-winter-close.png) |
| Outpost | [Begrüntes Fundament](native/outpost/outpost-overview.png) | [Tor und Zugangsweg](native/outpost/outpost-close.png) |

Die Bauaufnahmen verwenden reale Generierung mit Seed **42042** und
Strukturrevision **7**, in den Regionen **[-1,-1]**, **[0,-1]** und **[1,4]**.
Eine private Kamera mit weiter Sicht und ausgeschaltetem Nebel zeigt die Geometrie;
reguläre Spieleinstellungen werden nicht geändert. Die gebauten Zellen und die
begehbaren Wege werden zusätzlich in den automatisierten Tests geprüft.

Die acht [originalen Biomaufnahmen](biome-sources/) haben **2562 × 1440 Pixel**.
Der [Herkunftsnachweis](../../../../src-ui/assets/biome-previews.json) dokumentiert
Quellen, Ausschnitte und SHA256-Prüfsummen. Die Fotos verwenden die aktuellen
Biomdefinitionen einer frischen Welt mit Seed 42042. Für die Ozeanaufnahme blendet
der normale Entfernungsnebel den Rand der geladenen Welt aus; Waldaufnahmen prüfen
wirklich erzeugte Baumgruppen. Die Aufnahmen verändern keine Terrainblöcke.

Die reproduzierbare Exportpipeline ist
[build_biome_previews.py](../../../assets/build_biome_previews.py). Sie erzeugt
1536 × 768 Pixel große 8-Bit-PNGs aus größeren nativen Aufnahmen und verweigert
kleinere Quellen.

| Menüprüfung | Struktur-Schalter | Biomauswahl |
| --- | --- | --- |
| Full HD | [Optionen](native/ui-fhd/create-world-options-scrolled.png) | [Vorschauen](native/ui-fhd/create-world-biome-chooser.png) |
| 4K | [Optionen](native/ui-4k/create-world-options-scrolled.png) | [Vorschauen](native/ui-4k/create-world-biome-chooser.png) |

Beide nativen Menüprüfungen enden mit Status 0. Sie bedienen die tatsächlichen
Button-/Switch-Handler, prüfen voneinander unabhängige Kingdom-/Outpost-Zustände
und laden anschließend die gespeicherte Weltkonfiguration
([Full HD](created-worlds/fhd-world.toml), [4K](created-worlds/4k-world.toml)). Die Vorschauprüfung
kontrolliert die tatsächlich geladenen Bilder, ihre Auflösung, den eigenen
Bildfilter sowie Karten- und Footerbegrenzungen. Pro Auflösung liegen acht
Menüaufnahmen vor. Die Eingaben bleiben im Test innerhalb von ECS; der
Drück-/Loslasspunkt wird gegen fremde Mausbewegungen stabil gehalten.

Die unveränderten Rohprotokolle enthalten nicht fatale Wayland-Meldungen zur
Cursorposition und beim Beenden eine übersprungene zusätzliche Screenshot-Anfrage.
Die hier verlinkten Menüaufnahmen wurden gespeichert; beide Prüfungen bestätigen
die gespeicherten Optionen erfolgreich.

## Prüfung

**3.918 Workspace-Library-/Bin-Tests bestanden, neun ignoriert.** Die neuen
Regressionen prüfen tatsächliche Feldhöhen, Fundamentübergänge, Türblattachsen,
Türpfosten und Stürze, begehbare Bergfriedgeschosse in Revision 6 und 7 sowie
gepaarte Türänderungen mit Spielern, Tieren und entladenen Türhälften.
Weltoptionen werden über TOML, Dedicated-Neustart und echte Welcome-Pakete geprüft;
die neutrale Eintrittsanzeige einschließlich Fade und erneuter Gebietseintritte
wird als laufendes Bevy-System getestet.
Build, Clippy aller Targets mit `-D warnings`, Format, Diff und alle 60
UI-Stylesheets sind erfolgreich geprüft. Insgesamt liegen 30 native Aufnahmen
vor: sechs Strukturansichten, acht Biomquellen und sechzehn Menüansichten.

```bash
cargo test --workspace --lib --bins --locked --no-fail-fast -j2 -- --test-threads=8
cargo clippy --workspace --all-targets --locked -j2 -- -D warnings
cargo build --workspace --locked -j2
cargo fmt --all -- --check
git diff --check
```

Prüfsummen, Bildgrößen und Rohprotokolle stehen in
[verification.json](verification.json) und im [Protokollordner](logs/).

## Speicherentlastung

Sieben unbenutzte Teramic-Testverzeichnisse wurden aus dem RAM-Dateisystem auf
die Datenplatte verlagert. Die Originalpfade bleiben als Verknüpfungen erhalten;
Dateigrößen und Änderungszeiten wurden vor dem Entfernen der RAM-Kopien verglichen.
Laufende Programme und Dateien anderer Projekte wurden erhalten.

Die Swap-Belegung sank durch die verlagerten Daten zunächst von ungefähr
**24,6 GiB auf 13,4 GiB**. Anschließend wurde der Swap nach erfolgreicher
Systemanmeldung vollständig zurückgesetzt: Das [Resetprotokoll](logs/swap-reset.log)
zeigt **0 B** Belegung und die wiederhergestellte ursprüngliche Priorität **100**.
Vorher wurde ausreichend verfügbarer RAM einschließlich 2 GiB Reserve geprüft;
ein Exit-Handler sichert das Wiedereinschalten. Laufende Programme können danach
wieder Swap belegen. Die Messung und die erhaltenen Pfade stehen in
[memory-relocation.json](memory-relocation.json).
