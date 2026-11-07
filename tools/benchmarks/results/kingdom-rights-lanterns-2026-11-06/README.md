# Kingdom-Rechte, Doppeltüren und Laternen (199)

Neue Welten verwenden Strukturrevision **8**. Bereits gespeicherte Welten behalten
ihre Strukturrevision und ihre Bauten. Rollen, Diplomatie, Doppeltüren und manuell
platzierte Laternen/Ketten funktionieren auch in vorhandenen Welten.

## Siedlungen

Kingdoms und Outposts erhalten echte Bäume, Gras, Blumen, Büsche und Farne auf
freien Hofflächen und begrünten Böschungen. Gebäude, Wege, Felder und Türbereiche
bleiben frei. Die Pflanzen verwenden die normalen Ernte-, Streaming- und
Jahreszeitensysteme; die Baumoption der Welt gilt auch für Hofbäume.

Die Verteilung wird aus Seed und Koordinaten bestimmt und hängt nicht von der
Ladereihenfolge ab. Kingdoms sind stärker ausgedünnt als Outposts. Zwischen zwei
Kingdom-Kandidaten liegen mindestens drei Strukturregionen, zwischen zwei
Outpost-Kandidaten mindestens zwei. Eine Region ist 384 Blöcke breit. Der Ursprung
bleibt im Radius von 1536 Blöcken frei von Kingdom-Zentren und von 768 Blöcken frei
von Outpost-Zentren. Die tatsächliche Startsuche hält zusätzlich 1024 bzw. 512
Blöcke Abstand, auch bei einer gewählten Startbiom-Präferenz. Ein deaktivierter
Strukturtyp reserviert keinen Startbereich.

## F57: Rollen und Diplomatie

| Rolle | Bauen | Abbauen | Inventare | NPC-Arbeit | Verwaltung |
| --- | --- | --- | --- | --- | --- |
| Founder | ja | ja | ja | ja | ja |
| Officer | ja | ja | ja | ja | ja, ohne Founder-/Officer-Beförderung |
| Member | ja | ja | ja | ja | nein |
| Builder | ja | ja | ja | nein | nein |
| Worker | nein | nein | ja | ja | nein |
| Visitor | nein | nein | nein | nein | nein |

Neue Mitglieder beginnen als Visitor. Frühere Mitglieder werden beim Laden zu
Member migriert und behalten ihre bisherigen normalen Zugriffe. Rechte werden auf
dem Server bei jeder Aktion geprüft; Entfernen, Rollenwechsel und Besitzwechsel
wirken auch auf bereits geöffnete Lager und laufende NPC-Befehle.

Die Tabelle gilt für geschützte Baugebiete. Das offene **Realm Territory** behält
die bisherige Freigabe zum Bauen und Abbauen gewöhnlicher Blöcke. Private Lager
bleiben geschützt, auch gegen das Ausschütten ihres Inhalts durch Blockabbau.
Das Wegwerfen eigener Gegenstände bleibt möglich.

Die Verwaltung zeigt Mitglieder, Rechte und Beziehungen. Karte und Eintrittstitel
unterscheiden eigenes, neutrales, verbündetes und feindliches Gebiet, ohne die
Wappenfarbe des Besitzers zu ändern. Spielerbündnisse benötigen die Zustimmung
beider Reiche; eine Feinderklärung wirkt sofort. Helteria nimmt Bündnisse an,
Lutera und Betross ab Reichsstufe 2; Petora bleibt feindlich. Die Oberfläche zeigt
ausstehende Angebote getrennt von der wirksamen Beziehung.

Verbündete teilen Inventar- und NPC-Zugriffe im Umfang ihrer eigenen Rolle;
Baurechte werden dadurch nicht vergeben. Förderbänder, Maschinenanschlüsse und
Rohrnetze beachten die Besitzergrenzen auch nach einer Beziehungsänderung. Eine
erlaubte Umleitung durch ein drittes Reich erlaubt keine direkte unerlaubte
Abkürzung. Aufgegebene Claims, entfernte Mitglieder, entladene Nachbarchunks und
bereits gestartete Baumarbeiten werden berücksichtigt.

Client und Server verwenden Protokoll **107**. Der neue Index **ID10** speichert
Rollen und Beziehungen; alte Indizes werden migriert. **PLY7** bleibt erhalten.
Die zusätzlichen produktiven NPC-Aufträge und deren Berufsverwaltung bleiben
separat in F58.

## Türen und Requisiten

Zwei passende nebeneinanderstehende Türen bilden eine Doppeltür mit gemeinsamem
Rahmen und entgegengesetzten äußeren Scharnieren. Bedienung ändert beide Flügel
zusammen; Kollision, Auswahl, Wasser und NPC-Durchgänge verwenden dieselbe
Geometrie. Beim Abbau bleibt der andere Flügel als Einzeltür erhalten. Alle vier
Holzarten und alle Platzierungsrichtungen sind abgedeckt.

Die [Laternenreferenz](../../../../docs/references/lantern-chain.png) ist als
Voxelmodell mit Metallrahmen, warmem Glas und echtem Licht umgesetzt. Die Laterne
steht auf Blöcken oder Zaunpfosten, hängt an Decken/Ketten und erhält an Wänden
eine Halterung. Ketten sind separat herstellbar und platzierbar. Beide Gegenstände
haben Rezepte und Einträge im Itemkatalog. Neue Dörfer, Kingdoms und Outposts
verwenden Laternen anstelle ihrer bisherigen Fackeln. Wasser zerstört Laternen;
Ketten lassen sich mit Wasser umschließen. Fehlende Halterungen lösen die normalen
Abbau- und Drop-Regeln aus.

## Prüfung

**3.963 Workspace-Library-/Bin-Tests bestanden, neun ignoriert.** Build und
Clippy aller Targets mit `-D warnings` sind erfolgreich; Format, Diff und alle
60 echten UI-Stylesheets sind geprüft.

Die Regressionen prüfen unter anderem die tatsächliche Startsuche mit
Biompräferenz, seltene koordinatenstabile Standorte, alle drei begrünten Baupläne,
Erntekennungen und veränderten Boden, echte Laternenmodelle einschließlich aller
Auswahlflächen, fehlende/entladene Halterungen, echte Doppeltür-Transaktionen,
Rollenentzug bei geöffneten Lagern, NPC-Befehle und Bündnisse über UDP sowie
Migration des eingefrorenen IDX9-Formats. Rohrprüfungen unterscheiden ausdrücklich
eine verbotene gemeinsame Fläche von einer tatsächlich erlaubten Umleitung.

```bash
cargo test --workspace --lib --bins --locked --no-fail-fast -j2 -- --test-threads=8
cargo clippy --workspace --all-targets --locked -j2 -- -D warnings
cargo build --workspace --locked -j2
cargo fmt --all -- --check
git diff --check
```

## Native Ansichten

| Ansicht | Aufnahme |
| --- | --- |
| Laternen bei Tag / Nacht | [Tag](lanterns/lantern-gallery-day.png), [Nacht mit echtem Licht](lanterns/lantern-gallery-night.png) |
| Drei Befestigungen | [Wand](lanterns/lantern-wall-close.png), [Tischfuß](lanterns/lantern-table-close.png), [Kette und Decke](lanterns/lantern-chain-close.png) |
| Doppeltüren | [Geschlossen](doors/doors-closed.png), [offen](doors/doors-open.png), [ein Flügel entfernt](doors/birch-door-separated.png) |
| Beziehungen auf der Karte | [Alle vier Beziehungen](claims/diplomacy-claims.png), [lebende Änderung mit gleicher Besitzerfarbe](claims/diplomacy-live-color.png), [kompakte Ansicht](claims/diplomacy-compact.png) |
| Rechteverwaltung | [Rollenauswahl](rights/kingdom-role-choices.png), [Worker](rights/kingdom-rights-worker.png), [entzogene Rechte](rights/kingdom-rights-revoked.png) |
| Diplomatieverwaltung | [Bestätigtes Bündnis](rights/kingdom-diplomacy-allied.png), [ausstehendes Angebot](rights/kingdom-diplomacy-pending.png), [authentifizierter Visitor](rights/kingdom-diplomacy-readonly.png) |
| Oberflächen bei anderen Auflösungen | [1024 × 768](rights/kingdom-ui-compact.png), [4K](rights/kingdom-ui-4k.png) |
| Begrüntes Kingdom | [Übersicht](native/kingdom-temperate/kingdom-temperate-overview.png), [Nähe](native/kingdom-temperate/kingdom-temperate-close.png) |
| Begrüntes Winter-Kingdom | [Übersicht](native/kingdom-winter/kingdom-winter-overview.png); zusätzliche Nahaufnahme nicht abgenommen |
| Begrünter Outpost | [Übersicht](native/outpost/outpost-overview.png), [Nähe](native/outpost/outpost-close.png) |

Laternenaufnahmen kontrollieren die wirklich erzeugten PointLights, nicht nur die
Emission des Glases. Die Türgalerie prüft auch einen Verbund über eine
Sektionsgrenze und den tatsächlichen Modellwechsel nach Abbau eines Flügels.
Die Kartengalerie projiziert vorübergehend definierte Beziehungen auf einen
isolierten echten Spielstand; die Autorität prüfen die separaten Server-/UDP-Tests.

Die Rechtegalerie benutzt für Beitritt, Rollenvergabe, Entzug und abgelehnte
Selbstbeförderung einen zweiten authentifizierten Client. Anschließend verbindet
sich der gerenderte Client mit dem Visitor-Profil neu und zeigt die tatsächlich
schreibgeschützten Ansichten. Es werden keine bestätigten Rollen oder Identitäten
im Client erfunden. Das automatisierte Gründen wartet auf den echten Bergfried-
Treffer und später bestätigte Eingabesamples.

Die Strukturaufnahmen stammen aus wirklicher Revision-8-Generierung mit Seed
42042. Die normale Temperate-Variante liegt hier in einem Pilzbiom und übernimmt
dessen natürliche Terrainfarben. Eine private Fotokonfiguration mit weiter Sicht
und ohne Nebel zeigt Höfe und Gelände; Spieleinstellungen werden nicht geändert.
Die letzten Winterversuche verwendeten kontinuierliche Hintergrundupdates.

**Keine weiteren Grafiktests:** Auf ausdrücklichen Nutzerwunsch sind alle
grafischen Prüfungen beendet. Es läuft kein Testclient mehr. Sechs native
Galerien sind vollständig bestanden; insgesamt liegen 37 Aufnahmen vor.
Die zusätzliche Winter-Nahaufnahme blieb vor der vollständigen Bereitschaft der
Render-/Prop-Queues im Zeitlimit hängen. Eine echte Übersicht der Region
`[-10,12]` bleibt erhalten; der spätere Versuch in `[51,32]` wurde ohne neue
Aufnahme geschlossen. [Metadaten](native/kingdom-winter/metadata.json) ordnen das
Bild seinem tatsächlichen Ursprungsort zu. Dieser zusätzliche Fotolauf wird
ausdrücklich nicht als bestanden gewertet. Die automatisierten Winter-Bauplan-,
Vegetations- und Zugriffsprüfungen sind bestanden.

Sämtliche veränderlichen Profile, Einstellungen und Prüfwelten bleiben unter
`target/phase199-*`. Der Report enthält Aufnahmen, Runner und Prüfprotokolle.
Die Fensterskalierung kann einen zusätzlichen physischen Pixel erzeugen; die
Metadaten erhalten die tatsächlichen Maße. 4K-Aufnahmen haben 3840 × 2160 Pixel.

Prüfsummen, Bildmaße und Befehle stehen in [verification.json](verification.json).
