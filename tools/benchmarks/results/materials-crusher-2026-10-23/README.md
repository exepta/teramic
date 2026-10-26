# Phase 108: Crusher-Verkleidung, Werkzeuge Und Farbstoffe

## Umsetzung

- Geschlossene Crusher-Rueckwand, Seitenwaende, Mitteltrennung und Einzugsschacht
  hinter den beiden hohlen Trichtern. Breite Querbaender, Holzrahmen und
  Metallbeschlaege folgen der Referenz. Auswurf und rechter Kurbelanschluss
  bleiben frei; neue feste Geometrie hat passende Kollisionsboxen.
- Neuer Klang aus acht bereits vorhandenen CC0-Holz-/Stein-/Kiesaufnahmen,
  einmal asynchron dekodiert. Unregelmaessige Tonhoehen, Einsatzabstaende und
  Huellkurven, kurze gedaempfte Metallkontakte statt dauerndem Sinuston.
  Ein Stereo-Stream, maximal zwoelf feste Foley-Stimmen, keine Audio-Entity
  oder Allokation pro Kontakt. Entfernung, Panorama und Mute bleiben erhalten.
- Schwert, Spitzhacke und Doppelaxt in Wood, Stone, Iron, Gold, Diamond und Ruby.
  Facettierte Koepfe/Klingen, Fassungen, Nieten, Lederwicklungen und
  materialspezifische Maserung/Bruchflaechen. Zwei bestehende Schaufeln bleiben.
  Insgesamt 20 nutzbare Werkzeuge mit eigenen Item-JSONs und Shaped-Rezepten.
- Ausgeruestetes Gesamtmass: Schwert 1,5 m, Axt/Spitzhacke 1,25 m.
  Inventar und Hand nutzen dieselbe Geometrie. Inventarbilder sind eingepasst,
  Pickup-Darstellung bleibt bewusst klein. Griff, Breitseite und Near-Plane-
  Schutz der First-Person-Darstellung sind angepasst.
- Insgesamt 13 Dyes, einschliesslich aller gewuenschten Farben. Sechs Farben
  sind neu; White/Green/Orange waren bereits Enumfarben. White bekommt einen
  herstellbaren Pigmentpfad. Alle 13 Farben faerben Betten und die fuenf
  Cape-Stufen. Black/White sind zusaetzlich aus Kohle-/Salzstaub crushbar.
  [Alle Rezepte und Werkzeugstufen](../../../../assets/terra/items/DYES_AND_TOOLS.md).
- 69 mitgelieferte Blockdefinitionen haben jetzt `bevel_width = 0`.
  Geschlossene Wuerfel, Slabs, Treppen und Zaune behalten gerade Kanten.
  Texturen, Oberflaechendetails und optionale Mod-Bevels bleiben erhalten.
- F25 Kinetik und F26 Mixer stehen als abhaengige neue Features in der TODO.
  Sie sind mit diesem Auftrag noch nicht als Antriebs-/Mischsystem umgesetzt.

## Kompatibilitaet Und Budgets

Die bisherigen sieben Dye- und neun Tool-Enumwerte bleiben identisch. Alte
Bettposen 0..27 bleiben unveraendert; neue Farben verwenden 28..51. Der
serverseitige Bett-Interaktionspfad akzeptiert die gemeinsame 52er-Grenze,
statt neue Farben nach dem Platzieren beim Schlafen abzuweisen.
Protokoll 60 erfordert gemeinsames Aktualisieren von Client und Server.
PLY7, RGA1, Generator 5, Weltformat 2 und bestehende Welt-/Block-IDs bleiben.
Alte Saves werden nicht nachtraeglich umgeneriert.

Alle Werkzeuge bleiben je ein gemeinsames Mesh/Material-Batch; wiederholtes
Anfragen erzeugt keine zusaetzlichen Materialien oder Entities. Der begrenzte
Ausruestungs-Modellcache skaliert mit den 65 Capevarianten auf maximal 510
Meshes, nicht mit der Anzahl gespawnter Spieler. Die neun gemeinsam genutzten
Crusher-/Kurbel-Mechanikmeshes und die bestehenden Partikelbudgets bleiben.

Nachvollziehbare Geometriezaehlung am Modellgenerator (kein VRAM-/FPS-Benchmark):

| Crusher | Phase 107 | Phase 108 |
| --- | ---: | ---: |
| Vertices | 15.832 | 16.184 |
| Dreiecke | 8.016 | 8.192 |
| Kollisionsboxen | 31 | 37 |
| Materialgruppen | 14 | 14 |
| Rohes Mesh-Payload | 602.816 B | 616.192 B |

Die geschlossene Verkleidung kostet damit rund 2,2 % mehr Geometrie. Das
Payload rechnet 32 B je Vertex und 12 B je Dreieck, nicht Treiber-/Allocator-
Overhead. Die acht dekodierten Mono-Foleys belegen zusammen 135.292 B f32-
Samples. Ein Idle-Fastpath stoppt RNG, Filter, Stimmen und Sinusmoden bei
stummen Ziel-/Ist-Gains; Metallhuellkurven werden vor dem Subnormalbereich
auf null gesetzt. Das ist eine lokale Audio-Kostenkorrektur, keine Behauptung
ueber den allgemeinen Streaming-FPS-Verlauf.

## Pruefungen

- `cargo fmt --all --check`, `cargo check --locked --workspace --all-targets`,
  `cargo clippy --locked --workspace --all-targets -- -D warnings` und
  `cargo build --locked --workspace` bestanden. Finale Protokolle:
  `logs/fmt.log`, `logs/check-final.log`, `logs/clippy-final.log`,
  `logs/build-final.log`.
- `cargo test --locked --workspace --no-fail-fast`: 1896 bestanden, null
  Fehler, drei ignoriert (zwei vorhandene explizite Messlaeufe und der separat
  auszufuehrende WAV-Export). Masshaltigkeit, alle 20 Werkzeug-Solids,
  zusammenhaengende grosse Kopffacetten, Normalen/Indizes, Griff und Framing,
  128-facher Cache-Warmup, alle 13 Dyes und gerade Blockkanten sind abgedeckt.
- Der native Schraegblick hat zunaechst Z-Fighting der gesonderten
  Schneidenringe gezeigt. Die ueberlagerten Extrusionen sind entfernt; jede
  Kopffluegelflaeche besitzt nur noch einen geschlossenen Solid mit integriertem
  Schleifrand. Ein gezielter baryzentrischer Test kontrolliert an beiden
  gespiegelten Aussenwaenden aller zwoelf Aexte/Spitzhacken exakt eine
  Dreiecksueberdeckung, statt der vorherigen zwei. Er besteht im finalen Lauf.
- Rezeptkataloge: 219 Crafting-/18 Crusherrezepte mit eindeutigen IDs.
  Alle 65 Cape-Recolors, 52 eindeutige Bettposen, alte Enum-Tags sowie echter
  RGA1-/PLY7-Save/Reload sind getestet. Ein echter UDP-Test schlaeft mit jeder
  der 13 Bettfarben und verlaesst das Bett wieder korrekt.
- Vorlaeufige neue Tests hatten zwei falsche Annahmen: Bevy-Ressourcen
  registrieren interne Entities, und das vorhandene Dye-Mesh hat 5088 statt
  unter 2000 Vertices. Die Tests vergleichen jetzt die Entity-Ausgangszahl
  und die gemeinsame, begrenzte Dye-Geometrie. Keine Produktionslogik wurde
  zum Erfuellen dieser falschen Annahmen veraendert. Der isolierte Albedo-Test
  hat nun explizit Bevy-PNG als Dev-Feature, statt nur durch Client-Feature-
  Vereinigung zu funktionieren; keine neue Dependency oder Versionsaenderung.

Vollstaendige Ausgaben: `logs/workspace-tests-final.log`. Vorlaeufige,
korrigierte Testanlaeufe bleiben zur Nachvollziehbarkeit separat erhalten.

Die sechs Sekunden lange [Hoerprobe](audio/crusher-mix.wav) wurde durch den
expliziten Exporttest geschrieben und erneut dekodiert: Stereo, 48 kHz,
PCM16. 0..2 s leeres Kurbeln, 2..5 s Mineralmahlen, 5..6 s Mute. Der
Exporttest besteht separat (`logs/audio-preview.log`).

Native Ansichten auf Vulkan/RX 6950 XT:

- [Werkzeuge, sechs Materialspalten](native/materials/material-reference-0.png)
- [Alle 13 Farbstoffe](native/materials/material-reference-1.png)
- [Gerade Blockkanten](native/materials/material-reference-2.png)
- [Schraege Werkzeug-Detailansicht](native/materials/material-reference-3.png)
- [Crusher im Spiel](native/crusher/crusher-results-world.png)
- [Echte manuelle Bedienung](native/crusher/crusher-running.png)

Der kurze integrierte Crusher-Lauf hat alle 18 Rezepte sowie fuenf Aufnahmen,
Pause/Release, Inputbindung, Audio/Mute, Foley-Stimmen, Partikel, TMI und
atomare Ausgabe bestaetigt. Headless-Dedicated-Smoke mit 27 Sections besteht.
Im ersten integrierten Start meldet Lightyear zweimal fruehe, noch nicht
abonnierte `CreatureStates`, im finalen Lauf einmal; die Queue wird explizit
geleert. Beide finalen nativen Fixtures beenden sich erfolgreich. Kein
Shader-/Slab-Allocator-Fehler im Lauf. Die Startwarnung ist kein
Renderer-Fehler und wird nicht als behoben ausgegeben.

## Reproduktion

```bash
TERRA_CAPTURE_DIR=target/phase108/materials \
  cargo run --locked -p terra-client -- --smoke-test-material-reference
TERRA_CAPTURE_DIR=target/phase108/crusher \
  cargo run --locked -p terra-client -- --smoke-test-crusher
TERRA_CRUSHER_AUDIO_PREVIEW=target/phase108/crusher-mix.wav \
  cargo test --locked --workspace \
  crusher_audio::source::tests::write_crusher_listening_preview -- --ignored --nocapture
```

Fuer isolierte Konfiguration/Aufnahmen `TERRA_CONFIG_DIR`,
`TERRA_MENU_CONFIG_DIR` und `TERRA_CAPTURE_DIR` auf Testverzeichnisse setzen.
Der Crusher-Lauf verwendet echte lokale UDP-/Place-Eingaben und prueft
alle 18 Rezepte, Montage, Pause, Ton, Partikel und Bestanderhaltung.

## Grenzen

Die Vorlagen sind gerenderte Bilder, keine lieferbaren Originalmodelle oder
Texturen. Geometrie und Formensprache sind daran ausgerichtet; eine pixelgenaue
1:1-Uebereinstimmung wird nicht behauptet. Kristallwerkzeuge besitzen helle
Facetten, aber keine unangeforderten dynamischen Lichtquellen.
Die Audioaufnahmen sind materialbezogene Foley-Aufnahmen, keine neue Aufnahme
eines realen Crushers. Signaltests und eine Hoerprobe ersetzen keine menschliche
Kopfhoerer-/Lautsprecherabnahme. Es gibt hier keine FPS-Paritaetsbehauptung und
keinen neuen 10-/15-/30-Minuten-Lauf.
