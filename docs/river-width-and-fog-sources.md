# Flussbreiten und natürliche Nebelquellen

Neue Welten verwenden `river_terrain_revision = 7`, `landscape_revision = 8`
und `ecology_revision = 8`.
Die Revisionen gehören zur gespeicherten Weltidentität. Beim Öffnen bestehender
Welten bleiben ihre bisherigen Revisionen einschließlich ihrer Edits
erhalten; fehlende historische Felder bedeuten weiterhin Revision 0. Es findet
keine nachträgliche Änderung gespeicherter Spielerwelten statt.

Revision 7 kombiniert kurze lokale Bäche mit großen Einzugsgebieten. Letztere
verwenden unterschiedlich große Verläufe, lange Zuflüsse und Gabelungen zu Seen.
Die tatsächliche Landstrecke entscheidet über die Flusslänge; ein Verlauf über
offenem Meer zählt nicht als langer Fluss. Ein begrenzter Cache hält 16 große und
256 lokale Einzugsgebiete, ohne die entfernte Welt oder ihre Chunks vorzuladen.

Landschaftsrevision 8 vergrößert die Klimaregionen und ordnet Biome Kalt, Warm oder
Heiß zu. Eine warme Zwischenzone verhindert direkte Wüsten-Hochland-Nachbarschaft.
Reliefbezirke variieren Ebenen, Hügel und Terrassen innerhalb desselben Bioms.
Diese Änderungen benötigen eine neue Welt; gespeicherte Landschaften werden
beim Laden nicht umgebaut.

## Flüsse

`RiverSample.width` bezeichnet eine **Halbbreite**. Sie allein sagt noch nicht,
wie viele Weltblöcke tatsächlich Wasser enthalten: Die alte flache Bachparabel
konnte nach Rundung des Bodens nur ein bis drei nasse Blöcke übrig lassen.
Revision 5 verwendet deshalb ein ausreichend tiefes, breites Flussbett mit
weichen Schultern, berücksichtigt den senkrechten Abstand schräger Flussläufe
und begrenzt die Halbbreite auch nach Einzugsgebiets- und Spirit-Verbreiterungen.
Die zusätzlichen Uferproben berücksichtigen die verbreiterten Bäche und
Nebenläufe, damit Wasser nicht über ungestützte Ufer hinausragt.

Das Ziel von 6–22 m betrifft den vollständigen nassen Querschnitt einzelner
Flussrinnen einschließlich Bächen und Nebenläufen. Die Grenze ist keine globale
Größenbegrenzung für Seen, Oasen, Meeresmündungen oder vereinigte Flussarme.
Quellen, Endkappen und Wasserfallübergänge laufen entsprechend ihrer Geometrie
aus. Landform, Verlauf und Wasserhöhen älterer Flussrevisionen bleiben erhalten.

Der ursprüngliche Regressionstest für Revision 5 misst die physische Breite einer durch reale Voxel laufenden
Querschnittslinie; mehrfach getroffene diagonale Zellen werden nicht als ganze
zusätzliche Meter gezählt. 164 regionale Querschnitte über drei Seeds ergeben
7,5–20,125 m. Jeder gemessene Wasserbereich wird gegen die echten leeren
`SectionGenerator`-Zellen geprüft. Zusätzliche Tests decken flache Bachbetten,
schräge Läufe, gebrochene Wasserhöhen, negative Koordinaten, Chunk-Grenzen und
Generierungsreihenfolge ab.

## Nebelsteine im Geistwald

Ecology 8 ergänzt den festen, abbaubaren `teramic::fog_block` (Material 430).
Das bestehende `MysticMistBlock`-Material 399 wird dadurch nicht verändert.
Die Quellen benutzen denselben begrenzten Natur-Patch-Cache wie Flora und
Felsgruppen; globale Einzelabfragen und gestreamte Sections erhalten exakt
dieselben Zellen.

Pro globalem 32-m-Tile gibt es höchstens einen nach Seed versetzten Kandidaten;
nur ungefähr ein Drittel der Tiles kommt überhaupt infrage. Kandidaten liegen
mindestens 17 m auseinander. Sie benötigen starken Spirit-Anteil, Spirit-Gras,
flachen trockenen Boden und Abstand zu Bäumen und reservierten Strukturen.
Höhleneingänge, Flusskavitäten, Wasser und bereits belegte Natur-Patch-Zellen
werden ausgeschlossen. Die meisten Steine liegen direkt unter der Grasdecke,
etwa ein Siebtel der akzeptierten Kandidaten ist an der Oberfläche sichtbar.
Der Renderer kann dadurch Nebel über vergrabenen Quellen zeigen, während die
Quelle selbst durch Freilegen auffindbar bleibt. Entfernte Quellen werden durch
das bestehende Edit-Journal auch nach erneuter Section-Generierung entfernt.

Die Diagnose-API lautet:

```rust
terrain.spirit_mist_sources([tile_x, tile_z]) // Vec<[i64; 3]>
```

Die Argumente sind globale 32-m-Tilekoordinaten mit euklidischer Division.
Eine geprüfte Region für Seed 42042 mit der ausgelieferten World-Konfiguration
und `BiomeDefinitions::default()` liegt um `[-2872, -4045]` in X/Z. Quellen
liegen beispielsweise bei `[-2968, 27, -4049]`, `[-2932, 28, -4042]` und
`[-2833, 44, -4012]`. Andere gespeicherte Biomdefinitionen können andere
geeignete Standorte ergeben; native Prüfungen sollen die API verwenden.

Prüfprotokolle:
[River-Tests](../tools/benchmarks/results/leaf-fog-ui-rivers-2026-11-15/river-tests.txt)
und [Nebelquellen](../tools/benchmarks/results/leaf-fog-ui-rivers-2026-11-15/mist-source-tests.txt).
