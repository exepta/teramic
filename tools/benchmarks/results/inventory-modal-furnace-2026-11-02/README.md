# Kompaktes Inventar und Rezept-Popup

Phase 181, 2026-11-02. Nacharbeit an den drei neuen UI-Referenzen.

- Inventar und Rezept-Popup ohne X-Schaltflaechen. E und ESC schliessen zuerst
  das Rezept-Popup und danach das darunter erhaltene Inventar. R ueber einem
  Katalogitem oeffnet dessen Rezept.
- Kompakter, zentrierter Inventarrahmen. Das Buch klappt den Itembrowser ein und
  aus. Kategorie-Tabs stehen wieder als vertikale Leiste links am Browser;
  der Filterknopf setzt auf alle Kategorien zurueck.
- Das Rezept-Popup liegt vor dem weiterhin sichtbaren, abgedunkelten Inventar.
  Der Hintergrund nimmt keine Zeigeraktionen an. Die Seitenzahl steht mittig
  zwischen den Pfeilen im Rezeptbereich.
- Eigene Ofenansicht nach der dritten Referenz mit echtem Input, Brennstoff,
  Ofenmodell und Ergebnis. Schmelzdauer und Brennzeiten stammen aus der
  vorhandenen Registry; angezeigte Ticks verwenden den konfigurierten Servertakt.
  Die Brennstoffauswahl ist eine Vorschau und verbraucht keine Items.
- Holz-Wildcards sind als „Any wood planks“ mit Oak, Birch, Spruce und Cherry
  beschrieben. Die Truhe verlangt unveraendert acht beliebige Holzplanken.

Referenzen: [Inventar](references/inventory-compact.png),
[Positionen](references/fix-position.png),
[Ofen](references/furnace-recipe.png).

## Aufnahmen

- [Kompakteres Inventar](native/player-inventory-reference-wide.png)
- [Eingeklappter Itembrowser](native/player-inventory-catalog-collapsed.png)
- [Kategorie-Tabs links](native/inventory-left-category-tabs.png)
- [Crafting-Popup ueber dem Inventar](native/crafting-recipe-reference-wide.png)
- [Truhe mit beliebigen Holzplanken](native/chest-any-wood-popup.png)
- [Ofenrezept](native/furnace-recipe-reference-wide.png)
- [Alternative Brennstoffe](native/furnace-alternative-fuels.png)
- [Ofenansicht im kleinen Fenster](native/furnace-recipe-reference-compact.png)

## Pruefung

3735 Workspace-Tests bestanden, keiner fehlgeschlagen, acht ignoriert; letzter
vollstaendiger Lauf mit vier Testthreads. Im ersten unbegrenzten parallelen Lauf
ueberschritt ein vorhandener Netzwerk-Disconnect-Test sein Fuenfsekundenlimit.
Er bestand anschliessend unveraendert sowohl isoliert als auch im gesamten Lauf.
Das erste Protokoll und die Wiederholung sind in `logs/` erhalten.

13 native Vulkan-Aufnahmen: zehn mit 1680 x 936 und drei mit 1025 x 641 Pixeln.
Echte Betriebssystem-Klicks pruefen Buch, Tabs, Reset, Pagination und Suche;
R oeffnet ein Rezept, E und ESC pruefen die Reihenfolge beim Schliessen.
Brennstoffvorschau und Navigation muessen den autoritativen Inventarinhalt
unveraendert lassen. Zusaetzliche Geometriepruefungen erkennen Ueberlappung der
Kategorieleiste, eine versetzte Seitenzahl und abgeschnittene Ofeninhalte.

Headline-Widgets erhalten eigene Stilklassen, weil HTML-Ueberschriften intern als
`headline` materialisiert werden. Die Titelplakette wird beim eingeklappten
Browser verbreitert, damit der Titel einzeilig bleibt. Lange Itembeschreibungen
haben einen eigenen Scrollbereich mit Platz fuer vollstaendige Textzeilen.

Build, Clippy mit `-D warnings`, Format und Diff-Pruefung sind in `logs/`
dokumentiert. Befehle stehen in `commands.txt`, Einstellungen in `configs/`,
Quellhashes in `source-sha256.json`. Aufnahmengroessen und Ergebniszahlen werden
zusaetzlich in `verification.json` gespeichert.
