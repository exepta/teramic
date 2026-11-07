# Geraete, Rezeptvorlagen und rahmenlose F1-Diagnose

Phase 185, 2026-11-03. Vorlagen: [Ofen](references/furnace.png),
[Crafting](references/crafting.png).

Ofen, Brecher, Mischer, Kessel, Lagerfeuer, Truhe/Doppeltruhe, verbundene
Werkbank und Logistikfilter verwenden Kupferrahmen, warme Arbeitskarten und
quadratische Slots. Arbeitsbereiche werden bei schmalen Fenstern gestapelt.
Bedienelemente fuer Farbauswahl, Filter und Tunnel behalten ihre bisherigen
Aktionen und Zustandsanzeigen. Ungefilterte Tunnel erhalten keinen Filter.

Blockinformation, Chat, Karten- und Wegpunktdialoge, Minimap-Bedienelemente,
Multiplayer und verbleibende Einstellungen folgen derselben Palette. Die
gemalten Karten-/Hotbar-Grafiken und semantischen Saisonfarben bleiben erhalten.

F1 zeigt zwei rahmenlose Textspalten: Build/FPS/Framezeit, Session-Minimum und
-Maximum, Sections/Meshes/Streaming, XYZ/Block/Chunk/lokale Koordinaten und
Blickrichtung, Welt/Biom/Zeit/Wetter/Seed sowie CPU/RAM/GPU/Display/Netzwerk
und Zielkennung. Die Werte stammen aus vorhandenen Ressourcen. Fehlende
optionale Messwerte zeigen `--`. Minimap und Blockkarte verdecken F1 nicht.
Terrain- und Kreaturendiagnose bleiben verfuegbar.

Crafting-Rezepte zeigen grosse Zutaten-/Ergebnisslots, Itemdaten und echte
Holzalternativen. Ofenrezepte zeigen Material, Brennstoff, Ofenillustration,
Ergebnis, echte Dauer sowie die vorhandenen Schmelz- und Brennstoffalternativen.
Schmale Fenster verwenden scrollbar bleibende Dokumente. Rezept-Popups bleiben
entsprechend den vorherigen Vorgaben ohne JEI und ohne X-Schaltflaeche.

## Grafik

Die neue [Ofenillustration](../../../../src-ui/assets/furnace-recipe-185.png)
wurde mit dem eingebauten Imagegen aus der inspizierten Referenz erzeugt.
Original-PNG und Alpha wurden ohne Raster-Nachbearbeitung in den Workspace
kopiert; die Grafik ist im endlichen Asset-Preload enthalten.
[Modus, vollstaendiger Prompt und Ursprungsdatei](imagegen-prompts.txt).

## Pruefung

3745 Workspace-Tests bestanden ohne Fehler, acht wurden ignoriert. Nach der
zusaetzlichen Koordinatenkorrektur bestehen alle 448 UI-Tests, einschliesslich
der neuen Regression fuer Spieler-Unterobjekte mit lokaler Nullposition.
Build, Clippy, Format- und CSS-Parserpruefungen sind in
[verification.json](verification.json) und den [Protokollen](logs/) dokumentiert.
Native Aufnahmen verwenden echte Bevy-/TiltUI-Fenster und Betriebssystem-Eingaben.
Die ueblichen Ansichten werden in 1680 x 936 und 1025 x 641 Pixeln geprueft;
Rezeptdokumente zusaetzlich bei 768 x 640 mit einem Pixel Rundungstoleranz.
72 Aufnahmen insgesamt: 25 Inventar-/Rezeptansichten, 28 Geraete-/HUD-Ansichten,
15 Menueansichten und vier schmale Rezeptansichten. Alle nativen Laeufe enden
mit Exitcode 0. Die F1-Geometriepruefung verlangt einen transparenten Hintergrund
ohne Border und vergleicht XYZ mit der bestaetigten Spielerposition.
Die Meshzahlen zaehlen die vorhandenen Section-Meshpuffer im ChunkStore.

Die Geraetegalerie verwendet validierte repraesentative ContainerSnapshots und
passende Materialien im temporaeren Client-Replikat. Sie prueft Darstellung,
Viewportgrenzen, quadratische Slots, verbundene Werkbank und fehlenden Filter
am normalen Tunnel. Sie behauptet keinen neuen Ofen-/Transport-Durchsatztest.
Die anschliessende Block-/F1-Aufnahme verwendet das echte Spielziel und native
E-/F1-Tasten. Das autoritative Spielerinventar bleibt unveraendert. Bestehende
Spielstaende werden nicht geoeffnet.

Die separate Inventar-/Rezeptgalerie befuellt zuerst 16 Fixture-Slots ueber
den echten Creative-Befehl der temporaeren Autoritaet. Alle anschliessenden
Vorschauaktionen erhalten diese Inventarbasis. Fuer die F1-Pruefung wird die
im Inventar-Smoke-Modus absichtlich deaktivierte Kameraeingabe wieder ueber
den normalen CaptureRequest-Pfad freigegeben.

Die Menuegalerie erzeugt private Weltmanifeste und entfernt diese anschliessend.
Create/Delete und Add Server werden geoeffnet und abgebrochen; keine Verbindung
zu einem Spielserver wird gestartet. Welt-Ladebilder zeigen repraesentative
Fortschrittswerte, keinen neuen Generierungslauf.

- Rezepte: [Ofen gross](native/furnace-recipe-reference-wide.png),
  [Ofen klein](native/furnace-recipe-reference-compact.png),
  [Holzalternativen](native/chest-oak.png),
  [Crafting schmal gescrollt](native/crafting-recipe-scrolled-narrow.png),
  [Ofen schmal](native/furnace-recipe-reference-narrow.png)
- Geraete: [Ofen](native/surface-furnace-wide.png),
  [Mischer](native/surface-mixer-wide.png),
  [Kessel klein](native/surface-cauldron-compact.png),
  [Werkbank mit Truhe](native/surface-workbench-linked-wide.png),
  [Werkbank klein](native/surface-workbench-linked-compact.png),
  [ungefilterter Tunnel](native/surface-tunnel-wide.png),
  [Verteiler](native/surface-distributor-compact.png)
- Diagnose: [F1 gross](native/surface-debug-wide.png),
  [F1 klein](native/surface-debug-compact.png),
  [Blockinformation](native/surface-target-wide.png)
- Multiplayer: [gross](native/multiplayer-wide.png),
  [klein](native/multiplayer-compact.png),
  [Dialog](native/multiplayer-add-dialog.png)

[Befehle](commands.txt), [Quellpruefsummen](source-sha256.json).
