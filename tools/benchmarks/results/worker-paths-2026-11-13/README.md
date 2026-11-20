# Phase 246: Handwerkerwege und erreichbare Erdarbeiten

Der aktuelle Spielstand `worlds/Test` enthält einen Handwerker bei
`[35.302784294, 16, 46.487119026]`. Sein erster Hausauftrag bei `[40,15,44]`
steht bei Schritt **6/907**. Das Replay des unveränderten Anfangszustands
reproduziert 30 Sekunden Stillstand bei dauerhaftem Status `Travelling`.
Die nächste Aushubzelle `[34,15,49]` liegt unter einer weiteren Erdschicht;
es existiert kein gültiger Arbeitsstandpunkt mit freier Interaktionssicht.

Die Korrektur umfasst:

- Oberflächennahe Aushubreihenfolge auch beim Fortsetzen gespeicherter Pläne.
- Erreichbare seitliche Aushubzellen innerhalb derselben kostenlosen Phase,
  wenn ein Steilrand die nächste obere Zelle verdeckt.
- Unterstützte Zwischenpositionen zwischen Zellmitten bei steilen Kanten;
  Reichweite, Sichtprüfung und vollständige Körperprüfung bleiben verbindlich.
- Getrennte Zeitgeber für Suchwiederholung und tatsächlichen Laufstillstand.
  Fehlende Wege zu Baustelle oder Lager werden korrekt angezeigt und erneut geprüft.
- Physisches Nachräumen ausschließlich bereits von demselben Auftrag ausgehobener
  Erdstellen vor der vorgesehenen Platzierung. Im Spielstand liegen sechs davon
  wieder voll. Die Ursache dieser vorherigen Veränderung ist nicht nachgewiesen.
  Neue Steinblöcke, Truhen und andere Objekte werden nicht ersetzt.
- Bereits extern geleerte Aushubzellen werden ohne Material- oder Arbeitszeitgewinn
  übersprungen; bezahlte Platzierungen behalten ihre normale Prüfung.

Kein Netzwerk- oder Speicherformat ändert sich. Fertige Schritte, Materialkosten
und Creative-Finanzierung werden nicht zurückgesetzt.

## Verfahren

Das Diagnoseprogramm `apps/server/examples/inspect_worker_paths.rs` lädt den
Spielstand und erzeugt dessen Build-Sections mit gespeicherten Overrides.
Es führt die echte Konstruktion mit 10 Hz und normalen Kollisions-/Reichweiten-
prüfungen aus. Nur angeforderte, berechtigte Türen werden geöffnet.
Weltdateien werden nicht geschrieben. Die endgültige Prüfung verwendet die
isolierte Kopie `target/worker-paths-source/Test`.

Das ist ein CPU-Replay der tatsächlichen Baudaten und Handwerkerbewegung,
kein erneuter Grafik- oder vollständiger Serverlauf: andere Kreaturen bleiben
an ihren gespeicherten Positionen. Ihr Kollisionsschutz bleibt aktiv.
Ein separater Headless-Test prüft den Serverstart.

```sh
cargo run --locked --offline -p terra-server --example inspect_worker_paths -- \
  target/worker-paths-source/Test 205294890893140149 8400 2
```

Die letzten beiden Argumente sind die maximale simulierte Dauer und die
Auftrags-ID, deren vollständiger Abschluss verlangt wird. Fehlerhafte oder
unvollständige Aufträge liefern einen Fehlercode. Reale Arbeitszeit wird pro
Tick fortgeschrieben; die Prüfung verwendet keinen beschleunigten Baufortschritt.

[Unkorrigierter Anfangslauf](before.txt),
[Prüfsummenvergleich der acht Originaldateien](source-world-check.json).

## Ergebnis

Beide ursprünglichen Aufträge werden vollständig abgeschlossen, mit derselben
Arbeitskraft und unverändertem Anfangsspielstand:

| Auftrag | Anfang | Abschluss erstmals beobachtet | Endzustand |
| --- | --- | --- | --- |
| 1, automatisch bei `[40,15,44]` | 6/907, Unterwegs | 3660 simulierte Sekunden | 907/907, Complete |
| 2, manuell bei `[3,16,19]` | 0/832, keine freie Arbeitskraft | 7300 simulierte Sekunden | 832/832, Complete |

Die Beobachtung erfolgt im Zehn-Sekunden-Abstand. Der Handwerker wechselt nach
dem ersten Abschluss automatisch zur zweiten Baustelle. Laufwege und zusätzliche
Erdarbeiten kommen zur aktiven Bauzeit hinzu. Die Simulation läuft bis 8400 s;
ein früherer 7200-s-Lauf erreichte beim zweiten Haus bereits den Gerüstabbau,
aber noch nicht seinen Abschluss. Dies war eine zu kurze Prüfdauer.

- **1399 Welt-Tests bestehen**, darunter 60 Bautests. Fünf vorhandene opt-in
  CPU-Mikrobenchmarks bleiben ignoriert.
- **Zwei Headless-Integrationstests bestehen**: 27 Sections ohne Renderer,
  regulärer App-Exit sowie Belegen und Freigeben eines echten Loopback-Sockets.
- `cargo fmt --all --check`, `cargo check --workspace --all-targets` und
  `cargo clippy --workspace --all-targets -- -D warnings` bestehen.
- Client und Server wurden unabhängig gebaut, jeweils mit `--locked --offline`.
- Die acht Original-Speicherdateien behalten ihre Prüfsummen. Der Session-Lock
  wird vom Vergleich ausgenommen.

[Replay-Protokoll](after.txt), [strukturierte Ergebnisse](outcome.json),
[Welt-Tests](world-tests.txt), [Headless-Tests](headless.txt),
[Client-Build](client-build.txt), [Server-Build](server-build.txt),
[Quelltextprüfsummen](source-hashes.json), [Workspace-Check](workspace-check.txt),
[Clippy](clippy.txt), [Exit-Codes der Abnahme](validation.json).
