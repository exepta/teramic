# TERAMIC Menu UI - 15.10.2026

## Implementierung

Der normale Client startet im Main Menu, ohne Host und ohne Terrain-Streaming.
`terra-ui::menu` verwaltet Main/Worlds/Servers/Options/Pause/Loading/Playing sowie
modale Dialoge. Der bestehende PlayerInputBlock und CaptureRequest sperren
Gameplay und geben die Maus frei. Die vorhandene Ingame-HUD bleibt erhalten.

TiltUI-Komponenten liegen unter `src-ui/pages/` und `src-ui/components/`:
Main Menu, Singleplayer, Multiplayer, Options, menu-button, world-card,
server-card, settings-row, modal-dialog und menu-background. HTML/CSS-Bindings,
html_shared, UiStore, html_fn und der bestehende TiltUI-Buildprozess werden
verwendet. Keine zweite UI-Bibliothek, keine manuell gespawnte produktive UI.
Bevy-UiScale wird ausschliesslich fuer die von TiltUI benutzte UI-Pipeline gesetzt.

Die lokale TiltUI-0.1-API wurde am installierten Quellcode geprueft: Headlines
haben den CSS-Tag `headline`; Component-CSS hat eigene Scopes. Wiederholte
Commands verwenden einen echten UiStore statt nicht unterstuetzter dynamischer
Element-IDs. ChoiceBox-Optionen sind statische Template-Kinder, da dynamische
For-Kinder in dieser Version nicht korrekt in das Popup umgehaengt werden.
Nicht vordefinierte Aufloesungen/FPS zeigen den geladenen aktuellen Wert.

## Einstellungen

- `configs/graphics.toml`: Aufloesung in physischen Pixeln, Fenstermodus, VSync,
  FPS-Limit, Sichtweite, LOD-Faktor, Schatten, Wasser, Vegetationsdichte,
  Belichtung, UI Scale und FOV; bisherige Wasser-/Wetteroptionen bleiben erhalten.
- `configs/controls.toml`: typisierte KeyCode-/MouseButton-Bindings aller 13
  vorhandenen Aktionen und Maus-Sensitivitaet. Dieselbe GameplayInput-Abstraktion
  versorgt Controller, Kamera, Cursor, Block-/Prop-Abbau und Platzierung.
- `configs/servers.toml`: gespeicherte Namen, Hostnamen/IPs, Ports und private
  Token-Dateipfade, keine Tokens oder Serversignierschluessel im TOML.

Der vorhandene Config-Trait und atomare TOML-Writer wurden erweitert, nicht
dupliziert. Ungueltige Dateien bleiben erhalten; Fehler sind im Menue sichtbar.
Apply speichert/aktiviert; Reset betrifft nur den Entwurf des aktuellen Tabs.
Back fragt bei Aenderungen nach. Display-Vorschauen laufen 15 echte Sekunden
und persistieren erst nach Keep. Ein Schreibfehler bei controls.toml versucht
auch graphics.toml auf den vorherigen Stand zurueckzusetzen. Die zwei Dateien
sind keine absturzsichere gemeinsame Dateisystem-Transaktion.

Exklusives Vollbild waehlt einen echten VideoMode des aktuellen Monitors und
lehnt unmoegliche Aufloesungen ab; Borderless benutzt den Desktop-Modus und
deaktiviert deshalb die Aufloesungsauswahl mit dem aktuellen Wert Desktop.
Schatten aendern Maps/Cascades/Enabled, Brightness die reale Kamerabelichtung.
Vegetationsdichte filtert stabil bestehende Prop-IDs in asynchronen Section-
Batches und beim Picking, ohne Prop-Zustaende zu entfernen. Der Server kennt
die jeweilige Dichte; deshalb ist der Hello-Protokollstand jetzt 2. Client und
Dedicated Server gemeinsam aktualisieren. Generatorversion, Welt-IDs, Block-IDs
und Seeds werden nicht migriert oder neu interpretiert.

## Welten Und Verbindungen

`terra-storage::WorldLibrary` erweitert WorldManifest um Titel, Version, Modus
und Last Played. `worlds/<id>/world.toml` wird wirklich erstellt/gelesen/erst nach
Bestaetigung geloescht. Inkompatible Eintraege bleiben sichtbar, aber sind nicht
spielbar; defekte Eintraege liefern Fehler. Vollstaendige u64-Seeds und Prop-IDs
werden getestet. Symlink-Weltverzeichnisse und abweichende Manifest-IDs werden
abgelehnt. `TERRA_WORLD_DIR` / `TERRA_MENU_CONFIG_DIR` erlauben eigene Pfade.

`edits.toml` ist eine Speicherung des bestehenden SectionDelta-Journals fuer
Block-Edits, Prop-Entfernungen und Bodenzustaende. IntegratedServer laedt vor dem
ersten Tick, speichert alle zehn Sekunden und beim Stop. Start/Stop und DNS/
Token-Lesen laufen ausserhalb des Rendering-Hauptthreads. Beim Verlassen werden
ausstehende Edit-Antworten bis zu fuenf Sekunden abgewartet. Ein Fehler beim
abschliessenden Speichern wird sichtbar statt als erfolgreicher Exit gemeldet.

Der Serverbrowser benutzt den vorhandenen privaten 2048-Byte-Netcode-Token.
Der gespeicherte Endpunkt muss zum Token passen; Join startet keinen lokalen
Host. Connecting, World Synchronization und Connection Failed stammen aus dem
echten ClientSession-Zustand. Keine simulierten erfolgreichen Verbindungen.

## Validierung

Erfolgreich: `cargo fmt --all --check`, `cargo check --locked --workspace`,
`cargo test --locked --workspace -j 2` (212 Tests),
`cargo clippy --locked --workspace --all-targets -- -D warnings` und
`cargo build --locked --workspace -j 2`. Separater Dedicated-Build erfolgreich;
Start ohne Display beendet den Headless-Smoke mit 27 generierten Sections.
Der separate Feature-Baum enthaelt kein bevy_render, bevy_winit, wgpu oder TiltUI.
[Abschlussnachweis und genaue Testbedingungen](VALIDATION.md).
Die erweiterten Unit-/Integrationstests pruefen Welt-CRUD, Journal-Roundtrip,
Versions-/ID-Fehler, TOML-Reload, Conflicts, Escape, Maus-Rebinding, Drafts,
Schreibfehler-Rollback, Display-Timeout, echte Monitor-Modi, Schatten/Belichtung
und stabile Vegetationsdichte. Abschlussstatus und Logs liegen in diesem Ordner.

Native Vulkan-Pruefung auf AMD RX 6950 XT / RADV Mesa 26.2.4-arch3.1:

- `--smoke-test-menu`: echte TiltUI-Eventhandler, alle vier Seiten, Create/Delete,
  Add/Edit, Validierungsfehler, Key Capture, Konflikt, Escape, Apply, Verwerfen
  und echter 15-Sekunden-Display-Timeout. Keine produktiven Testwelten.
- `--smoke-test-menu-small`: dieselbe Suite bei 960x640 mit benutzerdefiniertem
  75-FPS-Wert; native Screenshots unter `small/`.
- `--smoke-test-menu-pointer`: injiziert Bevy-PointerInput an den real berechneten
  UI-Koordinaten und prueft Hit-Testing, Press/Release, Singleplayer, Back,
  Options, Controls und den produktiven Quit-Pfad. Das ist keine manuelle
  Bedienung mit physischer Maus und kein direkter Aufruf der HTML-Handler.
- `--smoke-test-menu-session`: Play World, echter Integrated-Handshake,
  Bewegung mit neu belegtem ArrowUp, bestaetigter Edit, Leave, finaler Save,
  erneuter Hoststart und wiederhergestelltes Journal; Bilder unter `session/`.
- `--smoke-test-menu-join`: gespeicherten Server mit frischem privaten Token
  betreten, echter Dedicated-Handshake/Replica, kein Integrated-Host und sauberes
  Leave; Bilder unter `join/`. Dedicated lief ohne DISPLAY/WAYLAND_DISPLAY.
- Derselbe Join-Test mit `TERRA_MENU_EXPECT_CONNECTION_FAILURE=1` und Token fuer
  den zuvor gestoppten Server wartet auf den echten Timeout und auf den
  tatsaechlichen TiltUI-Fehlertext, nimmt ihn auf und kehrt ins Main Menu zurueck.
  Die bestehende Client-Synchronisationsfrist betraegt 30 Sekunden.

Die Screenshots sind native Client-Aufnahmen, keine UI-Mockups. Gepruefte
Ansichten: `session/main-menu.png`, `session/world-selected.png`,
`session/create-world.png`, `session/graphics.png`, `session/controls.png`,
`join/server-selected.png`, `join/server-error.png`, `failure/connection-failed.png`, `pointer/pointer-options.png`,
`pointer/pointer-controls.png`, `small/main-menu.png`, `small/graphics.png`,
`small/create-world.png` und `small/delete-world.png` wurden bildlich inspiziert.
1280x800 und 960x640 wurden geprueft, einschliesslich Aufloesungsvorschau bei
1280x720. Vollbild-Moduswahl wurde mit Bevy-Monitor-Komponenten automatisiert
getestet, nicht auf jeder Aufloesung/Hardware manuell durchgeschaltet.
Keine FPS-/GPU-Beschleunigung wird aus Menue-Smokes abgeleitet.
Ausgewaehlte Screenshots sind archiviert; redundante Zwischenaufnahmen wurden
entfernt. Das aktuelle Hauptmenue zeigt `pointer/main-menu.png`.
Der abschliessende komplette Menue-Smoke war ebenfalls erfolgreich; seine
aktuelle Serverkarte mit Description/Spieler-Maximum als Unavailable/Unknown
wurde unter `final-ui/server-selected.png` erneut bildlich geprueft.

## Konkrete Grenzen

- Das vorhandene Protokoll hat keine oeffentliche Status-/Ping-/Beschreibungs-
  Abfrage. Spielerzahl/Ping/Online-Status bleiben daher Unknown/Unavailable.
  Integrationspunkt: neuer authentifizierter/oeffentlicher Statusdienst neben
  ServerRuntime; UDP-Socket-Erreichbarkeit allein ist kein Online-Nachweis.
- Tokens muessen serverseitig ausgegeben werden, gelten eine Stunde und nur fuer
  den laufenden Server. Kein Login-/Account-/Discovery-Dienst oder Live-Token-UI.
- Dedicated hat noch keinen konfigurierten Save-Pfad. Der gemeinsame Speicher-
  adapter existiert, wird bisher nur von der Weltbibliothek des Integrated-Hosts
  eingebunden. Spielerpositionen/Inventar werden noch nicht gespeichert.
- Es gibt noch keinen Mehrprozess-Save-Lock: dieselbe Singleplayer-Welt nicht
  gleichzeitig in mehreren Clients oeffnen. Fuer gemeinsames Spielen Dedicated
  verwenden, nicht zwei getrennte integrierte Hosts mit derselben Save-Datei.
- Autosave serialisiert das Journal auf dem separaten Serverthread. Sehr grosse
  Journale und der Einfluss auf Server-Tick-Latenzen sind nicht lastgetestet.
  Unbestaetigte Edits nach dem fuenfsekundigen Close-Timeout sind nicht garantiert.
- Pause sperrt lokale Eingaben, stoppt aber nicht den gemeinsamen Server-Takt.
- Inventory, Interact, Lauf-Crouch und Kameraperspektivwechsel haben kein
  fertiges Gameplay-Backend und sind sichtbar deaktiviert. Survival/Creative
  steuern bisher vor allem die vorhandene Flug-/Werkzeugfreigabe.
- Linux/Vulkan wurde nativ getestet; andere Betriebssysteme und mehrere physische
  Monitore wurden nicht visuell validiert. Kein Internet-/Mehrrechnernachweis.

## Hintergrund-Asset

`src-ui/assets/menu-valley.png` wurde mit dem eingebauten `image_gen`-Tool
generiert, nicht mit CLI/API-Fallback. Es ist eigenstaendige Menue-Illustration,
kein Screenshot oder Versprechen der aktuellen Terrain-Renderqualitaet.
Verwendetes Motiv/Prompt: original wide fantasy voxel valley; terraced mossy
grass and small flowers, turquoise creek with visible stones, leafy forest with
warm autumn accents and distant mountains; warm soft sunlight, quiet central
composition for live menu controls; no UI, text, logos or characters; no copied
Minecraft branding. Die Referenzbilder dienten ausschliesslich als Stilvorlage.
Die ausgewaehlte Tool-Ausgabe wurde ins Repository kopiert; sie wird nicht aus
einem lokalen Codex-Cache oder temporaeren Pfad geladen.
