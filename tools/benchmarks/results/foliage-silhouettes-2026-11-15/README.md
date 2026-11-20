# Freigestelltes Laub und höherer Rabenflug — 2026-11-15

Abgeschlossen: **2.661 Rust-Tests, 16 Assettests und 15 native Aufnahmen erfolgreich**.
Striktes Clippy, Workspace-Check und Builds von Client, Server und Gebäude-Editor
sind grün. Alle 20 bestehenden Nutzerweltdateien haben unveränderte Prüfsummen.

Diese Prüfung behandelt die neue Laubgestaltung mit echten Lücken, bessere
Blatttexturen und den eigenen Höhenflug der Raben. Die 20 gelieferten PNGs behalten
ihre zehn bestehenden Block-IDs und Materialpfade.

## Prüfungen

`checks-final.json` enthält die Befehle, Laufzeiten und Exitcodes. Die Bibliotheken
terra-render, terra-world und terra-voxel haben zusammen **2.661 erfolgreiche Tests**;
sieben bereits vorhandene Tests bleiben ignoriert. `tests-final.log` enthält die
vollständigen Ergebnisse. Nach der abschließenden UV-Anpassung wird der Renderer
separat noch einmal geprüft (`tests-render-silhouettes.log`).

Die sechs Rabenregressionen prüfen tatsächliches Aufsteigen aus Bodennähe,
anhaltenden Reiseflug über Voxel-Kronen, horizontale Bewegung, Hindernisse,
ungeladenen Himmel, Wasserflächen, Landung/Abflug und Save/Reload. Der neue
Wegpunkttest prüft mehrtaktiges Drehen und vergleichbare Kurse bei drei
Simulationsschrittgrößen. Ein Cache-Miss-Test zählt die Auflösung natürlicher
Wasserhöhen und begrenzt sie auf einmal je Säule. Diese Prüfungen verwenden die
autoritative Simulation; die Laubgalerie belegt keinen Rabenflug.

Die Laubprüfungen kontrollieren alle exponierten Seitenmasken, gültige Dreiecke,
UV-Ausrichtung, vollständige Texturkonturen in jeder Variante und beide LODs.
Es gibt keine opake innere Füllfläche. Material-/Vorschauregressionen prüfen
Alpha-Maskierung und beide Blattseiten. Schattenregressionen verhindern einen
falschen Würfelschatten auch bei geteilten Materialslots. Alle gelieferten
RGBA-Bilder werden durch den echten Mipmapper geprüft; ihre Deckung bleibt
bis zur 16px-Stufe innerhalb der gemessenen Toleranz erhalten.

Die Python-Protokolle prüfen die tatsächlichen PNGs, Paletten, Detailauflösung,
vollständige Konturen und regenerierbare Pfade. `art-audit.json` enthält
Dateihashes, Deckung und Randabdeckung der endgültigen Bilder.

## Native Darstellung

Die Galerie benutzt normale Blockänderungen und den echten Chunkrenderer;
Wald-/Kirschwaldansichten benutzen die natürliche Weltgenerierung. Jede Sitzung
verwendet isolierte Konfiguration, Profile und Weltdateien. Die endgültigen
Aufnahmen sind nach dem letzten Build geprüft und unter `captures/` gesichert.
`capture-manifest.json` enthält die Herkunft und SHA-256-Prüfsummen.

- [Eichenlaub nah](captures/foliage-1.png), [Kirschlaub nah](captures/foliage-3.png),
  [Blutlaub nah](captures/foliage-6.png), [zehn Laubtypen](captures/foliage-0.png).
- [Natürlicher Wald](captures/picker-forest-overview.png),
  [Kirschwald](captures/picker-cherry-forest-overview.png),
  [generierte Birke](captures/reference-birch-overview.png).
- [Augen offen](captures/eyes-0-open.png), [halb geschlossen](captures/eyes-1-half-blink.png),
  [geschlossen](captures/eyes-2-closed.png), [wieder offen](captures/eyes-3-after-blink.png).

Die vollständigen Läufe liegen in `target/foliage-creatures/run.uyUyBx`
(elf Aufnahmen) und `target/reference-biomes/run.GGLb7G` (vier Aufnahmen).
`native-checks.json` hält dieselbe Client-Prüfsumme sowie alle zwanzig Texturhashes
über beide Läufe fest. `final-png-mips.log` bestätigt zusätzlich die
Mip-Abdeckung der exakt endgültigen PNGs. `world-integrity.json` bestätigt die
unveränderten Nutzerwelten.

Ein früher visueller Probelauf machte sichtbar, dass UV-Versätze und periodisch
angeschnittene Texturblätter gerade Schnittkanten erzeugten. Die endgültige
Fassung behält vollständige Blätter innerhalb eines transparenten Bildrands und
verwendet ganze Spiegelungen beziehungsweise Oberseitenrotationen. Lange freiliegende
Zweiglinien wurden durch kurze Blattstiele direkt an der tatsächlichen Blattbasis
ersetzt. Die endgültigen PNGs besitzen 66,61–69,75 % deckende Pixel, also etwa
30–33 % Zwischenräume, und mindestens zwei vollkommen transparente Randpixel.

## Grenzen

Die grobe Kronenform bleibt voxelartig; transparente Blattgruppen sind keine
vollständig frei modellierten Einzelblätter. Die Texturen sind weiter native
512px-Bilder, deren Gestaltung überarbeitet wurde. Eine subjektive Verbesserung
um einen festen Prozentsatz ist nicht messbar.

Die neue Nahgeometrie benötigt maximal 48 statt 228 Dreiecke je isolierter
Laubzelle, Middle 12 statt 132. Das ist keine FPS-Messung. Nadelkronen behalten ihre
vorhandene Geometrie und werfen nun ebenfalls ihre vollständigen Schatten.
Raben erreichen ihr Höhenband nur in hinreichend hohem geladenem Luftraum.
Bestehende Raben übernehmen die Steuerung ohne Save-Migration.

[Laubvertrag](../../../../docs/foliage-reference-surfaces.md),
[Texturerstellung](../../../assets/biome_surfaces/FOLIAGE.md),
[Rabensteuerung](../../../../docs/raven-flight.md).
