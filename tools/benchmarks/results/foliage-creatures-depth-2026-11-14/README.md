# Birken, Laub, Augen und Wasserbewegung — 2026-11-14

Die Prüfung umfasst die sechs Änderungswünsche: Birkenform, Laubdarstellung,
Blinzeln/Pupillen, tieferes Schwimmen sowie Pferde- und Spinnenmodelle.

Abgeschlossen: **5.147 Rust-Tests und 35 Assettests erfolgreich**, keine
fehlgeschlagenen Tests. Formatprüfung, striktes Clippy und Workspace-Check sind
grün. Client, Server und Gebäude-Editor wurden erfolgreich gebaut. Die drei
abschließenden nativen Läufe erzeugten zusammen **62 Aufnahmen** mit demselben
Client-Binary (SHA-256
`86310ad51a85148e7720f02d0c789b6d9f5f5c2e7f1c929a59414b5b2bad9778`).

## Nachweise

Die endgültigen Ergebnisse der Formatprüfung, Workspace-Tests, des strikten
Clippy-Laufs, Workspace-Checks und der drei Builds stehen in `checks.json`.
`workspace-tests.log` enthält die einzelnen Regressionen, unter anderem:

- echte Pupillenflächen und Sichtstrahlen zur Kopf-/Lidoberfläche;
- Halbblink mit festem oberen Lidrand bei Drehung und Jungtierskalierung;
- alle vorhandenen Modell-/Vertexbudgets und 16 Pferdefellvarianten;
- geschlossene Laubträger, alle Nachbarmasken, Seiten-UVs, getrennte Topslots;
- Birkenform, alte Revisionen, gepflanzte Birken und erhaltene Mature-Zellen;
- Abtauchen sämtlicher Wasserarten durch die Produktionsdispatch;
- bearbeitete Böden/Decken, ungeladene Tiefe und Teilwasser;
- gedrehte Körper, tatsächlich berührte OBB-Zellen, Zwischenwinkel und Fisch-Suchbudget;
- reproduzierbare Bewegung nach Population-Save/Reload.

Die fünf `python-*.log`-Dateien enthalten 35 erfolgreiche Assetprüfungen.
Zehn bereits vorhandene Rust-Tests bleiben ignoriert. Die nicht vorhandene
externe Fixture `FIVERR/VoxelGrassBlock glb file.glb` ist für genau einen
benannten Original-Importtest ausgefiltert; der ausgelieferte GLB-/PNG-/JSON-Pfad
wird von den übrigen Assettests und nativen Aufnahmen verwendet.

## Native Prüfung

`run-foliage-creatures.sh` verwendet den normalen Block-Journal-/Chunkrenderer
für zehn Laubtypen und das echte Kreaturen-Animationssystem für vier
Blinkphasen. Jede Augenaufnahme muss sechs sichtbare Köpfe enthalten;
Hintergrund-Dithering genügt nicht. Der erste Probelauf deckte eine vorzeitig
aufgenommene leere Phase auf; die Aufnahmeroutine wartet nun auf echten Inhalt.

`run-voxel-creatures.sh` prüft alle bestehenden Modellgalerien sowie drei echte
Spinnengrößen. `run-reference-biomes.sh reference-birch picker-forest
picker-cherry-forest` sucht tatsächlich generierte Bäume und kontrolliert
Proben erneut in den geladenen Sections. Die Birkenansicht setzt keine
Fixture-Blöcke. Der Umfang der geometrischen Formprüfung ist in der
[Birken-Dokumentation](../../../../docs/birch-aquatic-depth.md) beschrieben.

Die finalen Läufe sind in `native-checks.json` und den drei `*-verification.json`
festgehalten:

| Lauf | Temporärer vollständiger Bildsatz | Aufnahmen |
| --- | --- | ---: |
| Laub und Augen | `target/foliage-creatures/run.T4zXBe` | 11 |
| Kreaturen | `target/voxel-creatures/run.rx3Vgh` | 47 |
| Birke, Wald, Kirschwald | `target/reference-biomes/run.BbKQeU` | 4 |

14 unveränderte Rohaufnahmen sind dauerhaft hier unter `captures/` gesichert;
`capture-manifest.json` enthält Herkunft und Prüfsummen. Sichtgeprüfte Beispiele:

- [Eichenlaub](captures/foliage-1.png), [Kirschlaub](captures/foliage-3.png),
  [Laubübersicht](captures/foliage-0.png).
- Augen [offen](captures/eyes-0-open.png), [halb geschlossen](captures/eyes-1-half-blink.png),
  [geschlossen](captures/eyes-2-closed.png), [wieder offen](captures/eyes-3-after-blink.png).
- [Pferdevarianten](captures/horses-0.png), [Pferd seitlich](captures/horses-2.png),
  [Spinne](captures/spiders-3.png).
- [Generierte Birke](captures/reference-birch-overview.png),
  [Birke nah](captures/reference-birch-close.png),
  [Wald](captures/picker-forest-overview.png),
  [Kirschwald](captures/picker-cherry-forest-overview.png).

Die untersuchte Birke besitzt einen sechs Block breiten Fuß, eine zwei Block
breite Mitte und eine zwölf Block breite Krone. Die Inspektion bestätigt
Ecology 7, null Fixture-Edits und 49 geladene Voxelproben je Ansicht.
In allen vier Augenphasen sind alle sechs Köpfe sichtbar. Pupillen erscheinen
vor und nach dem Blinzeln; die geschlossenen Lider verdecken Pupille und Weiß.
Die Sichtprüfung fand keine offenen Laubflächen oder blockierenden Modellfehler.

## Grenzen

Die Aufnahmen belegen die tatsächlich gerenderten Modelle und Texturen,
keine pixelgleiche Nachbildung der Illustrationen. Laub verwendet begrenztes
Blattrelief auf geschlossener Blockgeometrie. Es werden keine neuen Laub-IDs,
Spinnen-Biomfarben, Sattel- oder Rüstungsmechaniken eingeführt. Die vorhandenen
16 Pferdevarianten und drei Spinnengrößen bleiben erhalten.
Das regelmäßige Blatt-/Blütenmuster ist in der Entfernung weiterhin erkennbar;
die Kronensilhouette bleibt voxelgestuft. Einzelne Modellnahaufnahmen beschneiden
Hufe oder Beine am Bildrand; ergänzende Ansichten zeigen diese Bereiche.

Die neue Birkenform gilt für neue Welten mit Ecology 7. Bestehende gespeicherte
Weltrevisionen und ausgewachsene Spielerbäume werden nicht umgebaut. Wasser-
tiere können nur innerhalb tatsächlich geladenen und ausreichend tiefen
Wassers schwimmen. Die Schwimmregressionen sind autoritative Simulationstests;
die trockene Modellgalerie ist kein Nachweis einer Unterwasserszene.

`worlds-before.json` erfasst die ursprünglichen Nutzerwelten.
`world-integrity.json` bestätigt nach allen isolierten Smoke-Sitzungen dieselben
SHA-256-Prüfsummen für sämtliche 23 Dateien und keine hinzugefügten Weltdateien.
