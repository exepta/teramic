# Kupferinventar, Crafting Recipes und Wasser an Rohren

Phase 180, 2026-11-02. Native Vulkan-Aufnahmen der echten Spieloberflaeche und
Weltgeometrie; die UI-Bilder enthalten echte Inventare, Itemmodelle und Rezepte.

## Rohrfehler

[Vorher](before-fluid-face-fix.png) / [Nachher](after-fluid-face-fix.png): dieselbe
Unterwasserkamera in einer isolierten Kopie der vorhandenen Testwelt. Der
Originalspielstand wurde nicht zum Testen geoeffnet oder veraendert.
Die gespeicherte Wassermenge im Rohr war bereits korrekt. `custom::compose`
rekonstruierte jedoch fehlende Voxelkontaktflaechen des Materials 27 als ganze
Wasserwuerfel, zusaetzlich zum eigentlichen Wasser-Renderer. Material 27 wird nun
bei dieser Rekonstruktion wie Luft ausgelassen. Der Regressionstest nutzt die
vier echten Rohrmodelle innerhalb einer Section und jenseits aller sechs Grenzen.
Die vorherigen reinen Wasserpatch-Tests konnten den Fehler nicht erfassen, weil
die fehlerhaften Flaechen aus dem Terrain-Renderer kamen.

Die weitere Korrektur fuer dekoratives Material 14 erhaelt beim Rohrsetzen acht
Wassereinheiten. Interne Kontakte zwischen seinen Wuerfelflaechen und vollstaendig
wassergefuellten Modellen werden ausgeblendet und nach Wasserpublikationen gezielt
aktualisiert. Die Eigenschaften eines unveraenderten dekorativen Wasserblocks
bleiben erhalten.

## Neue Oberflaeche

- [Player Inventory](ui-final/player-inventory-reference-wide.png)
- [Crafting Recipe](ui-final/crafting-recipe-reference-wide.png)
- [Alternative Rezepte](ui-final/crafting-recipe-alternatives.png)
- [Kompaktes Inventar](ui-final/player-inventory-reference-compact.png)
- [Kompakte Rezepte](ui-final/crafting-recipe-reference-compact.png)
- [Kategorieauswahl](ui-final/recipe-category-filter.png)
- [Itemsuche](ui-final/recipe-browser-search.png)

36 echte Rucksackplaetze, neun Hotbarplaetze und die bestehenden zehn
Ausruestungsplaetze. Suche, Filter, Itemseiten, Rezeptbuch und Schliessen werden
im nativen Lauf per echten Betriebssystem-Klicks geprueft. Die autoritative
Inventarkopie muss nach dem Blaettern unveraendert sein. Die breiten und kompakten
Ansichten pruefen sichtbare Icons, Elementgrenzen und den zentrierten Rahmen.
Die Abbildungen der Vorlage sind Gestaltungsreferenzen; Gegenstaende, Mengen,
Rezeptalternativen, Kategorien und Avatar stammen aus dem Spielzustand.

[Inventarvorlage](references/player-inventory.png),
[Rezeptvorlage](references/crafting-recipe.png),
[Fehlerbild](references/water-problem.png).
Der transparente Kupferrahmen wird als Nine-Slice verwendet; Prompt und Herkunft
stehen in [imagegen-frame-prompt.txt](imagegen-frame-prompt.txt).

## Kompatibilitaet

Inventare verwenden weiterhin die vorhandene vektorbasierte Speicherung; alte
27-Plaetze-Inventare bleiben gueltig. Das gemeinsame Limit ist jetzt 45. Die
kombinierte Ansicht mit zwei Truhenhaelften hat 81 Plaetze. Begrenzte Tuple-Serde
uebertraegt auch die erweiterten Drag-/Reservierungsarrays ohne unbeschraenkte
Array-Allokation. Binaer- und JSON-Roundtrips sowie die alten Inventardateien
werden geprueft. Netzwerkprotokoll 99 erfordert passende Client-/Serverstaende.

## Pruefung

`cargo test --workspace --locked -j1`: **3732 bestanden, 0 fehlgeschlagen,
8 ignoriert**. Workspace-Build, Clippy fuer alle Targets mit `-D warnings`,
Formatpruefung und `git diff --check` sind erfolgreich.

Native Vulkan-Pruefungen:

- Hydraulik: 35 Aufnahmen, inklusive gefuellter/geleerter Rohre, offener und
  geschlossener Ventile, Wasserbecken, echter Infuser-Transfers und Wasserlogging.
- Teilbloecke: neun Aufnahmen, inklusive einer echten Unterwasserkamera;
  Unterwasserzustand und Blend werden vor der Aufnahme geprueft.
- Rohr-Ufer: fuenf Aufnahmen fuer natuerliches, fliessendes und dekoratives Wasser.
- Inventar/Rezeptbuch: acht Aufnahmen, sechs mit 1680 x 936 und zwei mit
  1025 x 641 Pixeln; Zentrum, Grenzen und alle zehn Ausruestungsplaetze geprueft.
- Vorher/Nachher: zwei Aufnahmen der gespeicherten Testwelt in isolierter Kopie.

Die abschliessende native Inventarpruefung kontrolliert Buch, Filter, Suche,
Seitenauswahl, Rezeptalternativen und Schliessen mit Betriebssystem-Klicks sowie
die unveraenderte autoritative Inventarkopie. Ein Resize muss vor der Aufnahme
auch in UI-Layout und GPU-Ziel angekommen sein. Physische Aufnahmegroessen stehen
in `verification.json`; die fraktionale Wayland-Skalierung darf um einen Pixel
runden.

Rohprotokolle liegen in [logs](logs/), Befehle in [commands.txt](commands.txt),
Grafikeinstellungen in [configs](configs/). `verification.json` enthaelt die
Pruefergebnisse, Bildgroessen und verweist auf `source-sha256.json` mit SHA-256-Hashes der betroffenen Quellen.

Der Blur auf dem Inventar-Layoutroot wurde entfernt: Der Effekt hielt beim
Verkleinern seine alte Breite als feste Pixelgroesse fest und beschnitt den
Itembrowser. Das Layout behaelt nun die tatsaechliche Fenstergroesse.
