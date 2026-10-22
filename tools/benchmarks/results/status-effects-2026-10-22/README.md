# Phase 104: F01 Status Effects

Stand: 2026-10-22. Implementierung, Workspace-Pruefungen und native
Funktions-/GPU-Aufnahmen abgeschlossen. Keine neue Langzeit-/FPS-Paritaet
behauptet.

## Gameplay und Daten

Die 13 Arten verwenden [`assets/terra/effects`](../../../../assets/terra/effects/README.md),
keine zweite Spielerwerte- oder Inventarregistry. Je Art maximal ein zeitlicher
Slot; begrenzte Dauer/Level und konfigurierte Refresh-/Extend-/Replace-Regeln.
Poison, Freeze, Bleeding, Rotten und On Fire verursachen ihre definierten
Lebens-/Hunger-/Bewegungseffekte. Speed, Slow und Water Affinity verwenden die
gemeinsame autoritative/vorhergesagte Physik. Strength veraendert Nahkampfschaden;
Resistenzen verhindern passenden Schaden, Frostresistenz auch den Kaeltemalus.
Shield Power ist ein verbrauchbarer Absorptionsvorrat, kein Ruestungssystem.

Automatische Ausloeser:

- Alle fuenf Quallenarten: Koerperkontakt erneuert 10 Sekunden Poison.
- Ofenrauch: eigenstaendige Exposition zeigt Poison bis zum Verlassen;
  unveraendert 2 HP/s, keine doppelte Poison-DOT. Zeitliches Quallengift bleibt.
- Erfolgreich schlafende Survival-Teilnehmer: 5 Sekunden Regeneration.
  Abbruch und Nichtschlaefer erhalten nichts. Lebensregeln und Effektzeiten
  laufen auch waehrend der Wartezeit auf das Multiplayer-Schlafquorum.

Die anderen Effekte koennen berechtigt mit `/effect give <id> [Sekunden] [Level]`
und `/effect clear [id]` benutzt werden; Konsole `as <Spieler> effect ...`.
Keine neuen natuerlichen Waffen-/Klima-/Verrottungsausloeser behauptet.
Bestehende Nahrungsregeneration behaelt ihren fractional rate; kombiniert mit
Status-Regeneration gilt das Maximum statt doppelter Heilung. Tatsaechlicher
Schaden wird vor der Heilung gemeldet, nicht aus einem Netto-HP-Delta erraten.

## Replikation und Persistenz

Vorhandene Stats-Replikation uebertraegt aktive Effekte, Level, Restzeit,
Rauchexposition und Schild. Der Protokollstand ist 56; ein kanonischer,
gecachter Katalog-Fingerprint verhindert ungleiche Bewegungs-/Effektdefinitionen
zwischen Client und Server. Kein neuer Inhaltstransfer oder Live-Reload.

Neue PLY5-Spielerdateien behalten alle bestehenden Stats/Inventare. Explizite
PLY4/PLY3/PLY2/PLYR-Leser erhalten alte Daten. Rauch wird nur im Speicherabbild
entfernt und nach Laden neu abgetastet. Timer und Rest-Schild bleiben bestehen;
bewusst abgesenkte Definitionen begrenzen beim Laden Level/Dauer/Charge nach
unten. Welt-/Regionsformat, Generator und Block-IDs bleiben unveraendert.

## HUD und Ressourcen

Das vorhandene Herz-/Stamina-HUD bleibt verschiebbar und sichtbar unter
Inventaren. Status-Cards enthalten Symbol, Staerke und gerundete Restzeit;
bei Rauch steht `--:--` mit Expositionshinweis. Bis zu zwei Cards werden breit
dargestellt; bei mehr Effekten/kleinen Fenstern kompakt, maximal drei sichtbar
plus Ueberlaufanzeige mit weiteren Namen/Zeiten im Tooltip.

Gift und Frost werden auf das bestehende Herz-/HP-UI-Material aufgetragen;
Feuer, Blutung und Rotten erhalten dort dezente passende Farbakzente.
Keine neuen Fullscreen-Passes, dynamischen Lichter oder Welt-Meshes.
Shield Power zeigt einen Balken fuer tatsaechlich verbleibende Absorption.
Regeneration besitzt ein Icon mit genau zwei pinken Herzen; vier feste
Sprites steigen eine Sekunde auf und blenden aus, statt staendig neue
Partikel-Entities zu spawnen. Nach Effektende werden die UI-Zweige entfernt.

Ein geteiltes transparentes PNG-Atlas mit 13 Symbolen wurde ueber das eingebaute
Bildwerkzeug generiert, nicht mit einem CLI-/API-Fallback. Pfad/Prompt/Zellordnung:
[`src-ui/assets/status-effects-atlas.txt`](../../../../src-ui/assets/status-effects-atlas.txt).
Kein Netzwerkzugriff zur Laufzeit, keine individuellen Materialien je Card.

## Validierung

Erfolgreich ausgefuehrt, mit `CARGO_BUILD_JOBS=2` fuer Kompilierungen:

- `cargo fmt --all --check`
- `cargo check --locked --workspace`
- `cargo test --locked --workspace`: 1778 bestanden, 2 bestehende ignoriert.
  Das sind 36 zusaetzliche Tests gegenueber Phase 103, einschliesslich realer
  UDP-Anwendung/Expiry, Katalog-Mismatch-Ablehnung, Bewegung, Schlaf, Smoke,
  Widerstaenden, Speichermigration, HUD und sicherer Help-Pagination.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo build --locked --workspace`
- `cargo test --locked -p tilt-ui-runtime --lib`: weitere 309 bestanden.
- `env -u DISPLAY -u WAYLAND_DISPLAY ... terra-server --smoke-test`:
  Headless-Start mit echtem UDP-Socket und 27 Sections bestanden.

Logs: [Tests](logs/phase104-tests.log), [Runtime](logs/phase104-tests-runtime.log),
[Check](logs/phase104-check.log), [Clippy](logs/phase104-clippy.log),
[Build](logs/phase104-build.log), [Fmt](logs/phase104-fmt.log),
[Headless](logs/phase104-headless.log).

### Native UI-/GPU-Pruefung

RX 6950 XT/RADV/Vulkan, echter integrierter Survival-Server ueber UDP, echte
administrative Chat-Commands und replizierte Vitals. Keine lokal gefaelschten
Effekt-Snapshots. Der Test nutzt bewusst `120 s` fuer Aufnahmen; die natuerlichen
10-s-/5-s-Ausloeser sind separat durch Server-/UDP-Regressionen geprueft.
Bei allen 13 gleichzeitig wird Regeneration III zuerst angewandt, damit sich
die Testperson waehrend der vom echten Rate-Limiter verlangsamten Eingabe
nicht selbst toetet. Einzelaufnahmen verwenden Stufe I.

`--smoke-test-effects` prueft Atlas-Handles und gueltige Zellrects, Materialflags,
Cards/Ueberlauf, Viewportgrenzen, begrenzte UI-Nodes und leere Zweige nach Clear.
Die kompakte Ansicht prueft explizit Abstand zur Block-Infokarte. Acht Bilder
erfolgreich aufgenommen, WGSL/Pipeline ohne ERROR oder Panic. Der erste
Teststart ohne Menu-Initialisierung wurde korrigiert; ebenso der bei der ersten
Sichtpruefung gefundene kleine Info-HUD-Ueberlapp im schmalen Fenster.

| Zustand | UI-Nodes | Logische Fenstergroesse | Physischer Viewport |
|---|---:|---|---|
| Poison | 112 | 1280 x 800 | 1920 x 1200 |
| Freeze | 118 | 1280 x 800 | 1920 x 1200 |
| Regeneration | 120 | 1280 x 800 | 1920 x 1200 |
| Alle 13, 3 Cards + 10 weitere | 154 | 1280 x 800 | 1920 x 1200 |
| Clear | 102 | 1280 x 800 | 1920 x 1200 |
| Alle 13 | 154 | 1920 x 1080 | 2880 x 1620 |
| Alle 13 kompakt | 154 | 360 x 800 | 540 x 1200 |
| Clear kompakt | 102 | 360 x 800 | 540 x 1200 |

Die 1.5-fache physische Aufloesung stammt vom nativen Monitor-Scale-Faktor.
Alle acht Ansichten sind im [nativen Log](logs/phase104-native-effects.log)
nachvollziehbar. Poison, Freeze, Regeneration, Alle/1920 und die korrigierte
kompakte Ansicht wurden zusaetzlich visuell inspiziert:
[Gift](native/status-poison-1280.png), [Eis](native/status-freeze-1280.png),
[Regeneration](native/status-regeneration-1280.png),
[Alle Effekte](native/status-all-1920.png), [Kompakt](native/status-all-compact.png).

### Grenzen

Die Shaderstrukturen sind eine budgetierte prozedurale Interpretation der
Referenzen, keine Pixelkopie. Der Titel des Effekts und volle Staerke bleiben
im Tooltip auch bei kompakten/zusammengefassten Cards verfuegbar.
Neue Wetter-, Rohkost-, Waffen- oder Feuer-Ausloeser sind nicht vorgetaeuscht.
Ruestung gehoert weiterhin zu F02; der neue Balken misst ausschliesslich
Shield-Power-Absorption. Kein Mod-Live-Reload oder Transfer beliebiger neuer
Effektarten. Matching JSONs erfordern einen Neustart beider Programme.

Es wurde kein isolierter neuer FPS-Vorher-/Nachhervergleich und kein
10-/15-/30-Minuten-Lauf ausgefuehrt. Die gemessene konstante Node-Anzahl und
Clear-Rueckkehr belegen den kurzen UI-Lifecycle, keine allgemeine GPU-/VRAM-
oder Langzeitparitaet. Es gibt keine neue Entity pro Partikel pro Frame.

## Veroeffentlichung

Fuer die Erstveroeffentlichung auf das vom Nutzer angegebene private
GitHub-Repository sind Build-Verzeichnisse, lokale Profile/Welten/Karten
und historische Benchmarkbilder absichtlich ignoriert, nicht geloescht.
Spielassets und aktuelle visuelle Nachweise bleiben versionierbar.
