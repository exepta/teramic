# Phase 63: Fackellicht, Schwimmen Und Itemdarstellung

## Umfang

Sieben angefragte Bereiche im bestehenden Renderer, der gemeinsamen Physik,
dem Netzwerk und TiltUI verbessert. Details und Grenzen im Phase-63-Abschnitt
der Root-README. Keine Dependency-, Weltgenerierungs- oder Speicherformatwechsel.
Netzwerkprotokoll 37 verlangt gemeinsam aktualisierte Clients und Server.

Fackel: 11500 -> 1450 Lumen, 24 -> 28 m Cullingradius, Stuetz-Fill 10 -> 4 %.
Keine neue Schattenkarte oder neuer Fullscreenpass. Held-Items teilen sich die
vorhandenen Drop-Meshes und Materialien; keine neuen Modelle pro Frame.
Itembrowser misst seinen echten Platz statt fest 45 Eintraege zu verwenden.

## Validierung

Erfolgreich:

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo test --workspace --no-fail-fast`: **978 bestanden, ein ignorierter**
  bereits bestehender dreiminuetiger Soak; nicht erneut ausgefuehrt.
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --workspace`
- Isoliert `cargo build -p terra-server`; Featurebaum ohne `bevy_render`,
  `bevy_pbr`, `bevy_winit` oder `bevy_audio`. Dieser Server startet mit entferntem
  `DISPLAY`/`WAYLAND_DISPLAY` und beendet den Headless-Smoke nach 27 Sections.

Neue Regressionen pruefen Abtauchen/Loslassen/Auftauchen, seitliche Arme ueber
32 Schwimmphasen, auslaufende Schlagbewegung, First-Person-Vorstoss, Cursor-
Bitmap und Seitengroesse aus echten ComputedNodes inklusive Resize/DPI.
Der bestehende reale UDP-Zwei-Client-Test prueft nun auch den autoritativen
gehaltenen Itemtyp und dessen Entfernen bei leerer Auswahl. Persistenz von
Cursorfarbe und Reichweite sowie unveraenderte Inventar-Deckkraft sind geprueft.

Native Vulkan/Wayland-Pruefung auf RX 6950 XT, RADV/Mesa 26.2.4:

- `torches/`: zwoelf Ansichten, vier Hoehlenqualitaeten, gesetzte Boden-/
  Wandfackeln, Nacht, Erhalt jenseits 50 m und getragene Remote-Fackel.
  Hoehlen-/Nachtaufnahmen wurden stichprobenartig angesehen. Der Boden ist
  weniger ueberstrahlt; der Punktlicht-Falloff bleibt sichtbar.
- `hands/`: fuenf echte Rendereraufnahmen mit Steinblock, Spitzhacke, Schwert,
  Schlagphase und Third-Person-Fackel. Kein Handmesh in First Person.
  Dies ist explizit eine Transform-/Rendererfixture, keine simulierte erfolgreiche
  Netzwerkverbindung. Der Fixed-Step ist nur in dieser Fixture ausgesetzt,
  damit deren Vorschauzustand keine Eingabe-ACKs eines nicht vorhandenen Peers
  erwartet. Die erste Probe vor dieser Fixturekorrektur zaehlt nicht als final.
- `menu/`: vollstaendiger Menue-Smoke einschliesslich offenem Colorpicker,
  Farbwahl aus dem automatischen Default, Apply/Reload, Discard und Rollback.
- `inventory/`: echter OS-Mauslauf, neun Aufnahmen; Auswahl, Tooltip, MouseTweaks,
  Crafting, Stacktransfer, Ausserhalb-Drop und Scrollen mit echtem Integrated
  Server. Dieser Lauf bei normaler Fensterbreite ist bestanden.
- `inventory-4k/inventory-open.png`: **3840 x 2160 Renderpixel**, 79 Items und
  **1 / 1** Seiten, zentriertes Inventar und korrekt genutzte rechte Breite.
  Der zusaetzliche Layout-only-Modus wartet auf den GPU-Capture und prueft die
  Einzelseite. Ein vorheriger voller 4K-Mauslauf erreichte dagegen keinen
  Desktopzeiger (`pointer=None`) am vergroesserten Fenster und schlug beim
  Tooltip fehl; dessen Log bleibt erhalten. Kein bestandener 4K-Mauslauf behauptet.

Alle finalen Cargo-/Native-Logs liegen neben diesem Bericht. Ein frueher
Gesamttest fand die alte 24-m-Erwartung und einen falsch gewaehlten Cursor-
Randpixel im neuen Test. Beide Erwartungen sind korrigiert; erst der erneute
vollstaendige Lauf mit 978 bestandenen Tests wird als erfolgreich gewertet.

## Grenzen

Keine neue FPS-/GPU-/Speicherparitaet aus funktionalen Kurztests ableiten.
Teile der Renderlaeufe liefen neben Cargo; deren geloggte FPS sind keine
kontrollierte Performanceabnahme. Kein neuer Framepass; gehaltene Items
benoetigen selbstverstaendlich zusaetzliche sichtbare Geometrie/Draws.
PBR-Falloff ist kein Voxellicht/GI. Vertikale Spielerkapsel bleibt erhalten.
Fern-Namen verwenden feste UI-Schriftgroesse, keine entfernungsabhaengige
Vergroesserung. Native Renderfixtures sind keine echte Zwei-Betrachter-
Truheninteraktion. Der neue Cursor ist als Bitmap/Style getestet, aber ein
nativer Zwei-Betrachter-Truhencursortest bleibt offen. Noch kein generisches Remote-Attack-Ereignis fuer einzelne
Creative-/Luft-/Mobschlaege; Survival-Mining wird uebertragen. Erste Person
verwendet die Weltkamera, keinen gesonderten Viewmodel-Renderpass.
