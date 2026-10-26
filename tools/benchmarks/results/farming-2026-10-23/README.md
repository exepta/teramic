# F04 Ernte Und Anbau

Phase 109 erweitert die vorhandenen Welt-Props, Inventare, Nahrungsdefinitionen,
Rezeptkataloge und das EditJournal. Weltformat 2 und Generator 5 bleiben erhalten;
keine neue Pflanzen-Engine, Dependencies oder geaenderten bestehenden Block-IDs.

## Anbaukreislauf

- Drei Saaten: Weizen, Mais und Salat. Wildes Gras, Blumen und kleine Pflanzen
  geben deterministisch gelegentlich einen Samen; alle drei Arten sind in Survival erhaeltlich.
- Platzieren/Benutzen mit Samen bereitet freie Gras-/Erdflaechen als Ackerboden
  vor. Ein Pflanzenfeld belegt einen Block; Weizen/Mais brauchen zwei freie Zellen.
- Acht sichtbare Stadien. Reife Pflanzen geben Ertrag und mehrere Samen;
  unreife Pflanzen nur einen Samen. Neue Aussaat verwendet dieselben Inventarstapel.
- Mais, Salat und Brot sind essbar. Fuenf Einzelrezepte ergaenzen Brot,
  Samenrueckgewinnung und herstellbaren Ackerboden im Too-Many-Items-Browser.
- Das Info-HUD zeigt die Wachstumsstufe und Erntereife. Interne Pflanzen-Props
  erscheinen nicht als zusaetzliche, direkt platzierbare Creative-Items.

Die Grundzeiten sind 600/780/480 Sekunden. Feuchtigkeit aus Wasser innerhalb
von vier Bloecken beziehungsweise exponiertem Regen, Tageslicht und die
geblendeten SeasonSample-Gewichte bestimmen den Fortschritt. Glas laesst
gedaempftes Licht, aber keinen Regen durch. Trockene oder dunkle Pflanzen sowie
diese drei Kulturen im Winter wachsen nicht. Die Wachstumsdauer wird beim Saeen
festgeschrieben; spaetere JSON-Aenderungen machen alte Pflanzen nicht ungueltig.

Reife Futterpflanzen werden von passenden Tieren gefressen und beginnen erneut
zu wachsen. Wildes Futter erhaelt einen gespeicherten 120-180-s-Nachwachstimer.
Beide Pfade behalten ihre Pflanzen-ID und erzeugen keine weiteren Erntedrops.

## Darstellung Und Lebensdauer

Solide, gebuendelte 3D-Templates zeigen goldene Weizenaehren, breite gestufte
Maisblaetter mit Kolben/Rispen und geschichtete Salatkoepfe. Drei LODs verwenden
dieselbe Materialidentitaet und das bestehende Wind-/Picking-System. Ackerboden
101 nutzt den gemeinsamen Cube-Mesher mit flachen dreidimensionalen Furchen.

Eine neutrale geteilte Pflanzenpalette erhaelt reife Gold-/Gruentoene. Pro
Microblatt entstehen weder ECS-Entity noch Materialinstanz. Reife Near-Templates:
Weizen 3888, Mais maximal 5760, Salat 3064 Vertices; entfernte Modelle sind einfacher.
Der kompakte Salatkopf besitzt gefaltete Voxeloberflaechen ohne innere,
unsichtbare Wuerfelfaces. Weizen bleibt in Stufe 6 teilweise gruen; erst die
tatsaechlich erntereife Stufe 7 wird vollstaendig golden.

Die Autoritaet verarbeitet maximal 128 Kulturen und 128 Wildfutter-Timer pro
Sekunde ueber einen Section-Index. Ungeladene Bereiche pausieren, Zeitnachlauf
ist begrenzt. Reiner Fortschritt aendert keine Terrain-/Wasser-Meshes oder
Prop-Epochen; nur ein sichtbarer Stufenwechsel erneuert Pflanzengeometrie.

Stabile IDs je Feld/Pflanzenart mit sicherer Wiederbelebung verhindern, dass
jede Ernte einen neuen dauerhaften Entfernungseintrag hinterlaesst. Eine volle
Section wird vor der Bodenaenderung abgewiesen. Mehrteilige Ernten reservieren
Drop-Kapazitaet vor dem Entfernen; alte Anfragen bleiben durch Prop-Epochen gesperrt.

## Speicherung Und Inhalte

Neue Regionen verwenden RGA2 mit Pflanzenfortschritt, Feuchtigkeit, gespeicherter
Wachstumsdauer und Wildfutter-Timern. RGA1 und aeltere Leser bleiben erhalten.
Spielerdateien bleiben PLY7; neue Item-/Prop-Tags sind angehaengt. Protokoll 61
erfordert gemeinsam aktualisierte Clients und Server.

Neue Inhalte: Farmland (101), Wheat/Corn/Lettuce als Welt-Props, Wheat Seeds,
Corn Seeds, Lettuce Seeds, Wheat, Corn, Lettuce und Bread als Items. Wachstum
liegt einzeln in `assets/terra/farming/`, andere Definitionen in den bestehenden
Block-/Prop-/Item-/Crafting-Verzeichnissen.

Die Werkzeug-Griffrotation richtet die Arbeitsseite nach vorne und die breite
Kopfflaeche zur tatsaechlichen Ego-Kameraposition aus. Drittpersonen-Werkzeuge
sind ebenfalls um die Griffachse gedreht; Groessen, Griffpunkt und Vorwaertsschlag
bleiben erhalten.

## Grenzen

Kein Offline-Wachstum und derzeit kein Wachstum durch ausschliessliches
Fackellicht in geschlossenen Hoehlen. Die Vorlage ist eine illustrierte
Zyklusdarstellung; native Voxelgeometrie ist keine pixelidentische Bildkopie.
Kurze Funktionspruefungen sind keine FPS-/VRAM-Paritaetsmessung. Keine langen
Soak-Tests gemaess Nutzerwunsch.

## Reproduzieren

`target/debug/terra-client --smoke-test-farming` erstellt einen getrennten
temporaren Survival-Spielstand. Die Szene enthaelt die drei Kulturen mit allen
Stufen. Nahezu reife Pflanzen pruefen echte zeitgesteuerte Reifung ohne einen
zehnminuetigen Wartezyklus; Aussaat/Ernte verwenden echte Eingaben und den
integrierten Server. Nach Abschalten wird der gespeicherte Zustand erneut gelesen.
`TERRA_FARMING_SMOKE_DIR` muss auf ein noch nicht existentes Testverzeichnis
zeigen. `TERRA_CAPTURE_DIR` bestimmt die Bildablage.

`target/debug/terra-client --smoke-test-hands` prueft Werkzeuggriffe,
Vorwaertsschlag, Bildschirmrand und das Entfernen nicht gehaltener Gegenstaende.

## Pruefungen

| Pruefung | Ergebnis |
| --- | --- |
| `cargo fmt --all --check` | Bestanden ([Log](logs/fmt.log)) |
| `cargo check --locked --workspace --all-targets` | Bestanden ([Log](logs/check.log)) |
| `cargo test --locked --workspace --no-fail-fast` | 1922 bestanden, 3 ignoriert ([Log](logs/tests-final.log)) |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Bestanden ([Log](logs/clippy.log)) |
| `cargo build --locked --workspace` | Client und Dedicated Server bestanden ([Log](logs/build-final.log)) |
| `terra-server --smoke-test --no-save` | Headless, 27 Sections bestanden ([Log](logs/server.log)) |
| `terra-client --smoke-test-farming` | Echter Survival-Zyklus und 5 Aufnahmen bestanden ([Log](logs/native-farming.log)) |
| `terra-client --smoke-test-hands` | 9 Ansichten bestanden ([Log](logs/native-hands.log)) |

`CARGO_BUILD_JOBS=1 cargo test --locked --workspace --no-fail-fast`:
1922 Tests bestanden, drei bestehende Tests bewusst ignoriert. Der erste Lauf
hatte fuenf veraltete Katalog-/Material-/Audio-/Drop-Erwartungen offengelegt;
diese wurden fuer die tatsaechlich ergaenzten Inhalte aktualisiert. Der finale
Lauf ist vollstaendig erfolgreich: [Testprotokoll](logs/tests-final.log).
Nach der letzten rein dimensionalen Maiskolbenkorrektur wurden die drei
Pflanzenmodelltests nochmals mit `cargo test --locked -p terra-world --features
bevy props::visual::crops::tests` erfolgreich ausgefuehrt:
[Modelltests](logs/crops-final.log). Die breiteren sichtbaren Koerner und die
schmalere reife Huelle erzeugen keine zusaetzlichen Vertices.

Abgedeckt sind unter anderem Licht/Glas/Regen, Feuchtigkeit, Saisongewichte,
Stufenwechsel ohne Terrain-Invalidierung, volle Sections vor Bodenaenderung,
5000 Nachpflanzzyklen ohne anwachsende Prop-/Tombstone-Listen, geladene und
ungeladene Nachbar-Sections, echte Futterwahl, Drop-Kapazitaet und Replay-Schutz,
RGA1-Migration/RGA2-Roundtrip sowie gueltige, deterministische Meshes mit festen
Budgets. Separate Tests sichern breite Werkzeugflaechen und den Vorwaertsschlag.

Die native Survival-Pruefung verwendet reale Benutzen-/Abbauen-Eingaben und
den integrierten Server: alle drei Saaten gepflanzt, jeweils genau ein Samen
verbraucht, Dirt in Farmland umgewandelt, drei beinahe reife Kulturen zeitbasiert
gereift, Weizen geerntet und Ertrag/Samen geprueft. Nach sauberem Serverende
enthielt der erneut geladene Spielstand 29 Kulturen; geernteter Weizen blieb
entfernt. [Laufprotokoll](logs/native-farming.log).

Der Werkzeugtest hat neun native Ansichten einschliesslich Spitzhacken,
Schwert, Axt und Schlag erfolgreich aufgenommen. Die gehaltenen Modelle
bleiben am unteren Bildschirmrand verankert, der Ego-Arm bleibt ausgeblendet.
[Laufprotokoll](logs/native-hands.log), [Schwertansicht](native/hands/hands-2.png).

Die Funktionsszene verwendet einen isolierten flachen Spielstand mit Sichtweite
4 bei 1280x800, die Werkzeugansichten 1920x1200. Das sind Darstellungs-/
Interaktionspruefungen, keine vergleichenden FPS-Messungen oder Veraenderungen
der regulaeren Benutzereinstellungen. Beim Welthandshake trat die vorhandene
kurze Warnung ueber vorzeitig empfangene `CreatureStates` auf; sie wird hier
nicht als durch F04 behoben ausgegeben.

## Native Ansichten

Die Bilder wurden direkt im Vulkan-Client auf der RX 6950 XT aufgenommen und
visuell kontrolliert. Die Gesamtansicht zeigt alle acht Stufen jeder Kultur;
Nahansichten zeigen frischen bis erntereifen Weizen, Maisblaetter/Kolben und
die gefalteten Salatkoepfe. Kein Anspruch auf pixelidentische Reproduktion der
illustrierten Vorlage oder auf allgemeine FPS-Paritaet.

- [Alle Wachstumsstufen](native/farming/farming-stages.png)
- [Weizen](native/farming/farming-wheat.png)
- [Mais](native/farming/farming-corn.png)
- [Salat](native/farming/farming-lettuce.png)
- [Survival-Inventar](native/farming/farming-inventory.png)
- [Ego-Spitzhacke](native/hands/hands-1.png)
- [Ego-Schwert](native/hands/hands-2.png)
- [Vorwaertsschlag](native/hands/hands-3.png)
- [Ego-Axt](native/hands/hands-7.png)
