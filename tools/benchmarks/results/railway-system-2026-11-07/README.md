# F62 — Modulares Schienensystem

Implementierungs- und Bedienbericht vom 07.11.2026. Maßgebliche Gestaltungsvorlage:
[Railway Tracks — Modular 3-Block-Wide Rails](../../../../docs/references/railway-tracks-three-blocks.png).

Alle zwölf Workspace-Library-/Bin-Testsuiten bestehen: **4.154 Tests erfolgreich,
keiner fehlgeschlagen, neun bestehende Tests ignoriert**. Clippy aller Targets mit
`-D warnings`, Formatierung, Diff und der abschließende Workspace-Build sind sauber.

Auf ausdrücklichen Nutzerwunsch werden keine Grafiktests, Spielclients,
GPU-Galerien oder Screenshots gestartet. Der Asset-Export erfolgte mit einem
isolierten kopflosen `rustc`-Hilfsprogramm und Python. Die Prüfung der Modell- und
Polygonzahlen benötigt kein Grafikgerät.

## Module und Maße

Ein einzelner Gleisstrang hat drei Blöcke Nennbreite, warme Holzschwellen und zwei
Stahlrails mit getrenntem Fuß, Steg und Kopf. Die Schwellen sind 2,9 Blöcke breit;
die nominelle Spurweite beträgt zwei Blöcke zwischen den Rail-Mittellinien.
Bolzen befestigen die Rails auf dem Holz. Kurven, Kreuzungen und Weichen benötigen
ihren tatsächlichen größeren Mehrzellenbereich; der Weichenhebel steht seitlich
am Gleis und besitzt einen roten Griff.

Die Anschlussfläche liegt 0,125 Blöcke über dem Modulanker. Darauf kommen 0,1875
Blöcke Railhöhe. Angegebene Längen sind horizontale Ausdehnungen beziehungsweise
Mittellinienradien; ein Block entspricht einer Welteinheit.

| ID | Item / Asset | Geometrie |
| ---: | --- | --- |
| 196 | `rail_straight` — Gerade | Länge 4, eben |
| 197 | `rail_curve_small` — Kleine Kurve | Viertelkreis, Mittellinienradius 4,5; Bogenlänge etwa 7,069 |
| 198 | `rail_curve_large` — Große Kurve | Viertelkreis, Mittellinienradius 8,5; Bogenlänge etwa 13,352 |
| 199 | `rail_slope` — Steigung | Horizontal 4, Höhengewinn 1; konstante Steigung 1:4 beziehungsweise 25 % |
| 200 | `rail_slope_entry` — Steigungseinfahrt | Horizontal 8, Höhengewinn 1; sanfter Übergang von 0 auf 25 % Steigung |
| 201 | `rail_slope_exit` — Steigungsausfahrt | Horizontal 8, Höhengewinn 1; sanfter Übergang von 25 % auf 0 Steigung |
| 202 | `rail_switch_left` — Linke Weiche | Hauptstrang Länge 8, linker Abzweig mit Radius 4,5 |
| 203 | `rail_switch_right` — Rechte Weiche | Hauptstrang Länge 8, rechter Abzweig mit Radius 4,5 |
| 204 | `rail_crossing` — Kreuzung | Zwei getrennte, rechtwinklige Wege mit je Länge 7 |
| 205 | `rail_buffer` — Prellbock | Gerades Modul Länge 4 mit geschlossenem Ende und sichtbarem Anschlag |

Alle Module lassen sich in vier horizontalen Ausrichtungen platzieren. Die
Steigungsübergänge sind echte Parabelprofile: Bei normiertem Fortschritt `t`
steigt die Einfahrt um `t²`, die Ausfahrt um `2t − t²`. Damit stimmen sowohl
Anschlusshöhe als auch Anschlusstangente mit dem ebenen beziehungsweise dem
1:4-Gleis überein. Die Steigungsstrecke selbst hat räumliche Länge `√17`.

## Bedienung und Platzierung

1. Ein Schienenmodul über das Crafting-Rezept beziehungsweise Too Many Items
   auswählen und in die Hand nehmen. Ein Item repräsentiert die vollständige
   Baugruppe, unabhängig von ihrer Anzahl belegter Zellen.
2. Auf einen tragfähigen Unterbau zeigen und rechtsklicken. Auf gewöhnlichem
   Boden folgt die Ausrichtung der Blickrichtung; der vollständige Ghost zeigt
   die Baugruppe an ihrem tatsächlichen Anker.
3. Beim Zeigen auf ein bestehendes Modul wird ein freies, erreichbares Ende
   gewählt. Der gemeinsame Platzierungsplan prüft exakte Anschlussposition,
   Gegenrichtung und Neigung. Alle vier Ausrichtungen und die passenden
   Modulanschlüsse werden begrenzt geprüft. Ein geometrisch unpassender Anschluss
   wird abgelehnt, statt ein nur scheinbar verbundenes Gleis zu erzeugen.
4. Ungültige Schienenplatzierungen erhalten eine rote Kontur; gültige behalten
   die bestehende pulsierende Kontur. Die Vorschau bewertet alle belegten Zellen,
   Unterbau, Kollision und Baurechte. Ein roter Entwurf verändert keine Weltzelle
   und verbraucht kein Item.
5. Mit leerer Hand auf eine Weiche rechtsklicken, um zwischen Hauptstrang und
   Abzweig umzuschalten. Stahlzungen und roter Hebel ändern ihre sichtbare Stellung.
   Mit einem Schienenitem in der Hand hat stattdessen der Anbau eines Moduls
   Vorrang. **Shift** unterdrückt die gewöhnliche Rechtsklick-Interaktion;
   Platzieraktionen des gehaltenen Bauitems bleiben dadurch erreichbar.
6. Linksklick baut ein anvisiertes Modul ab; im Survival-Modus gelten Abbaudauer
   und geeignetes Werkzeug. Auch das Treffen einer Sekundärzelle adressiert die
   vollständige Baugruppe. Erfolgreicher Survival-Abbau erzeugt genau ein
   Modulitem; Creative erzeugt keinen zusätzlichen Drop.

Ein normaler Bauvorgang verbraucht in Survival genau ein Item aus dem bestätigten
Inventarslot. Platzierung, Inventarverbrauch, Abbau und Weichenstellung werden
serverseitig geprüft. Eine Überlappung, veraltete Revision, fehlendes Item oder
fehlendes Recht lässt die gesamte Transaktion unverändert.

Die zehn Werkbankrezepte verwenden voneinander unterscheidbare geformte
3×3-Muster. Dadurch verwechselt das automatische Befüllen keine Module mit
denselben Gesamtzutaten. Alle vier vorhandenen Plankenarten bleiben zulässig;
die Gesamtmaterialkosten und der Output von einem Modul sind pro Rezept fest.
Die Holzschwellen verwenden die vorhandenen Holz-Kontaktgeräusche.

## Unterbau, Raum und Rechte

Jeder vom gemeinsamen Profil vorgesehene Stützpunkt benötigt einen geladenen,
tragfähigen Boden mit voller Oberseite. Untere Slabs, andere Schienen und
nichttragende Objekte ersetzen diesen Unterbau nicht; geeignete obere beziehungsweise
vollständige Slabs werden als Stütze berücksichtigt. Eine Steigung braucht daher
auch auf ihren höheren Abschnitten einen passenden Unterbau.

Der belegte Raum folgt dem tatsächlichen Profil und füllt keinen pauschalen
rechteckigen Kasten um eine Kurve. Spieler, vorhandene Baugruppen, relevante
Props und gesperrte Zellen dürfen nicht von einem Modul überbaut werden.
Waterlogging erhält vorhandenes Wasser um die Teilgeometrie; ein Abbau lässt das
Wasser bestehen. Reguläre Blockänderungen dürfen einen vorhandenen Schienenunterbau
nicht ungültig machen, auch wenn die zugehörige Schienensektion gerade ausgeladen ist.

Baurechte werden auf jeder belegten Zelle geprüft. Dass der Anker im eigenen
Gebiet liegt, reicht bei einem über eine fremde Grenze reichenden Modul nicht aus.
Für den Abbau gelten die Abbaurechte der gesamten Baugruppe; Weichenumschaltungen
verwenden die autoritative Interaktionsberechtigung. Reichweite und tatsächlicher
Sichtstrahl werden ebenfalls am Server geprüft.

## Darstellung und gemeinsame Geometrie

[Core-Profile](../../../../crates/terra-core/src/rails.rs) liefern Balken,
Anschlüsse, Footprint, Stützpunkte und Kollisions-/Auswahlformen. Rampen verwenden
ihre tatsächlichen geneigten beziehungsweise gescherten Querschnitte;
[gemeinsame sichtbare Flächen](../../../../crates/terra-core/src/rails/surfaces.rs)
werden direkt daraus abgeleitet.

Die numerische Prüfung fand zunächst an den Weichenfröschen gleichgerichtete,
coplanare Kopfoberflächen mit zusammen etwa 0,017016 m² Überdeckung pro Weiche und
Stellung. Die gemeinsame Flächenaufbereitung entfernt solche Überdeckungen über
alle Materialrollen. Exporter und Renderer verwenden dieselben disjunkten
Konvexflächen, ohne Tiefenoffsets gegeneinander zu verschieben. Die erneute
Dreiecksprüfung zählt bei der Toleranz des tatsächlichen float32-Meshformats null
relevante Überdeckungen; gemeinsame Schnittkanten erzeugen höchstens Rundungsreste
von wenigen `10⁻⁸ m²`.

Der Terrain-Renderer zerlegt die Baugruppe in disjunkte Zellflächen. Eine
Sekundärzelle zeichnet ihren eigenen Ausschnitt und dupliziert kein vollständiges
Modul. Damit bleiben horizontale und vertikale Sectiongrenzen sowie negative
Koordinaten konsistent. Ein geladener Ausschnitt benötigt zur Formdarstellung
keinen zusätzlich geladenen Ankerchunk.

Der Mehrzellen-Ghost zeichnet die komplette Baugruppe genau einmal am tatsächlichen
Anker. Itemicons und Drops werden als ganze Baugruppe eingepasst. Die gewöhnliche
Einzellen-Mining-Isolation ist für Rails ausgenommen, damit beim Abbau keine ganze
Baugruppe zusätzlich über den verbleibenden Section-Slices dargestellt wird.
Die sichtbare Schiene bleibt bis zur bestätigten Mehrzellenänderung stabil.

Die zehn nativen JSON-/GLTF-/BIN-Modelle liegen unter
[`assets/terra/blocks`](../../../../assets/terra/blocks). Reproduzierbarer,
kopfloser Export gegen die bestehenden Build-Artefakte:

```sh
python3 tools/assets/build_rails.py --export-core
```

[`export_rails.rs`](../../../../tools/assets/export_rails.rs) liest dabei die
Produktionsquellen; der Python-Generator enthält keine zweite Kurven- oder
Steigungsformel. Das Manifest `models/rail_profiles.json` sowie die GLTF-Metadaten
enthalten Source-SHA256 und einen zusätzlich CPU-prüfbaren FNV-Fingerabdruck.
Die Assettests vergleichen exportierte Balkenecken und sichtbare Flächen erneut
mit den aktuellen Core-Profilen. Temporärer Core-Rohdatenexport und ausführbarer Helfer bleiben
unter `target/`; der Generator und seine Quelle sind Teil des Repositories.

## Speicherung, Ownership und Graph

Jede belegte Zelle besitzt persistente Objektmetadaten:

- `peer` verweist auf den kanonischen Modulanker, auch am Anker selbst.
- `secondary` ist genau für die übrigen Zellen gesetzt.
- `facing` enthält die vier Ausrichtungen und bei Weichen zusätzlich Bit 4 für
  die Abzweigstellung. Alle Zellen eines Moduls tragen dieselbe Stellung.

Die bestehende Objekt-/Journal-Speicherung transportiert diese Informationen;
F62 benötigt keine neue Save-Schemaversion. `SAVE_FORMAT_VERSION` bleibt 2.
Bestehende Block-IDs werden nicht recycelt. Das Netzprotokoll ist **111**;
Teilplatzierungen und erfundene Besitzerbezüge werden nicht als vollständige
Baugruppe akzeptiert.

Der Streckengraph wird aus vollständigen gespeicherten Modulen rekonstruiert,
einschließlich nicht geladener Journal-Sektionen. Unvollständige Importe ergeben
keine befahrbare Route und kein zweites Modulitem. Ein eigener Rail-Epoch- und
Identitätscache verhindert Neubauten des Graphen nach sachfremden Terrainänderungen.
Modul- und Verbindungseinträge sind kanonisch sortiert; Einfügereihenfolge,
Save/Reload und das Entfernen/Wiedereinfügen erzeugen denselben Graphen.
Der Raypicker behält die tatsächlich sichtbare Sekundärzelle, statt sie zum
Anker umzuschreiben; erst die autoritative Transaktion löst den Modulbesitzer auf.

Anschlüsse verwenden exakte Sechzehntelblock-Koordinaten und entgegengesetzte
Tangenten; visuelle Nähe allein verbindet keine Module. Die Kreuzung enthält
zwei unabhängige Wege. Die Weiche gibt nur ihren ausgewählten Weg frei; der
Prellbock besitzt ein geschlossenes Ende. Die deterministische Routensuche bewertet
Streckenlängen und hat feste Arbeitsgrenzen:

| Grenze | Wert |
| --- | ---: |
| Module im Graph / Journal | 8.192 |
| Belegte Zellen einer einzelnen Transaktion | höchstens 512 |
| Besuche einer Routensuche | höchstens 32.768; ein kleineres Aufruferbudget gilt zusätzlich |
| Distanzauflösung des Graphen | 1/1.024 Welteinheit |

## CPU-Prüfumfang und Gesamtabnahme

Vorhandene beziehungsweise ergänzte Regressionen prüfen:

- alle Modularten und Ausrichtungen, exakte Ports, sanfte Steigungsübergänge,
  aktive Weichenwege und begrenzte deterministische Routensuche;
- atomare Mehrzellen-Platzierung und -Entfernung, genau ein Survival-Item pro
  Modul, Rechte über Chunkgrenzen, veraltete Slots/Revisionen und volle Dropkapazität;
- stabile Weichen-Footprints und identische Metadaten aller Zellen nach dem
  Umschalten, vollständige Speicherung, ausgeladene Besitzer und Late Join;
- echten Unterbau, geschützte Stützzellen, Props und erhaltenes Wasser;
- tatsächliche Client-Vorschau und Weicheninteraktion sowie einen echten lokalen
  UDP-Pfad ohne natives Fenster oder Grafikgerät;
- GLTF-Import, Source-Provenienz, gemeinsame Auswahl-/Kollisionshüllen,
  disjunkte Flächen aller Materialrollen und Flächenerhaltung beim Schneiden;
- vollständige Multiblock-Ghosts, eingepasste Itemicons, rote ungültige Vorschau
  und negative beziehungsweise vertikale Sectiongrenzen.

Wichtige Quellen:
[Core-Tests](../../../../crates/terra-core/src/rails/tests.rs),
[World-Tests](../../../../crates/terra-world/src/rails/tests.rs),
[Authoritytests](../../../../crates/terra-network/src/server_rail_tests.rs),
[UDP-Regression](../../../../crates/terra-network/src/railway_tests.rs),
[Render-/Assettests](../../../../crates/terra-render/src/meshing/custom_rails_tests.rs).

| Zentrale Prüfung | Stand dieses Berichts |
| --- | --- |
| Workspace-Build | Aktuelle ausführbare Client-/Serverdateien erfolgreich gebaut (2m 49s) |
| Clippy | Alle Workspace-Targets mit `-D warnings` bestanden |
| CPU-Tests | 4.154 bestanden; 0 fehlgeschlagen; 9 bestehende Tests ignoriert |
| Format / Diff | Beide Prüfungen bestanden |
| Grafik-/GPU-Tests | Auf ausdrücklichen Nutzerwunsch nicht gestartet |

Der erste Lauf fand Rezeptkollisionen, eine reihenfolgeabhängige Verbindungsliste,
die generische Umleitung sekundärer Rayhits zum Anker sowie eine fehlende
Schrittgeräusch-Zuordnung. Die Korrekturen betreffen die tatsächlichen
Produktpfade. Die unveränderten strengen Speicher-, UI- und Serverassertions
bestehen danach; neue Grid- und Rayregressionen sichern die Ursachen zusätzlich ab.

Prüfbefehl:

```sh
cargo test --workspace --lib --bins --locked --no-fail-fast -j1 -- --test-threads=4
```

Cargo läuft ohne `DISPLAY`/`WAYLAND_DISPLAY`, mit einem Job, Nice 19 und
IO-Klasse 2 / Priorität 7. Es wurden keine Grafiktests oder Spielclients gestartet.
Die abschließenden Quell- und Loghashes stehen in `verification.json`.
