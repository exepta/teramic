# Phase 86: Essen Und Spawn-Befehl

## Architektur

Das bestehende Inventar, SurvivalVitals und der autoritative Sitzungsserver
bleiben zustaendig. Benutzen im aktiven Hotbar-Slot sendet alle 100 ms
eine kleine Halteabsicht mit Inventarrevision. Der Server prueft Slot,
Item, Revision, Spielmodus und Essdauer. Fehlende Pulse nach 400 ms,
Loslassen, Menue, Slot-/Inventarwechsel, Teleport, Tod und Treffer
verwerfen den Fortschritt. Neue Pakete verkuerzen die Essdauer nicht.

Inventarentnahme, optionales Restitem und Naehrwerte werden zusammen auf
einer Kopie geprueft und erst danach uebernommen. Ein volles Inventar
verliert bei fehlendem Platz fuer das Restitem weder Essen noch Inhalte.
Die Essensbestaetigung ist von normalen Inventarantworten getrennt,
damit ein Bissen keine wartende Klick-/Crafting-Transaktion bestaetigt.
Die alte Sofort-Essen-Nachricht darf die Haltezeit nicht umgehen.

Survival verbraucht genau ein Item je vollstaendigem Bissen und isst
nicht bei vollem Hungerbalken. Weiteres Halten kann weitere Bissen starten.
Creative zeigt denselben Ablauf ohne Verbrauch oder Survival-Bonus.
Spectator kann nicht essen. Laufender Fortschritt ist absichtlich nicht
persistent und beginnt nach Reconnect neu.

Hungerschaden laeuft auch waehrend des Essens weiter, unterbricht aber
nicht jeden Bissen: sonst waere Hunger null ein unumkehrbarer Zustand.
Tier-, Sturz-, Ertrinkungs- und sonstige direkte Schaeden unterbrechen
ueber eine serverseitige Schadensgeneration, auch nach zwischenzeitlicher
Heilung. Es wird keine Schadensimmunitaet durch Essen eingefuehrt.

## Nahrungsdaten

Zwoelf vorhandene Lebensmittel verwenden ein `food`-Objekt in ihren
bisherigen Dateien unter `assets/terra/items/` beziehungsweise
`assets/terra/props/mushroom.json`. Keine zweite Item-Registry und keine
neuen Item-IDs. Beispiel:

```json
"food": {
  "nutrition": 40.0,
  "saturation": 20.0,
  "eat_seconds": 1.6,
  "regeneration": {
    "health_per_second": 0.5,
    "duration_seconds": 8.0
  }
}
```

`remainder` ist optional und verwendet ein vorhandenes ItemKind.
Die gelieferten Lebensmittel benoetigen keine erfundenen Behaelteritems.
Rohes und gekochtes Fleisch/Fisch, Honig und Pilze besitzen unterschiedliche
Werte; Wolle und Leder bleiben nicht essbar. Saettigung puffert den
Hungerabbau. Regeneration ist begrenzt, addiert sich nicht unbegrenzt
und wird waehrend der Schadens-Erholung unterdrueckt.

Werte muessen endlich und innerhalb definierter Grenzen liegen.
Ungueltige Datei-Overrides melden einen Fehler und verwenden die
eingebauten Werte; entfernte `food`-Felder deaktivieren die betreffende
Nahrung beim Neustart. Assets auf Client und Server gemeinsam ausliefern,
damit lokale Tooltip-/Dauerangaben zu Serveranpassungen passen.
Eine dynamische Food-Mod-Registry/Live-Asset-Replikation ist nicht eingefuehrt.

Die vorhandenen TiltUI-Tooltips zeigen Hunger, Saettigung, Essdauer,
Regeneration und gegebenenfalls Restitem. Ego-Items bewegen sich zum
Mund; lokale und replizierte Avatare besitzen eine passende Armbewegung.
Die vorhandene kreditierte `audio/animals/eat.wav` wird als leises
Kau-Foley wiederverwendet, ohne neue Aufnahme zu behaupten.

## Spawn

```text
/spawn teramic::clown_fish
/spawn teramic::cow
/spawn teramic::bee
```

Der gemeinsame Parser liefert Vorschlaege und einen Help-Eintrag.
Alle vorhandenen Arten werden unter ihrem `teramic::`-Namen angeboten.
Der Befehl benoetigt Creative/Spectator oder die vorhandene Environment-Berechtigung.
In der Serverkonsole:

```text
as Spielername spawn teramic::cow
```

Die vorhandene Konsolen-Autoritaet erhoeht nicht dauerhaft Spielerrechte.
Die Position liegt vor beziehungsweise nahe beim Spieler. Eine begrenzte
Suche nutzt ausschliesslich geladenes, editiertes Terrain: Landtiere
brauchen Boden und Platz, Wassertiere ausreichend zusammenhaengendes
Wasser. Fehlende geeignete Positionen melden einen Fehler statt Tiere
in Luft oder Stein zu erzeugen. Natuerliche Biombindungen gelten nicht
fuer diesen administrativen Befehl.

Command-Tiere erhalten einen disjunkten persistenten ID-Bereich.
Natuerliche Habitat-IDs bleiben unveraendert. Regionen mit ausschliesslich
Command-Tieren werden ebenfalls gespeichert; das bestehende globale
Sicherheitslimit bleibt erhalten.

## Kompatibilitaet

Netzwerkprotokoll **49**: Client und Server gemeinsam aktualisieren.
Spielerdaten **PLY4** speichern Saettigung und den verbleibenden
Regenerationseffekt. Explizite eingefrorene PLY2-/PLY3-Leser uebernehmen
alte Inventare, Modi und Vitals; neue Werte starten dort bei null.
Legacy-Textdaten verwenden dieselben Defaults. Welt-/Block-/Item-IDs,
Generator und Regionsformat bleiben unveraendert. Keine Dependency-Upgrades.

## Reproduktion

Die neue native Fixture verwendet einen separaten neuen Welt-/Profilordner:

```bash
TERRA_FOOD_SMOKE_DIR=/tmp/teramic-food-check \
TERRA_CONFIG_DIR=tools/benchmarks/results/food-spawn-2026-10-20/configs \
TERRA_CAPTURE_DIR=/tmp/teramic-food-check/captures \
target/debug/terra-client --smoke-test-food
```

Der Ordner darf noch keine Testwelt enthalten. Es werden echte
umgebundene Tastatur-/Mauseingaben, der integrierte Server und die
normalen UI-/Renderpfade verwendet. Menuekonfiguration und Profil liegen im
eigenen Unterordner `menu/`; explizite Profil-Overrides werden abgewiesen.
Keine Produktionswelt wird bearbeitet.

## Native Pruefung

RX 6950 XT / Vulkan, 1280x800, separate flache Testwelt, Sichtweite 4:
Der Integrated-Server-Lauf besteht mit echter umgebundener Taste U und
rechter Maustaste. Fruehes Loslassen und Menueoeffnung verbrauchen nichts;
ein vollstaendiger Bissen reduziert gekochten Fisch von 3 auf 2.
Hunger steigt von rund 20 auf 59.49, Saettigung auf rund 14; die Gesundheit
beginnt durch den Daten-Effekt zu regenerieren. Nach Serverende werden
Inventar und erweiterte Vitals aus der gespeicherten Welt erneut geprueft.

Vier Screenshots wurden erzeugt. Ego- und Drittpersonenaufnahme wurden
geoeffnet: Itembewegung, angehobener Arm und das fertige Item-Vorschaubild
in der Hotbar sind sichtbar. Die Fixture wartet dafuer ausdruecklich auf
die bestehende Vorschau-Warteschlange, nicht nur auf Terrainbereitschaft.
[Ego-Bissen](captures/food-first-person-bite.png),
[Drittpersonen-Bissen](captures/food-third-person-bite.png),
[Menue-Abbruch](captures/food-menu-cancelled.png).
Ein aktiver AudioSink wurde nachgewiesen, Stop beim Abbruch ebenfalls;
das ist keine menschliche Hoer-/Mixabnahme.

Der Dedicated Server startet mit entfernten DISPLAY-/WAYLAND_DISPLAY-
Variablen, `--no-save --smoke-test` und beendet sich erfolgreich.
Keine neue GPU-/Audio-Abhaengigkeit wurde im Server eingefuehrt.
Diese Funktionspruefung ist kein neuer FPS-Benchmark und kein Langzeit-
Mehrspieler-Stresstest. Der vorhandene explizite UDP-Soak bleibt separat.

## Automatisierte Pruefungen

Erfolgreich, jeweils mit den vorhandenen Dependency-Versionen:

- `cargo fmt --all --check`
- `cargo check --locked --workspace`
- `cargo test --locked --workspace`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo build --locked --workspace`

`cargo test --locked --workspace`: **1350 bestanden**, kein Fehler.
Der vorhandene ausdruecklich ignorierte 180-Sekunden-UDP-Soak wurde
nicht ausgefuehrt. Die Suite umfasst Food-Validierung, Saettigung,
Regeneration, Restitems, PLY2/PLY3-Migration mit belegtem Inventar/Cursor,
PLY4-Reload, reale UDP-Halte-/Abbruch-/Schadens-/Reconnect-Faelle,
getrennte Spielerinventare sowie Spawn-Rechte, Platzierung und Persistenz.

Waehrend der Validierung wurden alte Sofort-Essen-Testaufrufe auf Halten
umgestellt, die Help-Eintraege erweitert und Kamera-Nahclip sowie
fehlende Eingaberessourcen in isolierten Render-Testwelten korrigiert.
Die ersten nativen Fixture-Starts deckten eine fehlende Menuefreigabe
und eine kollidierende Testprofil-Auswahl auf; der finale Lauf verwendet
ein eigenes Profil und Menueverzeichnis.

Build-/Testprotokolle liegen unter `logs/`. Beide finalen nativen
Protokolle enthalten keine WARN-, ERROR- oder Panic-Meldung.
