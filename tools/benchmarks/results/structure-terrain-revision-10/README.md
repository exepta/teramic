# Strukturrevision 10: Prüfung

`structures-tests.txt` enthält den isolierten finalen Lauf:

```sh
CARGO_BUILD_JOBS=2 cargo test --offline --target-dir target/world-validation \
  -p terra-world --features bevy --lib generation::structures::build::
```

Ergebnis: **41 Tests erfolgreich**, einschließlich der öffentlichen
`SectionGenerator`-Prüfung aller drei natürlichen Grabvarianten. Geprüft werden
auch echte Treppenkollisionshöhen, freie Außenzugänge, Truhen, unterschiedliche
Raumpläne und erhaltenes Gelände außerhalb örtlicher Fundamente.

`grave-sites.jsonl` enthält die mit `inspect_grave_sites` gefundenen, anschließend
im aktuellen World-Test geprüften Regionen für Seed42042 und `configs/world.toml`.
Die Fixturedatei enthält bewusst keine zwischengespeicherten Routen, Truhen oder
Bounds: Diese Daten werden vom aktuellen Planer und Native-Harness neu ermittelt.
So bleiben die Fixture-Koordinaten nutzbar, ohne veraltete Metadaten einer vorherigen
Truhennische zu übernehmen.

`royal-sites.txt` dokumentiert den echten Planer-Aufruf mit
`TERRA_ROYAL_ONLY=1 TERRA_STRUCTURE_CONFIG=configs/world.toml` und dem Beispiel
`structure_sites`. Er findet einen Außenposten in `[-7,-4]` und ein Königreich in
`[-27,7]`. Dabei werden keine Spielerwelten geladen oder verändert.

Native Bilder und die vollständige Workspace-Validierung werden separat vom
Hauptlauf protokolliert. Dieser Ordner behauptet keine GPU-Validierung.
