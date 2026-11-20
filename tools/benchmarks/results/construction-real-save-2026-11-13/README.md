# Phase 244: Hausplatzierung im echten Spielstand Test

Der Fehler wurde mit den wirklichen Generatorparametern und Blockänderungen aus
`worlds/Test` reproduziert. Der vorige native Lauf auf einer völlig flachen Welt
hatte diesen Fall nicht erfasst. Im Original gab es keine gespeicherten Bauaufträge.

Bei Anker `[16,16,24]` blockierten gespeicherte Erde-/Graswerte unter dem künftigen
Haus, beispielsweise Erde an `[10,15,19]`. Solche Journalwerte entstehen auch durch
Graswachstum und das Abdecken von Gras durch andere Blöcke. Bei `[40,13,32]` scheiterte
die freie Wiese an der bisherigen Begrenzung auf eine einzige natürliche Stufe.

Beide Stellen werden jetzt akzeptiert. Gespeicherte Erde/Gras sind ausschließlich
unter der Fußebene vorbereitbarer Baugrund; außerhalb der tatsächlichen Floor-/
Foundation-Zellen bleiben sie erhalten. Unverändertes natürliches Terrain kann
bis zu vier Blöcke hoch durch persistierte Arbeitsschritte eingeebnet werden.
Planung und wirklicher Bau verwenden dieselbe Regel. Andere gespeicherte Blöcke,
oberirdische Erd-Edits, Bäume, Objekte, Truhen und Wasser bleiben geschützt.
Unbekannte Sections, stale Overrides und unzureichende Gründung werden abgewiesen.

Die native Abnahme startet **eine Kopie des echten Spielstands und Profils**.
Sie benutzt den gespeicherten Spieler bei `[8.955101,27.694769,39.11475]` und das
vorhandene Königreich Test. TAB, die wirkliche HTML-Vorlagenkarte, Serverpreview
und ein projizierter Mausklick bei `[16.5,16.005,24.5]` erzeugen einen autoritativ
bestätigten Dorfwohnhaus-Auftrag bei `[16,16,24]`. Ein bestätigter Save und erneutes
Lesen der Datei prüfen Eigentümer, Vorlage, Position und Creative-Finanzierung.
Die Prüfsummen sämtlicher Original-Speicherdateien bleiben unverändert.

- [Orange Vorschau in der echten Testwelt](saved-construction-preview.png)
- [Bestätigter Bauauftrag](saved-construction-queued.png)
- [Native Prüfwerte](native.json), [Client-Log](native.txt), [gespeicherte Aufträge](native-saved-state.txt)
- [Original-Prüfsummen](save-source.json), [Quellstand](source-sha256.json), [Runner-Log](native-runner.txt)
- [Vorherige Ablehnungen](before-fix.txt), [Erd-Pad nach Fix](site-pad.txt), [Wiese nach Fix](site-terrace.txt)
- [Truhen geschützt](protected-chests.txt), [Bäume geschützt](protected-trees.txt), [fehlende Gründung](unsupported.txt)
- [1.381 Welttests](world-tests.txt), [Format](fmt.txt), [Workspace-Check](check.txt), [striktes Clippy](clippy.txt)
- [Client-Build](client-build.txt), [Server-Build](server-build.txt), [Headless-Start mit 27 Sections](headless.txt)

Sieben neue Regressionen prüfen echte bezahlte bzw. Creative-Bauabläufe bis zum
fertigen Haus, Materialbilanz, vier Ausrichtungen, persistierte Vorbereitung,
unveränderten Boden im Arbeitsring und spätere blockierende Stein-Edits ohne
Materialverlust. Alle 1.381 Welttests bestehen; fünf bestehende Tests sind ignoriert.
Check und Clippy prüfen den gesamten Workspace einschließlich aller Targets.

In der laufenden Kopie plant die Wohngebietsautomatik zusätzlich ein Haus bei
`[40,15,44]`. Der einzige vorhandene Handwerker ist diesem Auftrag zugeteilt;
das manuell bestätigte zweite Haus wartet zunächst auf einen freien Handwerker.
Das ist von der erfolgreichen Vorlagenplatzierung getrennt. Die Originalwelt
wurde durch die Abnahme nicht um diesen Auftrag ergänzt.

Wiederholen:

```sh
cargo fmt --all --check
cargo check --locked --offline --workspace --all-targets
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
cargo test --locked --offline -p terra-world --features bevy -- --test-threads=4
cargo build --locked --offline -p terra-client
cargo build --locked --offline -p terra-server
bash tools/benchmarks/run-saved-construction.sh worlds/Test configs/profile.toml
```

Der letzte Lauf benötigt eine Grafiksession und genau diesen gespeicherten
Regressionfall. Der Runner kopiert Save und Profil in eine neue Mappe unter
`target/saved-construction/`. Der Smoke verweigert den Original-Weltordner.
Für andere Anker existiert die schreibgeschützte CLI:

```sh
cargo run --locked --offline -p terra-server --example inspect_building_world -- worlds/Test 205294890893140149 16 16 24
```

Die CLI nimmt optional Ausrichtung und Vorlagen-ID entgegen. Protokoll 141,
Speicherindex ID26, Vorlagenschema 1 und Abhängigkeiten bleiben unverändert.
