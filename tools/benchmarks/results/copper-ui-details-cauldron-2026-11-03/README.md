# Slotdetails, Kessel, Ladebildschirme und Kupfer-Popups

Phase 184, 2026-11-03. Aktuelle Vorlagen: `references/better.png` und
`references/cauldron-reference.png`.

- Crafting-Eingaben und Ergebnis verwenden dieselben quadratischen Masse wie
  die Inventarzellen. Die Groesse folgt dem Layout auch nach Fensterwechseln.
- JEI-Zellen haben 75 Prozent der Inventarzellgroesse. Seitenkapazitaet und
  sichtbarer Item-Offset folgen den verfuegbaren Zeilen und Spalten.
- Suchtext steht links mit vertikalem Innenabstand. Kategorien sind rund
  20 Prozent groesser; die Tonne steht unten in der Kategorienleiste links
  neben JEI. Das Spielerinventar besitzt weiterhin 18 + 9 Plaetze.
- Holz-Wildcards besitzen vier sichtbare Alternativkarten mit eigenem Grid,
  Namen und Mengen fuer Oak, Spruce, Birch und Cherry. Karten und Seitenpfeile
  wechseln dieselbe Vorschau. IDs sind pro Variante eindeutig. Es werden keine
  zusaetzlichen Registry-Rezepte angelegt und keine Items/Reservierungen bewegt.
- Die Kesselansicht verwendet eine freigestellte Illustration nach der Vorlage,
  groessere Zutaten- und Trankvorschauen, das echte Rezept und seine echte Dauer.
  Der Fortschritt bleibt autoritativ. Es werden keine Mana-Rezepte oder
  zusaetzlichen Rezeptalternativen fuer die Dekoration erfunden.
- Start-Ladescreen mit Logo, warmen Farben, goldenem Balken, mittiger Anzahl
  und Assetpfad darunter. Er bleibt ohne Containerrahmen und ohne Quit-Button.
  Der existierende Fade ins Hauptmenue bleibt erhalten.
- Welt-Ladeansicht und Create-/Delete-Popups verwenden Kupferrahmen und passende
  Controls. Weltfortschritt steht numerisch im Balken, Details darunter.
  Menueaktionen haben mehr Innenabstand zu den aeusseren Rahmenleisten.
- Rezept-Popups enthalten weiterhin keinen JEI und keine X-Schaltflaeche.

## Grafik

Die neue Grafik liegt in `src-ui/assets/cauldron-recipe-184.png`.
Sie wurde mit dem eingebauten Imagegen aus der inspizierten Kesselreferenz
erstellt. Original-Alpha und Bilddaten bleiben erhalten, ohne Raster-Nachbearbeitung.
Die Datei ist im endlichen Asset-Preload enthalten und wird fuer Kessel,
Stations-Tab und Alternativsymbol wiederverwendet.
[Vollstaendiger Prompt, Modus und Ursprungsdatei](imagegen-prompts.txt).

## Pruefung

3744 Tests bestanden, keine fehlgeschlagen, acht ignoriert. Workspace-Build,
Clippy fuer alle Targets mit `-D warnings`, Format- und Diff-Pruefung erfolgreich.
Das aktuelle Stylesheet besteht auch die direkte TiltUI-CSS-Parserpruefung.
Die neue Regression prueft vier eindeutige Holzalternativen, Auswahl,
begrenzte Navigation sowie unveraendertes Inventar und Reservierungen. Der
Event-Routing-Test prueft die Weitergabe von Rezeptindex und Holzvorschau.

40 native Aufnahmen: 25 im Spiel, zwoelf Menueansichten und drei echte
Start-/Preload-/Fade-Aufnahmen. Spiel- und Menueansichten sind mit echten
Betriebssystem-Klicks und Tastendruecken in 1680 x 936 und 1025 x 641 Pixeln
geprueft. Die Geometriepruefung vergleicht Crafting und Inventar, die 75-Prozent-
JEI-Zellen, die Tonne in der Leiste und die Abstaende der Menueaktionen zum
Rahmen. Ein nativer Klick auf die Birch-Alternativkarte wechselt die Vorschau.
Das autoritative Inventar bleibt nach allen Vorschauaktionen unveraendert.

Der Starttest durchlaeuft das echte Asset-Preload mit 460 Inventarvorschauen,
1633 UI-Assets und 1611 UI-Icons, prueft den Fade und startet weder Terrain
noch Server. Die Welt-Ladebilder verwenden repraesentative Fortschrittswerte
im echten Loading-View; sie behaupten keinen neuen Welt-Generierungslauf.
Die Menuepruefung erzeugt fuenf private Weltmanifeste und entfernt sie danach.
Create und Delete werden geoeffnet und abgebrochen. Bestehende Spielstaende
und Benutzereinstellungen werden nicht geoeffnet oder veraendert.

- Inventar: [gross](native/player-inventory-reference-wide.png),
  [klein](native/player-inventory-reference-compact.png),
  [Suchtext](native/catalog-search.png)
- Holzvarianten: [alle vier](native/chest-oak.png),
  [Birch per Alternativkarte gewaehlt](native/chest-birch-alternative.png)
- Kessel: [gross](native/cauldron-recipe-wide.png),
  [klein](native/cauldron-recipe-compact.png)
- [Start-Ladescreen](native/startup/startup-icons.png),
  [Fade](native/startup/startup-transition.png)
- Welt-Ladescreen: [gross](native/world-loading-wide.png),
  [klein](native/world-loading-compact.png)
- [Create World](native/singleplayer-create-dialog.png),
  [Delete World](native/singleplayer-delete-dialog.png)
- Menue-Abstaende: [Settings gross](native/settings-wide.png),
  [Settings klein](native/settings-compact.png),
  [Singleplayer](native/singleplayer-wide.png)

[Befehle](commands.txt), [Ergebnisse und PNG-Abmessungen](verification.json),
[Quellpruefsummen](source-sha256.json), [Protokolle](logs/).
