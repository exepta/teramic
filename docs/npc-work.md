# NPC-Arbeitsauftraege Und Holzfaeller (F58 / F61)

Die erste produktive Arbeitsverwaltung verwendet echte NPC-Bewegung, Voxeldaten,
Inventare und Serverantworten. Bergarbeiter, Landwirte, Sammler und Holzfaeller
arbeiten mit tatsaechlich geladenen Weltobjekten und vorhandenen Vorraeten.
Berufsstationen verbinden diese Arbeit mit eigenen Lagerbloecken.

## Auftrag Zuweisen

1. Einen bereits gegruendeten Bergfried oeffnen und den Tab **Arbeitsauftraege**
   waehlen. Die Liste zeigt verfuegbare zivile Dorfbewohner und eigene Auftraege.
2. Eine Arbeitskraft und den Beruf Bergarbeiter, Landwirt, Sammler oder
   Holzfaeller auswaehlen.
3. Die beiden inklusiven Ecken des Arbeitsgebiets als X/Y/Z eingeben. Die maximale
   Groesse ist 32 x 16 x 32 Bloecke; negative Koordinaten werden unterstuetzt.
4. Die Koordinaten einer normalen Truhe, eines weltplatzierten Frachtlagerblocks
   oder einer passenden Berufsstation als Lager angeben. Bei einer Doppeltruhe
   wird die Hauptseite verwendet. Die zulaessigen Berufe stehen in der Tabelle
   unten; Inventare montierter Fahrzeuge sind keine solchen Weltbloecke.
5. Prioritaet 0 bis 9 einstellen und **Auftrag speichern** waehlen. Hoehere Werte
   haben Vorrang; der Server bestaetigt oder verwirft die Zuweisung.

Bei der Zuweisung muss der Spieler hoechstens 128 Meter vom NPC entfernt sein.
Beide angegebenen Gebietsenden und das Lager duerfen hoechstens 64 Meter vom NPC
entfernt liegen. Fremde Arbeitskraefte lassen sich nicht uebernehmen. Das Lager
muss geladen sein; Zuweisung und Ausfuehrung pruefen die Kingdom-Rechte fuer
NPC-Arbeit sowie die erforderlichen Lager-, Abbau- und Baurechte.
Eine frisch platzierte, noch nie geoeffnete Truhe oder Station ist bereits ein
gueltiges leeres Lager. Ein leerer Inventardatensatz entsteht erst bei einer
tatsaechlichen Anlieferung. Generierte Truhenbeute wird bereits beim Laden der
Struktur angelegt und dadurch nicht durch ein leeres Lager ersetzt.

## Berufsstationen Und Lager

Alle neun Stationen besitzen ein echtes Inventar mit 18 Plaetzen, ein eigenes
Blockmodell und die vorhandene Containeransicht. Station platzieren, Vorraete
einlagern und ihre Koordinaten im Arbeitsauftrag als Lager eintragen. Das
Platzieren oder Oeffnen einer Station vergibt noch keinen NPC-Auftrag.

`terra_core::profession_stations::supports_work` legt die Kombinationen fest.
Serverzuweisung und laufende Arbeit pruefen dieselbe Zuordnung:

| Weltblock | Material-ID | Unterstuetzte Arbeitsauftraege |
| --- | --- | --- |
| Normale Truhe | 29 | Alle vier Berufe |
| Weltplatzierter Frachtlagerblock | 216 | Alle vier Berufe |
| Holzfaeller-Station | 297 | Holzfaeller, Sammler |
| Schmiede-Station | 298 | Sammler |
| Verkaufsstand | 299 | Sammler |
| Armee-Station | 300 | Sammler |
| Lok-Station | 301 | Sammler |
| Minen-Station | 302 | Bergarbeiter, Sammler |
| Metzger-Station | 303 | Sammler |
| Bischofs-Station | 304 | Sammler |
| Farmer-Station | 305 | Landwirt, Sammler |

Sammler koennen jedes Stationslager beliefern. Schmied, Verkaeufer, Soldat,
Lokfuehrer, Metzger und Bischof erhalten dadurch noch keine automatisierte
Produktion, Handels-, Militaer-, Bahn- oder Heilungslogik. Die sechs entsprechenden
Stationen sind derzeit Lager und vorbereitete Berufsorte.

## Berufe Und Versorgung

| Beruf | Tatsaechliche Arbeit | Vorrat im Lager |
| --- | --- | --- |
| Bergarbeiter | Erreichbare, freiliegende natuerliche Bloecke im Gebiet abbauen und die Ertraege anliefern | Eine Spitzhacke mit passender Abbaustufe |
| Landwirt | Bereits angelegte, reife Kulturpflanzen ernten und dieselbe Kultur auf demselben Beet neu pflanzen | Eine Hacke und mindestens ein passender Samen |
| Sammler | Bereits existierende, aufhebbare Item-Drops im Gebiet einsammeln und anliefern | Kein Werkzeug erforderlich |
| Holzfaeller | Zugeordnete natuerliche oder eigene gepflanzte Baeume faellen, echte Drops einsammeln und anliefern, danach dieselbe Baumart nachpflanzen | Eine Axt und ein passender Setzling fuer die Nachpflanzung |

Werkzeuge, Samen und Setzlinge werden aus dem echten Lager in das neunplaetzige
NPC-Inventar uebertragen. Samen und Setzlinge werden erst bei erfolgreicher
Pflanzung verbraucht. Bergarbeiter und Landwirte behalten ihre realen Werkzeuge;
fuer ihre Spitzhacken und Hacken gibt es noch keinen Verschleiss.

Der NPC laeuft zum Lager und zum Arbeitsziel. Ertraege wandern zuerst in seinen
eigenen Vorrat und danach in das Lager. Nicht passende Fracht bleibt erhalten,
wenn das Lager voll ist. Passende Arbeitswerkzeuge und ein Samen je Kultur werden
fuer den naechsten Auftragsschritt zurueckbehalten. Ziele werden zwischen NPCs
reserviert, damit derselbe Block oder Drop nicht mehrfach verarbeitet wird.

## Ablauf Des Holzfaellers

Der Holzfaeller sucht Baumwurzeln im Arbeitsgebiet und verwendet die vorhandenen
exakten Faellplaene. Diese ordnen Staemme und Blaetter einem natuerlichen oder
gepflanzten Baum zu; zusammenhaengende Holzbloecke allein gelten nicht als Baum.
Spielerersetzte Bloecke und Holzgebaeude bleiben erhalten. Fremde gepflanzte
Baeume werden nicht automatisch abgeerntet. Abbaurechte fuer den gesamten
Faellplan und die jeweils aktuellen Rechte bei weiteren Schritten werden geprueft.

Das Gebiet begrenzt die waehlbare Baumwurzel. Zugeordnete Aeste und Kronen koennen
ueber dessen Rand hinausragen; auch dort gelten die jeweiligen Rechte. Fehlende
Sections eines bereits ausgewaehlten Baums werden ueber das vorhandene begrenzte
Streaming angefordert.

Die Arbeitskraft holt eine echte Axt am Lager ab und laeuft zum Stamm. Die
Arbeitszeit richtet sich nach Stammzahl und Axttyp. Anschliessend entfernt sie
zugeordnete Bloecke in begrenzten Schritten. Jeder tatsaechlich entfernte Stamm
erzeugt einen echten Holz-Drop; pro Baum entsteht einmal ein passender Setzling.
Die gespeicherten Drop-IDs verhindern ein erneutes Erzeugen beim Fortsetzen.
Bereits von einem Spieler oder Sammler aufgenommene Drops werden nicht ersetzt.
Der Holzfaeller laeuft die Drops an, nimmt sie auf und bringt die Fracht zum Lager.

Nach dem Faellen und Aufraeumen der Krone holt er einen Setzling der urspruenglichen
Baumart aus dem Lager. Auch ein zuvor angelieferter Setzling ist dabei ein realer
Lagervorrat. Bevorzugter Pflanzort ist die bisherige Wurzel; die begrenzte Suche
prueft ausserdem Plaetze bis zwei Bloecke daneben in X/Z auf derselben Bodenhoehe
und innerhalb des Arbeitsgebiets. Die normale Setzlingspruefung beachtet Boden,
Wasser, belegte Flaechen und Rechte. Ohne Vorrat oder geeigneten Pflanzplatz bleibt
der Teilauftrag mit entsprechendem Wartegrund erhalten; ein Setzling wird nicht
bei einem gescheiterten Pflanzversuch verbraucht.

### Reale Axt-Haltbarkeit

Jeder erfolgreich entfernte Stamm verbraucht genau eine Benutzung der verwendeten
Axt. Bei null Restbenutzungen zerbricht sie; fuer weitere Staemme wird eine neue
Axt am Lager benoetigt. Blaetter und zwischenzeitlich veraenderte, nicht mehr
zugeordnete Bloecke verbrauchen keine Benutzung.

| Axt | Benutzungen im Neuzustand |
| --- | ---: |
| Holz | 64 |
| Stein | 128 |
| Eisen | 256 |
| Gold | 96 |
| Diamant | 768 |
| Rubin | 1024 |

Eine benutzte Axt wird als individuelles `ItemKind::WornTool` mit Typ und
Restbenutzungen gespeichert, mit Stapelgroesse eins. Inventartransfer, Rueckgabe
ins Lager, Drops und Speichern erhalten diese Daten. Spieler koennen eine solche
Axt mit derselben Werkzeugklasse, Abbaugeschwindigkeit und Angriffskraft weiter
benutzen. Allgemeiner Spielerverschleiss wird derzeit nicht simuliert; Benutzung
durch einen Spieler vermindert die gespeicherte Resthaltbarkeit noch nicht.

## Status Und Pause

Die Ansicht zeigt den bestaetigten Zustand: bereit, unterwegs, bei der Arbeit,
Anlieferung oder einen Wartegrund. Dazu gehoeren fehlende Werkzeuge oder Samen,
volle Lager, versperrte Wege, ungeladene Bereiche, fehlende Rechte, fehlende
Arbeitsziele und ein ungeeigneter Pflanzplatz. Nachts ruht die Arbeit entsprechend
dem Villager-Tagesrhythmus; die vorhandene Bewohnerlogik uebernimmt Heimkehr und
Schlaf, statt den NPC parallel weiterarbeiten zu lassen.

Der Schalter **Aktiviert / Pausiert** sendet eine revisionsgepruefte Serveraktion.
Eine Pause verwirft weder Fracht noch Werkzeuge oder einen begonnenen Baumauftrag.
Teilbaum, ausstehende Drops, Nachpflanzung und restliche Axtbenutzungen bleiben
erhalten. Auch ein volles Lager oder Axtbruch loescht diese Daten nicht.
Ein begonnener Baumauftrag muss abgeschlossen werden, bevor Beruf, Gebiet oder
Lager geaendert werden koennen; Pausieren und Fortsetzen bleiben moeglich.
Die Anzeige bleibt bis zur Serverbestaetigung beim autoritativen Zustand. Es gibt
keinen Loeschen-Button, der einen getragenen Vorrat unbemerkt entsorgen koennte.

## Speicherung Und Grenzen

Auftraege, Aktivierung, NPC-Inventare und begonnene Baumauftraege werden im
Speicherindex **ID22** persistiert. Der eingefrorene **ID21**-Decoder uebernimmt
bisherige F58-Auftraege und Inventare und ergaenzt leere Baumauftragsdaten.
Aeltere Indexversionen laden weiterhin ohne erfundene Arbeitsauftraege.
Client und Server benoetigen gemeinsam Netzwerkprotokoll **135**.

Die Autoritaet arbeitet mit 10 Hz und begrenztem Budget: hoechstens 16
Arbeitskraefte pro Aufruf und 64 Rasterkandidaten je besuchter Arbeitskraft.
Der Sammler betrachtet hoechstens 256 passende Drops bei seiner Zielsuche.
Der Holzfaeller bearbeitet je besuchter Arbeitskraft hoechstens acht Staemme oder
32 Blaetter pro Arbeitsaufruf. Navigation und Handlungen verwenden die vorhandenen
geladenen Weltbereiche und die gemeinsame begrenzte Wegsuche; Arbeitsauftraege
erzwingen kein unbegrenztes Chunk-Laden.

Die erste Fassung ist bewusst begrenzt:

- Bergarbeiter zerstoeren keine Spielerbauten, gespeicherten Edit-Bloecke oder
  geschuetzten Strukturen. Sie untergraben nicht ihren eigenen Standplatz und
  legen keine automatischen Tiefenschaechte an.
- Landwirte bearbeiten vorhandene reife Kulturbeete. Neue Felder umgraben,
  automatische Bewaesserung und eine allgemeine Aussaatplanung fehlen noch.
- Sammler ernten keine Pflanzen oder Baeume; sie transportieren echte Drops.
- Holzfaeller bearbeiten exakt zugeordnete Baeume und pflanzen an der alten Wurzel
  oder in deren unmittelbarer Umgebung nach. Eine allgemeine Forstflaechenplanung,
  Terrainglaettung und neue Wege gehoeren nicht zum Auftrag.
- Die sechs weiteren Stationsberufe haben noch keine produktive Automatisierung.
- Weitergehende Navigation und schwierige Hoehen-/Tuersituationen bleiben F49.

Implementierung: `terra-core::npc_jobs`, `terra-world::npc_work`,
`terra-network::server_npc_work`, `terra-core::profession_stations` und
`terra-ui::npc_work` mit der TiltUI-Komponente
`src-ui/components/npc-work-panel/npc-work-panel.component.*`.
