# Blood-Forest-Darstellung

Wasser verwendet einen eigenen roten Streufarbwert und einen unabhängig vom
Spirit-Glühen interpolierten Blood-Anteil. Vier gemeinsame Weltkoordinaten pro
Section halten die Übergänge auch an negativen Chunk-Grenzen stetig. Der zusätzliche
Float32-Vertexwert wird vom Oberflächenwasser, Wasserblöcken, Rohrstrahlen und
Becken weitergegeben; Vorschauen ohne Biome-Metadaten erhalten den Wert null.

Bei vollständig rotem Wasser laufen die Wellen mit 24 % der normalen
Geschwindigkeit und 45 % Auslenkung. Höhere Absorption, weniger heller Schaum,
unterdrückte Bodenkaustiken und breitere Glanzlichter lassen es dichter wirken.
Das sind optische Eigenschaften der bestehenden Wasseroberfläche, keine neue
viskose Flüssigkeitssimulation. Die Atmosphäre mischt dezent Rot in Fernnebel und
Umgebungsfarbe; Sonnen-/Lampenenergie bleibt gleich. Unterirdisch verschwindet
dieser atmosphärische Farbanteil wie bei den anderen Waldprofilen.

Blattblock 406 erzeugt rein visuelle Tropfen. Der Renderer hält genau 192
Poolplätze mit derselben Kugelgeometrie und demselben opaken Material bereit.
Bevy kann sie regulär instanzieren; sie werfen keine Schatten und benötigen keine
Sortierung transparenter Flächen. Alle 0,5 Sekunden werden maximal 32 nahe,
bereits geladene Sections geprüft. Ein Positionshash wählt Laubunterseiten,
bevor Terrainabfragen erfolgen. Ab 28 Metern wird die Dichte reduziert, bei
42 Metern endet die Darstellung. Nur geladene Daten und bestätigte Journalwerte
werden gelesen, keine Weltgenerierung auf dem Renderthread.

Die Animation verbindet Wachstum, beschleunigtes Fallen und einen kurzen,
auslaufenden Aufprall ohne Positionssprünge. Aufprallhöhe stammt aus geladenen
Block-/Wasserdaten. Entfernte Blätter werden sofort ausgeblendet; nach Entladen
entstehen keine Tropfen weiter. Es gibt keine dauerhaften Flecken oder Änderungen
an Blöcken, Schaden oder Inventaren. `BloodDripStatus` liefert Pool-, Kandidaten-,
Animations- und Suchzähler für native Prüfungen.

Der native Helfer `apps/client/src/biome_smoke/blood.rs` sucht natürliche rote
Wasserflächen und echte Blattunterseiten. `TERRA_BLOOD_VIEW='[x,z]'` begrenzt die
Suche auf einen bekannten Bereich. Er setzt keine Fixtureblöcke. Die Aufnahmen
sollen Wasser aus der Übersicht sowie die Tropfen unter einer Baumkrone zeigen.

Der Blood-Harness behält die geladene Weltkonfiguration bei und ergänzt nur eine
fehlende Biome-Definition sowie Blood-Revision 1. `blood-forest-droplets.json`
enthält je Ansicht mindestens zwei Sekunden echte Beobachtung: geladene
Kandidaten, aktive Frames, veränderte sichtbare Entity-Transforms, Pool-/Suchgrenzen
und tatsächlich gerenderte Wasserdreiecke. Die Nahansicht wartet bei einer Pause
zwischen Tropfen bis zu zwölf Sekunden auf Bewegung. Der getrennte Biome-Picker
erstellt absichtlich nur eine Übersicht und benutzt seinen eigenen Erfolgsmarker.

Royal-Strukturaufnahmen behalten ebenfalls die Finder-Konfiguration und Revision
10. Nur für diese explizite Smoke-Szene ist Fernnebel ausgeschaltet, weil die
entfernte Übersichtskamera sonst vollständig jenseits des Nebelendes läge. Die
Produktdarstellung und gewöhnliche Sichtweite werden dadurch nicht geändert.
