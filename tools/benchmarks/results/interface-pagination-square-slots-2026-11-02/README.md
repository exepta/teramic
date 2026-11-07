# Quadratische Slots, Pagination, Kessel, Hotbar und Startmenue

Phase 182, 2026-11-02. Die zwei Bildvorlagen sind unter `references/` abgelegt.

- Quadratische Rucksack- und Inventar-Hotbar-Slots. Das Inventar passt seine
  Breite an die verfuegbare Hoehe an; beim Einklappen bleiben die Slots quadratisch.
- Rechtsbuendige Pagination. Explizite Grid-Tracks tolerieren Rundung auf echte
  Pixel. Aenderungen der Kapazitaet erhalten den bisherigen Item-Offset;
  Suche und Seitenwechsel normalisieren den internen Seitenindex.
- Rezept-Popups enthalten keinen JEI-Itembrowser. Das Inventar bleibt im
  Hintergrund abgedunkelt sichtbar, E/ESC schliessen weiterhin zuerst das Popup.
- Oak, Spruce, Birch und Cherry erscheinen als vier waehlbare Vorschauen;
  Auswahl aendert ausschliesslich die Anzeige, nicht Reservierungen oder Items.
- Kesselansicht nach Vorlage mit echten Zutaten, Wasser, wiederverwendbarem
  Ruehrwerkzeug, Kesselmodell, Ergebnis, Dauer und registrierten Alternativen.
  Der Fortschrittsbalken zeigt ausschliesslich die autoritative Kesselarbeit;
  reine Rezeptvorschau beginnt ohne erfundenen Fortschritt.
- Neue Spiel-Hotbar mit Kupferrahmen, Blaettern, Kristallen, Nummernplaketten,
  echten Itemmodellen, Mengen und Auswahl. Keine dekorativen Demo-Items.
- Startmenue mit gemaltem Landschaftshintergrund, freigestelltem Logo und
  einzeln bedienbaren Buttons. Versionsnummer stammt aus dem Build.

## Grafikassets

Builtin-Imagegen, Referenz: `references/main-menu-hotbar.png`. Die PNGs bleiben
mit Original-Alpha erhalten. Bei Button und Hotbar clippt der UI-Renderer nur
das transparente Aussenpadding ueber `ImageNode.rect`; keine Raster-Nachbearbeitung.

- `src-ui/assets/menu-valley-182.png`
- `src-ui/assets/menu-logo-182.png`
- `src-ui/assets/menu-button-182.png`
- `src-ui/assets/hotbar-frame-182.png`

Promptset und Herkunft stehen in `imagegen-prompts.txt`.
Die vier Texturen sind im endlichen Asset-Preload vor dem Startmenue enthalten.

## Pruefung

3740 Tests bestanden, keine fehlgeschlagen, acht ignoriert. Workspace-Build,
Clippy fuer alle Targets mit `-D warnings`, Format- und Diff-Pruefung erfolgreich.
Neue Regressionen pruefen Seitenbegrenzung, Pixelrundung/Kapazitaetswechsel,
alle vier Holzvorschauen und den Kesselfortschritt ohne Inventarrevision.

24 native Aufnahmen: 21 im Spiel und drei im Startmenue. Echte Betriebssystem-
Klicks und Tastendruecke pruefen Suche, Kategorie-Tabs, Seitennavigation, Buch,
R zum Oeffnen des Rezept-Popups, E/ESC, vier Holzauswahlen, Ofenbrennstoffe,
Kessel und Hotbar-Auswahl. Die Geometriepruefung kontrolliert quadratische
Slots, rechtsbuendige Pagination, fehlenden JEI im Popup und sichtbare Inhalte
in 1680 x 936 sowie 1025 x 641 Pixeln. Die angeforderte kleine Fensterbreite
1024 wird von KWin bei gebrochener DPI auf 1025 physische Pixel gerundet.

Der Test verwendet eine temporaere integrierte Autoritaet mit unveraendertem
Inventar nach den Vorschauaktionen. Bestehende Spielstaende werden nicht
geladen. Die Menuepruefung startet keinen Server und laedt kein Terrain;
Settings und Back werden ueber die echten Buttons bedient.

Die letzte reine CSS-Anpassung verkleinert die zuvor beschnittene Trankvorschau;
sie ist in den abschliessenden nativen Aufnahmen enthalten. Der vollstaendige
Workspace-Testlauf fand vor dieser kosmetischen Anpassung statt.

- [Inventar gross](native/player-inventory-reference-wide.png),
  [Inventar klein](native/player-inventory-reference-compact.png),
  [eingeklappter Browser](native/player-inventory-catalog-collapsed.png)
- [Katalogseite zwei](native/catalog-page-two.png),
  [Suche](native/catalog-search.png)
- Truhe: [Oak](native/chest-oak.png), [Spruce](native/chest-spruce.png),
  [Birch](native/chest-birch.png), [Cherry](native/chest-cherry.png)
- Kessel: [gross](native/cauldron-recipe-wide.png),
  [klein](native/cauldron-recipe-compact.png)
- Hotbar: [gross](native/hotbar-wide.png), [klein](native/hotbar-compact.png)
- Startmenue: [gross](native/start-menu-wide.png),
  [klein](native/start-menu-compact.png)

[Befehle](commands.txt), [Pruefergebnisse und PNG-Abmessungen](verification.json),
[Quellpruefsummen](source-sha256.json), [Protokolle](logs/).
