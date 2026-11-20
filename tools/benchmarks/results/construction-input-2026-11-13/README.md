# Phase 245: Bau-Eingabe und konkrete Ablehnungsdiagnose

Auf `House.png` fehlen HUD-Koordinaten und die Rückmeldung nach dem Klick. Der
exakte Anker ist deshalb **noch nicht bestätigt**. Die Bildgeometrie legt den
Bereich um X4/Y16/Z20–22 nahe. Die vorhandene Testwelt wird dort von der
autoritativen Planung akzeptiert. Diese Abnahme behauptet daher keine sicher
ermittelte Ursache für die unbekannte exakte Bildstelle.

Zwei Eingabefehler sind unabhängig davon reproduziert und korrigiert:

- Versteckte UI-Flächen behielten ihre Layout-Rechtecke und konnten Bauklicks
  abfangen. Der Picker berücksichtigt nun lokale und geerbte Sichtbarkeit.
- Beim Baumodus-Einstieg kann ein verspäteter Bergfried-Resume in PreUpdate
  `CaptureRequest(true)` senden, bevor der Baumodus in Update den Inputblock
  erneuert. Die Kamerasteuerung läuft dazwischen und konnte den Cursor einen
  Frame sperren. Aktive Konstruktion besitzt jetzt ausdrücklich den freien,
  sichtbaren Cursor und verwirft solche ausstehenden Anforderungen. Auch ein
  widersprüchlicher Fensterzustand wird repariert.

**Eingabe/Enter** und die neue Schaltfläche **Vorlage platzieren · Eingabe**
bestätigen die angezeigte Position. Mausklick, Taste und HTML-Handler verwenden
eine gemeinsame Request-Funktion. Eine laufende Anfrage sperrt weitere Aufträge.
Der zuletzt gewählte Ort bleibt beim Bewegen zur Schaltfläche erhalten. Die
orange Vorschau ist weiterhin eine Vorschau; die Freigabe erfolgt durch den Server.

Der Planner besitzt jetzt eine lokale `BuildSiteIssue`-Diagnose für den ersten
wirklichen Fehler. Legacy-API und detaillierte API verwenden denselben Codepfad.
Die CLI zeigt Ursache, Koordinate, geladenes/gespeichertes Material oder den
reservierenden Auftrag; QueueHouse protokolliert diese Angaben bei Ablehnung.
Die Bauplatz-Schutzregeln, Netzwerkpakete und Speicherformate bleiben erhalten.

## Abnahme

823 Tests bestehen: 44 Bau-/Diagnosetests, 657 UI-Tests und 122 Spielerkamera-/
Eingabetests. Der neue Cursor-Test aktiviert den normalen `accept_input=true`-
Pfad und prüft ausstehende Panel-Resume-Anforderungen, echte Fensterkomponenten,
Maus-/Tasten-/Motion-Messages, unveränderte Kamerapose sowie sauberen Exit und
anschließenden expliziten Capture. Ein Hierarchie-Test prüft verborgene und
explizit sichtbare UI-Kinder. Format, Check und striktes Clippy aller Workspace-
Targets, getrennte Builds und ein Headless-Start mit 27 Sections bestehen.

Drei native Läufe starten jeweils eine frische Kopie des **aktuellen** Spielstands
`worlds/Test` und des gespeicherten Profils:

| Bestätigung | Ort | Ergebnis |
| --- | --- | --- |
| OS-Maus auf Gelände, Linux uinput/KWin | `[4,16,22]` | Auftrag bestätigt, Save/Reload bestanden |
| OS-Maus auf die tatsächliche UI-Schaltfläche, Linux uinput/KWin | `[4,16,22]` | Auftrag bestätigt, Save/Reload bestanden |
| Eingabetaste als KeyboardInput | `[4,16,20]` | Auftrag bestätigt, Save/Reload bestanden |

Die Mausrouten benutzen den vorhandenen compositorabhängigen OS-Eingabehelfer.
Die Tastaturroute benutzt Bevy-KeyboardInput. Alle prüfen wirkliche Server-
Antworten, Eigentümer, Vorlage, Position, Ausrichtung und den gespeicherten Auftrag.
Alle Original-Speicherdateien behalten ihre Prüfsummen. Die ausgewählten Aufnahmen
wurden visuell geprüft. Die Vorlagenauswahl läuft über den vorhandenen HTML-Handler.

- [Vorschau mit Bestätigung](button-preview.png), [Bestätigter Schaltflächen-Auftrag](button-queued.png), [OS-Geländeklick](os-mouse-queued.png)
- [Schaltfläche: Prüfwerte](native-button.json), [Log](native-button.txt)
- [OS-Maus: Prüfwerte](native-os-mouse.json), [Log](native-os-mouse.txt)
- [Eingabetaste: Prüfwerte](native-enter.json), [Log](native-enter.txt)
- [Gesamtwerte](validation.json), [Quellstand](source-sha256.json)
- [Bautests](world-tests.txt), [UI-Tests](ui-tests.txt), [Kameratests](player-tests.txt), [aktualisierter Stylesheet-Parser](styles-after-button.txt)
- [Format](fmt.txt), [Check](check.txt), [Clippy](clippy.txt), [Client-Build](client-build.txt), [Server-Build](server-build.txt), [Headless](headless.txt)

## Tatsächlich belegte Nachbarstellen

Die schreibgeschützte Prüfung der aktuellen Welt zeigt:

- `[4,16,22]`: [akzeptiert](site-near-image.txt).
- `[4,16,28]`: [Truhe an 5/16/33](site-chest-edge.txt) im benötigten Arbeits-/Gerüstrand.
- `[40,15,44]`: [bereits durch Auftrag 1 reserviert](site-existing-job.txt).

Dies sind konkrete Ergebnisse dieser Anker, keine aus dem Bild abgeleiteten
Fehlermeldungen. Die genaue Benutzerstelle benötigt weiterhin ihre HUD-Koordinate.

Wiederholen nach dem Client-Build:

```sh
cargo test --locked --offline -p terra-world --features bevy building_jobs::tests
cargo test --locked --offline -p terra-ui --lib
cargo test --locked --offline -p terra-player --lib
TERRA_NATIVE_OS_POINTER=1 TERRA_SAVED_CONSTRUCTION_CONFIRM=os-mouse TERRA_SAVED_CONSTRUCTION_ORIGIN=4,16,22 bash tools/benchmarks/run-saved-construction.sh
TERRA_NATIVE_OS_POINTER=1 TERRA_SAVED_CONSTRUCTION_CONFIRM=button TERRA_SAVED_CONSTRUCTION_ORIGIN=4,16,22 bash tools/benchmarks/run-saved-construction.sh
TERRA_SAVED_CONSTRUCTION_CONFIRM=enter TERRA_SAVED_CONSTRUCTION_ORIGIN=4,16,20 bash tools/benchmarks/run-saved-construction.sh
cargo run --locked --offline -p terra-server --example inspect_building_world -- worlds/Test 205294890893140149 4 16 28
```

OS-Läufe benötigen die bestehende KDE/Wayland-Sitzung und Zugriff auf `/dev/uinput`.
Protokoll 141, Speicherindex ID26, Vorlagenschema 1 und Dependencies bleiben gleich.
