# Phase 64: Fackeln, Angriff Und Wassertreten

## Umsetzung

- Fackelmodell ohne eigenen Schatten, bisheriger Nah-Fill entfernt.
- Gesetzte und getragene Fackeln: Hoehlenfaktor 0.6, weicher Uebergang,
  unveraenderte Aussenhelligkeit und Reichweite.
- Rechte Hand bewegt sich bei Schlaegen zur Blickrichtung (-Z). Die erste
  Person fuehrt das vorhandene Item deutlicher vor; kein Handmesh hinzugefuegt.
- Eigene aufrechte Wassertretpose mit weiterlaufenden Arm-/Beinzyklen,
  weicher Uebergang zu aktivem Schwimmen bei Bewegungseingabe.
- Werkbanklaterne als 4-m-Punktlicht aus ihrer vorhandenen JSON, ein Licht
  pro logischer Werkbank, passend zu allen Ausrichtungen und Doppelbreite.

Keine neuen Dependencies, Weltgenerierungs-, Speicher- oder Protokollaenderungen.
Lichtpool/Qualitaetsbudgets bleiben bestehen. Die Laterne benoetigt keine eigene
Schattenkarte; entfernte Fackel-Fills sparen jeweils eine Punktlicht-Entity.
Keine neue FPS-/Speicherparitaet behauptet.

## Validierung

Erfolgreich ausgefuehrt:

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo test --workspace --no-fail-fast`: **982 bestanden**, ein bestehender
  dreiminuetiger Soak ignoriert und hier nicht erneut ausgefuehrt.
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --workspace`
- Fensterloser Server-Smoke mit entferntem `DISPLAY`/`WAYLAND_DISPLAY`:
  erfolgreicher Abschluss nach 27 geladenen Sections.

Regressionen messen Hand-/Werkzeugendpunkte, anhaltende Idle-Bewegung,
Hoehlendimmung, echte Schattenflags hochgeladener Fackel-/Terrainbatches
und Laternenpositionen bei Doppelbreite, allen vier Rotationen und negativen
Koordinaten. Die bestehenden Tests pruefen weiterhin Abtauchen/Loslassen,
Uferausstieg, Inventare und reale UDP-Replikation.

Native Vulkan-/Wayland-Pruefung auf RX 6950 XT / RADV Mesa 26.2.4:

- `caves/`: 13 Aufnahmen, vier Hoehlenqualitaeten, Boden-/Wandfackeln,
  Nacht, Erhalt jenseits 50 m, getragenes Licht und eigene Werkbankansicht.
  Die Fixture prueft genau einen schattenlosen 4-m-Emitter an der Doppel-
  werkbank. Nachtfackel und Laterne wurden visuell angesehen: kein dunkler
  eigener Fackelschatten, dezentes warmes Licht auf der Arbeitsflaeche.
- `hands/`: fuenf Rendereraufnahmen fuer Block, Spitzhacke, Schwert,
  Schlagphase und Third Person. Diese Transformfixture ist keine simulierte
  erfolgreiche Netzwerkverbindung. Der Vorwaertsweg ist zusaetzlich ueber
  echte Hand-/Werkzeugendpunkte getestet.

Die Wassertretpose ist automatisiert geprueft, aber in dieser Phase nicht
mit einem nativen Schwimm-/Tauchlauf visuell abgenommen. Keine gegenteilige
Behauptung aus den trockenen Held-Item-Aufnahmen ableiten. Die Tests liefen
teilweise neben den Renderfixtures; geloggte FPS sind keine kontrollierte
Vorher-/Nachher-Performanceabnahme. Rohlogs liegen neben diesem Bericht.

## Grenzen

Die Spielerkapsel und vertikale Wasserphysik bleiben unveraendert. Fuer
Remote-Avatare ist Geschwindigkeit weiterhin der Naeherungswert fuer aktiven
Schwimmantrieb; reines Mitgetragenwerden ist noch nicht getrennt repliziert.
Schatten anderer Objekte im Fackellicht bleiben erhalten. Punktlicht ist
keine indirekte Voxellichtsimulation. Viele Lampen unterliegen weiterhin
dem bestehenden Qualitaetsbudget. Keine aus Kurztests abgeleitete FPS-Aussage.
