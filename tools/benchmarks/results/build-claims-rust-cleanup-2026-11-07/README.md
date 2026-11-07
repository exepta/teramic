# 203 — Build-Anschluss, Rust-Lesbarkeit und Fluent-TODO

Auftrag vom 07.11.2026. [Bildreferenz](../../../../docs/references/build-claims-must-connect.png).

## Claim-Regeln

Mit der standardmäßig aktiven `require_adjacent_claims`-Regel müssen alle neuen
Build-Chunks über kantenangrenzende **Build-Chunks** mit dem Bergfried-Chunk ihrer
Siedlung verbunden sein. Diagonalen, Territorium und das Land anderer Siedlungen
ersetzen diesen Build-Pfad nicht. Territorium darf weiterhin am gesamten eigenen
zusammenhängenden Land andocken. Einen Radius zum Bergfried gibt es nicht.

Ein gemeinsamer Vorher-/Nachher-Graphvergleich prüft die endgültige Transaktion:
einzelne Claims, gemischte Batchauswahl, Typwechsel und Freigaben. Abgelehnte
Änderungen hinterlassen weder Claims noch Teilrevisionen. Eine Build-Brücke darf
nur ersetzt werden, wenn der endgültige Batch einen anderen vollständigen
Build-Pfad enthält. Die Legacy-APIs verwenden dieselbe Standardprüfung.

Vorhandene historische Inseln bleiben lesbar und können entfernt oder durch einen
Build-Pfad wieder angeschlossen werden. Sie dürfen nicht erweitert werden; bereits
angeschlossene Build-Flächen dürfen ihre Verbindung nicht verlieren. Der explizite
Server-/Welt-Opt-out `require_adjacent_claims=false` deaktiviert beide Graphprüfungen.
Das Speicherformat und die Netzwerkschnittstelle bleiben unverändert.

Die Kingdom-UI verwendet getrennte Core-Probe-Transaktionen für Build und Territory.
Ihr Cache berücksichtigt Registry-/SettlementMap-Revisionen, Regeln, Akteur,
Zielchunk und Limits. Dadurch stimmen die beiden Buttons mit der Autorität überein,
ohne pro Frame erneut die gesamte Registry zu kopieren. Hinweise erklären den
Build-Pfad und den bewussten Opt-out. Parent-Territory-Überlagerungen für Dörfer und
Städte bleiben benutzbar.

## Dokumentation und Namen

1.104 Rust-Dateien wurden nach Bereichen vollständig geprüft, einschließlich
`src-ui`, Werkzeugen und archivierten Vergleichsquellen. Die 1.780 vorhandenen
Rustdoc-Zeilen waren bereits Englisch; die zusätzliche Sprach-/Unicodeprüfung
findet keine deutschen Rustdoc-Kandidaten. Zusammengeschriebene Formulierungen
wurden verbessert. Der Umfang ist in `rust-audit.json` dokumentiert.

Unklare längerlebige Aliase wurden unter anderem im Baumwachstum, Tree-Registry,
Flussnachweis, Bewohnerverhalten, Physik, Netzwerkteilpaketen, Modellbau, Vorschau,
Sessionstart und der Siedlungsverwaltung präzisiert. Konventionelle mathematische
Koordinaten/Indizes bleiben erhalten. Spieltexte, Asset-IDs, öffentliche API-Namen
und Serde-Schlüssel wurden durch den Lesbarkeitsteil nicht geändert. Historische
Benchmark-Quellen behalten ihre ursprünglichen Bezeichner als Vergleichsdaten.

[TODO.md](../../../../TODO.md) enthält **F72 Sprachsystem mit Fluent**: die vorhandene
TiltUI-Unterstützung nutzen, Deutsch/Englisch, Fallback, Platzhalter, Plural-/
Zahlformate, Profilwahl und Mod-Inhalte konsistent anbinden. Das Sprachsystem ist
als neues offenes Feature erfasst.

## Prüfung und Veröffentlichung

Keine Grafiktests, Spielclients, Screenshots oder GPU-Galerien werden gestartet.
Cargo verwendet einen Job, vier Testthreads, effektive Nice-Priorität 19 und
IO-Klasse 2 / Priorität 7; DISPLAY und WAYLAND_DISPLAY sind entfernt.

4.111 Workspace-Library-/Bin-Tests bestehen, keiner ist fehlgeschlagen; neun
bestehende Tests bleiben ignoriert. Die zwölf Testsuiten laufen mit
`cargo test --workspace --lib --bins --locked --no-fail-fast -j1 -- --test-threads=4`.

- Clippy aller Targets mit `-D warnings`: bestanden.
- Formatprüfung und Diff-Whitespace: bestanden.
- Alle 60 tatsächlichen UI-Stylesheets mit dem TiltUI-Parser: bestanden.
- Abschließender Build der aktuellen Server-/Clientdateien: bestanden (2m 49s).

Zehn neue Corefälle prüfen Territory-/Diagonal-Bypass, atomare Mischbatches,
Build-Brücken bei Typwechsel und Release, alternative Build-Korridore, historische
Inseln samt Reparatur, expliziten Opt-out, den eigenen Dorf-Seat und Legacy-Wege.
Ein echter UDP-Fall prüft Bestätigungen und unveränderte Revisionen bei Ablehnung,
Live-Regelwechsel und einen 320-Meter-Build-Pfad. Drei zusätzliche UI-Fälle prüfen
die getrennten Buttons, Cache-Neuberechnung und Parent-Territory-Überlagerungen.
Quell- und Loghashes stehen nach dem Build in `verification.json`.

Der bestehende lokale und entfernte Branch `main` startete bei `d2b7956`.
Der anschließende Commit umfasst den gesamten nicht ignorierten Projektstand,
einschließlich der bereits zuvor vorhandenen Änderungen. Spielstände, Profile und
lokale Builddateien bleiben durch die vorhandenen Git-Regeln ausgeschlossen.
GitHub wird über den vorhandenen SSH-Zugang angesprochen; der separate lokale
TiltUI-Checkout bleibt eine in der Projekt-README dokumentierte Voraussetzung.

Git-Attribute kennzeichnen Binärassets ausdrücklich als binär, damit Modellbuffer
weder als Textdiff erscheinen noch Zeilenendkonvertierungen erhalten. Alle 264
vorgemerkten regulären Binärassets wurden gegen die exakten Arbeitskopiebytes geprüft.
Unveränderte historische Profilingausgaben und Unified-Patches behalten ihre
aufgezeichneten Leerzeichen als Daten.
