# Navigation Und Kingdom-Startbevoelkerung (F49 / F80)

## Navigation Auf Veraenderlichem Terrain

Die lokale Landnavigation prueft die wirklichen Voxel und Objektformen. Eine
kurze Bewegung muss entlang der ganzen Strecke tragfaehigen Boden und genuegend
Platz fuer die jeweilige Koerpergroesse haben. Einzelne Stufen und Treppen sind
begehbar; grosse Abgruende, niedrige Decken und ungeladene Sections sperren den
Weg. Journal-Eintraege ersetzen keine geladene Kollisionsumgebung.

Wenn der direkte Weg versperrt ist, sucht der bestehende lokale Graph mit
hoechstens 96 Knoten in einem Fenster von sechs Bloecken. Unterschiedliche
Etagen bleiben getrennte Knoten. Ein Teilweg ist noch keine Ankunft am Ziel.
Gecachte Kanten werden gegen aktuelle Hindernisse, Boden und gegebenenfalls
Zugriffsrechte erneut geprueft. Zielwechsel verwerfen veraltete Wege;
feststehende NPCs planen zeitlich begrenzt neu statt bei jedem Tick.

Arbeitsauftraege fuer Abbau, Pflanzen, Sammeln, Holzfaellen und Anlieferung
verwenden diese Bewegung. Arbeit und Inventartransfers beginnen erst nach der
physischen Annaeherung und einer freien Interaktionsstrecke. Der vorhandene
Arbeitsstatus meldet versperrte Wege, ungeladene Bereiche und fehlende Rechte.
Ein nicht erreichbares Objekt erzeugt keine vorgetaeuschten Ertraege.

Eigene Arbeiter duerfen vollstaendige Holztueren auf ihrem Weg anfordern. Der
Server prueft Besitzer, lebenden NPC, aktiven Auftrag, Entfernung und die Rechte
**NPC-Arbeit** und **Oeffnungen** fuer saemtliche Tuerteile erneut. Erst die
normale Servertransaktion oeffnet die echte Tuer. Bis dahin bleibt die Kollision
wirksam. Eisen-/Fuehrerhaustueren werden nicht automatisch bedient. Automatisch
geoeffnete Tueren schliessen nach der Passage, sobald Spieler und NPCs den
Bereich verlassen haben; manuelle Aenderungen bleiben erhalten.

Dies ist lokale Navigation im geladenen Simulationsbereich. Globale Strassen-,
Transport- und Fahrplanrouten gehoeren weiterhin zu F69/F65/F66.

## Einmalige Startbevoelkerung

Beim erfolgreichen Gruenden eines **Kingdoms** am Bergfried erscheinen:

| Rolle | Anzahl |
| --- | ---: |
| Schwertkaempfer | 3 |
| Bogenschuetzen | 2 |
| Lanzentraeger | 3 |
| Arbeitslose | 10 |

Die zehn Arbeitslosen erscheinen in der bestehenden Arbeitsverwaltung des
Gruenders. Sie erhalten dort Berufe, Arbeitsgebiete und ein Depot; die acht
Soldaten verwenden die vorhandenen militaerischen Rollen und Tagesablaeufe.
Die NPCs gehoeren dem Gruender und sind ueber einen dauerhaften Gruendungsbeleg
mit dem Kingdom verbunden. Eigene NPCs verbrauchen keine natuerlichen Wildtier-
Spawnplaetze; die globale Kreaturenobergrenze gilt weiterhin.

Vor der Gruendung sucht der Server auf geladenem, trockenem, unveraendertem
Naturboden nahe dem Bergfried nach 18 freien Plaetzen in dessen neu beanspruchtem
Chunk. Spieler, vorhandene Kreaturen, Fahrzeuge und relevante Weltobjekte
werden bei der Platzierung beruecksichtigt. Die begrenzte Suche generiert keine
zusaetzlichen Chunks. Sichere gebaute Boeden duerfen als Weg vom Gruender zu
diesen Plaetzen dienen. Bei Platzmangel erscheint eine uebersetzte Fehlermeldung;
die Gruendung und die gesamte Kohorte bleiben unveraendert. Es gibt keine neue
zusaetzliche Inventargebuehr fuer die Bevoelkerung.

Kingdom, 18 NPCs, Besitzer und Gruendungsbeleg werden gemeinsam uebernommen.
Die Identitaeten verwenden einen eigenen Namensraum und werden auch nach Tod
oder Aufloesung nicht fuer eine spaetere Gruendung wiederverwendet. Wiederholte
Netzwerkanfragen, Laden und erneutes Oeffnen des Bergfrieds vergeben keine
weiteren NPCs. Tote oder entfernte Start-NPCs werden nicht ersetzt.

Doerfer, Staedte und Aussenposten vergeben keine weitere Startbevoelkerung.
Bestehende Kingdoms alter Spielstaende erhalten beim Laden keine nachtraeglichen
NPCs. Neutrale Nomaden (F78) und produktive generierte Doerfer (F79) sind eigene
offene Aufgaben.

## Speicherung Und Pruefung

Index **ID23** erhaelt die Gruendungsbelege; bisherige ID22-Spielstaende werden
mit leerem Belegregister migriert. Client und Server verwenden gemeinsam
Netzwerkprotokoll **138**; der Gruendungsfehler wurde in Protokoll136 eingefuehrt,
Protokoll137 ergaenzte private NPC-Inventarantworten; aktuell ergaenzt
Protokoll138 den Stations-Wartezustand fuer Holzfaeller.

Die automatisierten Tests pruefen lokale Hindernisse, Tueren, Etagen,
Zielwechsel, Rechte, ungeladene Sections, reale Arbeitsankunft, exakte
Startrollen, Atomaritaet, Replay-Schutz und Save/Reload.
Der kurze native Test verwendet ausschliesslich neu erzeugte Konfigurations-,
Profil- und Weltpfade unter `target/kingdom-starters/`:

```sh
cargo build --locked -p terra-client -p terra-server --bins
bash tools/benchmarks/run-kingdom-starters.sh
```

Er gruendet und benennt ueber die echte Client-/Server-Anwendung um, speichert,
startet erneut und prueft dieselben 18 gespeicherten und replizierten IDs und
Rollen. Anschliessend prueft er Aufloesung und freigegebenen Bergfriedabbau.
Sechs Fensteraufnahmen und der ID-Beleg bleiben im isolierten Testordner.
Der bestehende OS-Maushelfer benoetigt KDE/Wayland und vorhandenen
`uinput`-Zugang; auch ein XWayland-Clientfenster wird darueber bedient.
Den nativen Lauf nach abgeschlossener Kompilierung
ausfuehren; parallele Vollbuilds koennen den Clientstart bis zum Sitzungstimeout
verzoegern.
