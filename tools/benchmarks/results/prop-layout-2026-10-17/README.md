# Phase 37: Prop-Groessen Und Zentrale Gruppen

## Regeln

Die Prozentangaben bezeichnen bei Blumen/Pilzen die Hoehe ueber dem Boden;
25 Prozent bezeichnen die maximal reservierte XZ-Grundflaeche: 0.5 x 0.5 m.
Blumen erreichen im Nahbereich 0.65-0.95 m, kleine Pilze 0.35-0.55 m.
Gras bleibt unveraendert mittig und kostet zwei der vier Belegungseinheiten.

Ohne Gras: eine Pflanze mittig, zwei nebeneinander, drei als Dreiergruppe,
vier als Quadrat. Der Abstand folgt den gemeinsamen Template-Grenzen mit
kleinem Zwischenraum. Mit Gras stehen Begleiter diagonal versetzt. Pro Gruppe
werden nur vier Referenzen auf dem Stack sortiert, keine neuen Prop-Listen
pro Bodenblock angelegt. Neue Pflanzen erhalten deterministische Groessen.

Die Hoehenlimits werden auch bei gespeicherten Blumen/Pilzen rein visuell
eingehalten. Alte unpassend grosse oder frei gedrehte Objekte werden nicht
zwangsweise in Gruppen geschoben. Ihre Nachbelegung wird abgelehnt, wenn sie
nicht in die gemeinsamen Bounds passen.

## Persistenz Und Darstellung

Anordnungen sind aus aktiven IDs abgeleitet, nicht aus Paket-/HashMap-Reihenfolge.
Nach Hinzufuegen/Entfernen duerfen verbleibende Pflanzen auf demselben Block
ihre Position wechseln. Ihre IDs, Arten, Rotationen und Groessen bleiben gleich.
Benachbarte Bodenbloecke aendern sich nicht. Gespeicherte Prop-Rohdaten bleiben
unveraendert; Generierung, Commit und Journal-Anwendung berechnen identische
Darstellungspositionen. Keine neuen Save-Felder oder zweite Speicherung.

Kanonische Meshes passen Breite und Hoehe getrennt an. Ausmasse werden einmal
je Art/Variante aus allen drei LODs ermittelt und gecacht; Near definiert die
exakte Blueten-/Pilzhoehe. Einfachere LODs verwenden dieselbe Transformation,
koennen aber bestehende kleinere Silhouetten behalten. Normalen erhalten die
inverse Skalierung und werden normalisiert. Rendering, Bounds und Dreiecks-
Picking verwenden weiterhin dieselbe Geometrie; Wind bleibt aktiviert.

Keine neuen Meshvertices, Materialien oder Einzelprop-Entities. Keine neue
vergleichbare FPS-/Speichermessung fuer diese Anpassung: die groesseren
Silhouetten und einmaligen Template-/Gruppenberechnungen sind nicht als
kostenlos oder als Performanceparitaet ausgewiesen. Historische Phase-36-
Messungen bleiben unveraendert und messen nicht diesen neuen Stand.

## Native Reproduktion

```bash
env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/fluid-recession-mushrooms-2026-10-16/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra-prop-layout-native-menu \
  TERRA_SCREENSHOT=tools/benchmarks/results/prop-layout-2026-10-17/captures/botanical.png \
  target/debug/terra-client --smoke-test-botanical
```

Die Nahkamera zielt jetzt auf die wirkliche hoechste Bluetendreiecksflaeche der
geteilten Geometrie statt auf eine alte fest kodierte Bluetenhoehe. Das
Produktions-Picking wird fuer den Test weder verbreitert noch vereinfacht.

## Validierung

Alle fuenf Cargo-Pruefungen erfolgreich: `fmt --all --check`, `check --workspace`,
`test --workspace`, `clippy --workspace --all-targets -- -D warnings` und
`build --workspace`. 623 Tests bestanden, ein bestehender Langzeittest bleibt
ignoriert. Logs: `fmt.log`, `check.log`, `test.log`, `clippy.log`, `build.log`.

Geprueft sind unter anderem echte Hoehen/Grundflaechen fuer die Varianten und
LODs, IDs und Anordnung nach 1/2/3/4 Pflanzen, Gras hinzufuegen/entfernen,
negative Koordinaten, wiederholter Reflow, Paket-Reihenfolge, echte UDP-
Platzierung mit zwei Clients und Journal-/Section-Reload. Verbleibende
Nachbarbloecke und Terrain-Meshepochen bleiben unveraendert. Render-/Picking-
Bounds werden mit den wirklichen kanonischen Varianten und Windextrema
statt alten radialen Groessenannahmen geprueft.

Der native Botanical-Test auf RX 6950 XT/Vulkan bestand mit sieben Aufnahmen
einschliesslich Gras-/Busch-Windpaaren und echter HUD-/Zielauswahl. Die Blumen-
und Buschaufnahmen wurden geoeffnet. Die Bush-Fixture hatte zuvor einen
niedrigen Blickwinkel, in den bewegtes Nachbargras hineinragte; sie blickt nun
von naeher/hoeher auf die echte Kronengeometrie und prueft mehrere Windposen.
Die produktive Auswahl wurde dafuer nicht veraendert. Keine dedizierte native
Pilz-Nahaufnahme; Pilzgroessen und Auswahl sind geometrisch getestet.

Der Dedicated Server startete ohne DISPLAY/WAYLAND_DISPLAY und beendete seinen
Headless-Smoke erfolgreich mit 27 Sections (`server.log`).

Aufnahmen: [Blumen](captures/botanical-flowers.png),
[Blumendetails](captures/botanical-flowers-detail.png),
[Busch](captures/botanical-bush.png).
