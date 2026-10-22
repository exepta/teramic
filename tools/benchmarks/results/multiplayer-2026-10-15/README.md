# Integrated / Dedicated Server: Validierung

Datum: 2026-10-15, Linux, native Bevy/Vulkan-Clients. Dies ist ein
Funktionsbericht, **kein isolierter Vorher-/Nachher-Performancevergleich**.
Dependency-Versionen, Welt-ID, Seed, Generatorversion und Block-IDs bleiben
unveraendert. UDP/Netcode aktiviert vorhandene Lightyear-Features und deren
kryptographische Transitivabhaengigkeiten.

## Architektur

Vorher war der normale Client ein lokaler World-/Controller-/Edit-Owner;
Lightyear registrierte nur Plugins und nicht verwendete Protokoll-Skizzen.
Jetzt startet der normale Client `IntegratedServer::start`: ein eigener
Headless-Bevy-App-Thread, gebunden an 127.0.0.1 mit freiem UDP-Port. Er verwendet
denselben Bootstrap und dieselbe Laufzeit wie das Dedicated-Binary. Client und
Server kommunizieren auch im Singleplayer ueber verschluesselte Netcode-Pakete.
Der Thread wird ueber sein besitzendes Resource beim Beenden gestoppt/gejoint.

Einzige Autoritaet ist der Server. `terra-world::WorldAccess` trennt Server,
Replikat und explizite Offline-Testfixtures. `Welcome` ersetzt die vollstaendige
Client-Weltkonfiguration vor dem Generierungsstart; `Ready` folgt auf die initialen
zugehoerigen Journal-Deltas. Das Basisterrain bleibt deterministisch lokal
generiert. Der Server haelt die Vereinigung aller Simulations-/Collider-Bereiche.

`terra-physics::character` enthaelt die gemeinsame Fixed-Tick-Kapselsimulation.
Der Client sagt eigene Eingaben voraus und korrigiert/replayed offene Eingaben
anhand bestaetigter Sequenzen. Eingaben sind Zustandsproben, kein Tickbudget:
ein Rueckstau wird zum neuesten Zustand mit einmaligem Sprungimpuls verdichtet.
Ohne neue Probe gilt eine Richtung hoechstens 250 ms. Snapshots kommen beim
Standardtakt mit 20 Hz und werden fuer andere Spieler interpoliert.

Block-/Prop-/kreative Bodenzustandskommandos sind zuverlaessig/geordnet.
Reichweite, Sichtstrahl, Revision, Material, Prop-ID, Koerperueberlappung und
Aktionsrate werden vor dem bestehenden gemeinsamen Commit geprueft. Ein
vollstaendiges `SectionDelta` ersetzt jeweils nur den bestaetigten Abschnitt
des vorhandenen Journals. Lokale Worker-Epochen bleiben unabhaengig von den
Revisionszaehlern des Servers. Props und Appearance besitzen eigene Epochen.
Weder Micro-Voxel noch normale Terrainbloecke erhalten neue Netzwerk-Entities.
Entfernte Spieler verwenden ein geteiltes Cube-Mesh und wenige Materialien.

## Automatisiert

Erfolgreich ausgefuehrt:

```bash
cargo fmt --all --check
cargo check --locked --workspace
cargo test --locked --workspace -j 2
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace -j 2
cargo build --locked -p terra-server -j 2
```

197 Tests: darunter neun echte UDP-Netzwerktests. Geprueft werden Handshake
mit abweichender lokaler Weltkonfiguration, zwei Clients, bestaetigte
Blockaenderungen, gleichzeitiger Revisionskonflikt, Reichweitenablehnung,
unveraenderbares lokales Replikat, ein einzelnes natuerliches Prop und seine
Nachbarn, Late Join, Reconnect mit geleertem Altjournal, unabhaengige Bewegung
ueber negative Chunk-Grenzen und zwei weiter voneinander entfernte
Streaming-Zentren. Ein 100-Eingaben-Burst wird ohne 100 zusaetzliche
Simulationsschritte bestaetigt. Der Stoerungstest verwendet Lightyears
RecvLinkConditioner mit 40 ms eingehender Latenz, 8 ms Jitter und 5 % Verlust
in beiden Richtungen sowie eine pausierte Client-Schleife. Ungueltige
NaN-Eingaben trennen nur den betroffenen Spieler. Kreative Bodenzustaende
werden ohne Serverfreigabe abgelehnt und mit Freigabe repliziert.

Zusaetzlich: Delta-Idempotenz, monotone Authority-Zaehler, negative
Section-Raender, ungueltige Delta-Koordinaten, Prop-/Appearance-Aenderungen ohne
Terrain-Invalidierung, freigegebene Maus bei Verbindungsabbruch sowie Besitz
und Freigabe des echten Loopback-Sockets durch den Integrated-Server-Thread.

## Native Sitzungen

- `integrated.log`, `integrated.png`: normaler Integrated-Host, echter
  Handshake, autoritative Bewegung ueber 5.520 m, ACK 558, drei offene Eingaben
  bei Aufnahme und erfolgreicher automatischer Abschluss.
- `dedicated.log`: Dedicated ohne DISPLAY/WAYLAND_DISPLAY, zwei private
  Tokens, zwei tatsaechliche UDP-Clients, danach geordnetes SIGINT-Ende.
- `observer.log`, `dedicated-observer.png`: getrennte Client-Instanz,
  `remote_players=1`, `avatars=1`, ACK 817 und drei offene Eingaben bei Aufnahme.
- `companion.log`, `dedicated-companion.png`: zweiter Client bewegt sich
  5.367 m; ACK 1416 und drei offene Eingaben bei Aufnahme.
- `physics.log`, `physics.png`: isolierte bestehende Offline-Fixture fuer
  Fall, Bodenkontakt, 1.335 m Sprunghoehe, Wandkollision, Menuesperre und
  anschliessenden Fall nach Support-Abbau; nach Controller-Umstellung bestanden.
- `headless.log`: separat gebauter Dedicated generiert 27 Sections ohne
  DISPLAY/WAYLAND_DISPLAY und beendet den Smoke-Test mit Exit 0.
- `server-features.txt`: isolierter Server-Dependency-Baum ohne bevy_render,
  bevy_winit, wgpu, terra-render, terra-ui oder TiltUI.
- `bind-failure.log`: nicht vorhandene lokale Bind-Adresse wird ohne Panic
  mit Fehlerstatus beendet; erwarteter Exit 1.

Integrated- und Dedicated-Observer-Aufnahmen wurden geoeffnet und angesehen.
Der andere Spieler ist im Dedicated-Bild sichtbar; dies ist keine simulierte
Replikation oder hineingesetzte Testfigur.

In vorherigen Versuchen wurden ein dauerhaft nachlaufender FIFO-Eingabepfad
und ein falscher serverseitiger Disconnect-Aufruf korrigiert. Der native
Observer-Test schaute ausserdem bei ueberlappendem Spawn senkrecht nach unten
und verletzte selbst das Blickwinkel-Limit; er richtet die Kamera jetzt erst
nach raeumlicher Trennung auf den anderen Spieler. Die finalen Protokolle
stammen aus dem erfolgreichen erneuten Lauf.

## Messgrenzen

Die zwei nativen Clients liefen zeitweise gleichzeitig mit Workspace-Linking.
Ihre HUD-FPS und Ladezeiten sind deshalb **keine** vergleichbaren Messwerte
gegen den frueheren isolierten Offline-Client. Keine FPS-, p95-/p99-, GPU-,
Draw-Call- oder Speicherverbesserung durch diesen Umbau wird behauptet.
Ein isolierter Performancevergleich des Integrated-Hosts bleibt offen.
Die historische CPU-Generierungs-Baseline 4375 Sections / etwa 2.6 s wurde
in diesem Schritt nicht neu gemessen; Generator und Worker-Batching bleiben.

## Offene Grenzen

- Nur Loopback nativ und konditionierte UDP-Verbindungen getestet; kein
  Mehrrechner-LAN-/Internet- oder Langzeittest.
- Noch keine Accounts, Serverliste/Join-Menue, NAT-Relay oder Rechte je Spieler.
  Join erfolgt mit privaten, eine Stunde gueltigen Token-Dateien.
- `allow_flight` erlaubt serverweit Flug/Bodenzustandswerkzeuge, Dedicated
  standardmaessig false; der Integrated-Host erlaubt die lokalen Werkzeuge.
- Reconnect behaelt Welt-Edits waehrend derselben Serversitzung, aber weder
  Spielerposition/-identitaet noch Inventar. Serverneustart braucht neue Tokens.
- Keine Disk-Persistenz. Client-Unload/Reload, Late Join und Reconnect erhalten
  Server-Session-Edits, Serverneustart nicht.
- Keine Spieler-Spieler-Kollision, historische Physik-Rollbacks oder
  Lag-Kompensation. Bei Stoerungen koennen Positionskorrekturen sichtbar sein.
- Initialsynchronisation hat begrenzte Queues und 30 s Timeout; sehr grosse
  Session-Journale und viele Spieler sind noch nicht lastgetestet.
- Render-/Physik-Smoke-Fixtures bleiben gezielt offline, um bestehende
  Szenen- und Geometrieregressionen isoliert pruefen zu koennen.
