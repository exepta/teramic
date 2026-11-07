# F62 — Freier Gleisbau mit einem Gleisitem

Implementierungs- und Bedienbericht vom 07.11.2026. Dieser Bericht beschreibt die
Weiterentwicklung des [modularen Schienensystems](../railway-system-2026-11-07/README.md).
Alle **4.186 Workspace-Library-/Bin-Tests bestehen**, keiner ist fehlgeschlagen;
neun bestehende Tests sind ignoriert. Clippy aller Targets mit `-D warnings` und
die Formatprüfung sind sauber. Der abschließende Workspace-Build ist erfolgreich.
Ausdrücklich wurden keine Grafiktests, Spielclients, GPU-Galerien
oder neuen Screenshots gestartet.

Die Bedienidee orientiert sich an Creates gespeichertem Verbindungsstart,
Endpunkten und daraus berechneter Kurve samt Materialbedarf. Die eingesehene
Primärquelle ist
[Create: TrackPlacement.java, Branch mc1.21.1/dev](https://github.com/Creators-of-Create/Create/blob/mc1.21.1/dev/src/main/java/com/simibubi/create/content/trains/track/TrackPlacement.java).
Die vorhandene [Gestaltungsreferenz](../../../../docs/references/railway-tracks-three-blocks.png)
bleibt für Schwellen, Railprofile, Bolzen und Weichenhebel maßgeblich.

## Ein Item, vier Holzarten

Das gemeinsame Item heißt **Track** und verwendet weiterhin die bestehende ID
**196**, Identifier `teramic::rail_straight`. Too Many Items bietet Eiche, Birke,
Fichte und Kirsche als vier Varianten dieses Items an. Das einzige Gleisrezept
verwendet ein geformtes 3×3-Werkbankmuster mit insgesamt vier Eisenbarren und vier
Planken; es liefert ein Gleisitem. Alle vier Plankenarten sind zulässig. Die beim
Crafting verwendete Holzart wird im Ergebnis gespeichert und beim Platzieren auf
sämtliche belegten Zellen übertragen. Mischrezepte verwenden die vorhandene
allgemeine Holzauswahl des Craftings.

Die neun bisherigen Formrezepte entfallen; der gesamte eingebaute Rezeptkatalog
enthält dadurch 325 Rezepte. Die IDs 197–205 bleiben als interne Bauformen und für
bestehende Welten erhalten, erscheinen aber nicht mehr als zusätzliche Pflichtitems
im Kreativkatalog. Aufnehmen und Survival-Abbau liefern das gemeinsame Gleisitem
mit der gespeicherten Holzart zurück. Vorhandene alte Inventarstapel bleiben lesbar.

Die Holzvarianten wählen die tatsächlichen vorhandenen Plankenmaterialien von
Eiche, Birke, Fichte und Kirsche. Stahlfuß, Steg, Kopf, Bolzen und roter Hebel
behalten ihre eigenen Materialien. Icons, gehaltene Varianten, Drops, Bauvorschau
und platzierte Gleise verwenden denselben Holzentscheid. Der vorhandene
Schrittgeräuschpfad ordnet alle Gleismaterialien der Holz-Kontaktfamilie zu.

## Bedienung

Rechtsklick bezeichnet im folgenden Ablauf die konfigurierte **Place-/Platzieren-Aktion**;
die Beispiele setzen deren Belegung auf die rechte Maustaste voraus. Der HUD-Hinweis
verwendet die tatsächlich konfigurierten Tasten; Use ist standardmäßig R.

1. Ein Gleisitem der gewünschten Holzart halten. Im Standardmodus **Gleis/Auto**
   setzt Rechtsklick auf tragfähigem Boden zunächst ein gerades Startstück. Seine
   Richtung folgt der Blickrichtung.
2. Auf ein freies Ende eines vorhandenen Gleises zeigen und rechtsklicken. Dadurch
   wird der Verbindungsstart gemerkt; diese Auswahl baut noch keinen Bogen.
3. Zum Ziel gehen und auf ein anderes freies Gleisende oder eine Bodenoberseite
   zeigen. Die vollständige Vorschau zeigt die berechnete Verbindung und der HUD
   den Bedarf an Gleisitems. Rechtsklick bestätigt eine gültige Verbindung.
4. **Shift + Rechtsklick** verwirft den gemerkten Start. Wechsel des gehaltenen
   Items, Abbau, blockierte Eingabe oder Wechsel der Bauform verwerfen ihn ebenfalls.
5. **R** beziehungsweise die Aktion **Use** schaltet durch Gleis/Auto, Weiche links,
   Weiche rechts, Kreuzung und Prellbock. Alle Bauformen verbrauchen das gleiche
   Gleisitem und übernehmen dessen Holzart. Es gibt keine Auswahl einzelner
   vorgefertigter Kurvenradien oder Steigungsstücke mehr.
6. Ohne gehaltenes Gleisitem schaltet Rechtsklick auf eine Weiche zwischen
   Hauptstrang und Abzweig. Zungen und roter Hebel verändern ihre Stellung. Shift
   unterdrückt diese gewöhnliche Objektinteraktion.
7. Linksklick baut die vollständige anvisierte Baugruppe ab, auch wenn nur eine
   ihrer Sekundärzellen getroffen wird. Survival berücksichtigt Werkzeug und
   Abbaudauer; Creative erzeugt keine zusätzlichen Survival-Drops.

Mit einem gehaltenen Gleis besitzt Use Vorrang vor Werkbank-, Kisten-, Tür-,
Wasserhahn-, Kurbel- und Bergfriedaktionen. Ein Moduswechsel löst daher keine
zweite Objektaktion aus. Auch länger gehaltenes Use treibt keine Kurbel an. Die
separate Place-Aktion behält die bisherige Objektbedienung und den Gleisanbau bei.

Der aktuelle Zielstrahl bleibt auf **4,5 Meter** begrenzt. Dies gilt für die
Auswahl eines erreichbaren Startports und den jeweils aktuellen Zielklick. Nach
Auswahl des Starts darf man sich zum Ziel bewegen. Eine einzelne freie Verbindung
ist höchstens **32 Meter** lang; die räumliche Entfernung ihrer Enden darf dieses
Limit ebenfalls nicht überschreiten.

Auch der tatsächlich verbundene Zielport muss in 4,5 Metern Reichweite liegen;
ein erreichbarer Klick auf die Mitte eines langen Moduls ersetzt dies nicht.
Start- und bestehende Zielmodule müssen vollständig geladen und über sämtliche
besessenen Zellen baubar sein. Damit kann kein fremder Anschluss allein dadurch
belegt werden, dass die neue Kurve außerhalb des fremden Claims liegt.
Dasselbe gilt beim direkten Andocken von Weichen, Kreuzungen und Prellböcken:
Das gesamte vorhandene Anschlussmodul muss geladen sein, zum Journal passen
und Build-Rechte erlauben.

## Freier Bogen und nachvollziehbare Grenzen

Die Verbindung wird als echte kubische Bézierkurve aus vier Kontrollpunkten
berechnet. Endpositionen und Anschlusstangenten stammen aus den tatsächlichen
Gleisports. Die Punkte sind relativ zum Anker in Sechzehntelblöcken gespeichert.
Der Verlauf zwischen den Enden ist frei berechnet und wird nicht durch Auswahl
eines der zehn alten Modelle ersetzt.

Neue Bodenenden besitzen vier kardinale horizontale Ausrichtungen. Bestehende
Anschlüsse geben ihre tatsächliche Richtung und Neigung vor. Diese bewusste
Rasterbindung der Enden hält angrenzende Mehrzellen-Baugruppen eindeutig getrennt;
der Kurvenverlauf im Inneren bleibt frei. Ein beliebiger seitlicher Klick auf ein
Rail wird nicht zu einem neuen Anschluss mitten auf dem Profil.

Die nominale Gleisbreite bleibt drei Blöcke, die Spurweite zwei Blöcke zwischen den
Rail-Mittellinien. Schwellen sind 2,9 Blöcke breit. Zulässige freie Verbindungen
halten mindestens 4,5 Meter Krümmungsradius und höchstens 25 Prozent Steigung ein.
Zu enge, zu steile, rückwärts verlaufende, selbstschneidende oder überlange Entwürfe
werden abgelehnt. Kurven können seitlich versetzen und gleichzeitig Höhe ändern,
sofern diese gemeinsamen Grenzen und der reale Bauplatz dies erlauben.

Der Materialbedarf beträgt `ceil(Summe der räumlichen Pfadlängen / 4)` Gleisitems,
mindestens eins. Deshalb kostet das ebene Viermeter-Startstück ein Item; längere
Bögen und mehrsträngige Weichen/Kreuzungen benötigen entsprechend mehr. Beim
Survival-Abbau wird derselbe berechnete Bedarf in der gespeicherten Holzart
zurückgegeben. Fehlendes Material lässt die ganze Transaktion unverändert.

## Unterbau, Props und Transaktionen

Der gemeinsame Platzierungsplan prüft alle belegten Zellen, echte Stützpunkte,
Raum und Spielerüberlappung. Der erforderliche Unterbau muss geladen und tragfähig
sein; ein schwebender Bogen wird nicht durch eine rein optische Verbindung
zugelassen. Eine ungültige, noch geometrisch berechenbare Vorschau bleibt rot
sichtbar. Sie verändert keine Blöcke und verbraucht keine Items.

Schienen dürfen mit ihrer tatsächlichen Geometrie berührte zerstörbare Props
freiräumen. Client und Server bestimmen diese Props aus demselben Plan. Rechte
werden sowohl am Support als auch im gesamten berührten Propbereich geprüft;
fehlende Build-/Break-Rechte verhindern die vollständige Platzierung. Geschützte
Früchte und Bienenstöcke blockieren den Bau. Die Authority reserviert vor dem
Commit auch Inventaränderungen und Dropkapazität. Veraltete Weltzustände,
Spielerkollision, unvollständige Baugruppen und fehlende Rechte werden vor einer
Veröffentlichung abgelehnt.

## Gemeinsame Geometrie, Speicher und Netz

`Piece`, seine Kontrollpunkte und die Holzart bilden die gemeinsame Grundlage von
Mesh, Kollision, Auswahl, Wasserprofilen und Bauvorschau. Der Renderer verwendet
die bereinigten Core-Oberflächen und schneidet sie in disjunkte Zellflächen.
Sekundärzellen zeichnen ausschließlich ihren Anteil. Ihre Darstellung und
Kollision benötigen keinen geladenen Ankerchunk. Die Vorschau zeichnet dagegen
einmal die vollständige Baugruppe an ihrem echten Anker. Kurvenspezifikation und
Holzart sind Bestandteile des Rendercache-Schlüssels.

Jede belegte Zelle speichert Ankerzuordnung, Sekundärstatus, Pose, Holzart und bei
freien Bögen identische Kontrollpunkte. Ein unvollständiger Datensatz wird nicht
als zweite Baugruppe oder befahrbare Verbindung akzeptiert. Der Routengraph wird
aus bestätigten vollständigen Datensätzen abgeleitet, auch für nicht geladene
Sections; Routing erzeugt keine Terrainchunks. Weichenstellung und Porttangenten
bestimmen die tatsächlich verbundenen Wege.

**Protocol 112** überträgt freie Verbindungsbefehle und die erweiterten
Objektmetadaten. Die Regionhülle **RG13** ergänzt Kurveninformationen, ohne die
historischen Regionpayloads nachträglich umzudeuten. Alte vierfeldrige
Objektzustände werden über einen expliziten Adapter gelesen; alte Modul-IDs und
Posen bleiben erhalten. Regionen ohne neue Kurvendaten benötigen diese zusätzliche
Hülle nicht. Die Kompatibilität beschreibt das Lesen alter Welten mit dem neuen
Programm, keine Zusage für das Öffnen neuer RG13-Daten mit alten Binärdateien.

Die Arbeit bleibt begrenzt: höchstens 512 Zellen je Bauplan, 256 freigeräumte Props,
8.192 Graphstücke und ein begrenztes Routingbudget. Detaillierte Kurvenanalysen und
Layouts verwenden begrenzte Caches; die Metrikvalidierung besitzt zusätzlich einen
begrenzten Cache für bestätigte Datensätze. Wiederholte Gültigkeits- und
Längenabfragen müssen bekannte Kurven nach Layoutverdrängung nicht erneut
vollständig analysieren.

Die zehn nativen Legacy-GLTFs bleiben für bestehende Module und deren
Materialreferenzen verfügbar. Der kopflose Export übernimmt die echten Core-Daten;
die binären Legacy-Meshes blieben bei dieser Erweiterung bytegleich. Die
Provenienz umfasst jetzt auch `rails/curve.rs`. SHA-256 und FNV64 in
`rail_profiles.json` und GLTF-Metadaten machen spätere Abweichungen prüfbar.

## CPU-Prüfung

Ergänzte Regressionen prüfen insbesondere:

- echte Bézierpositionen und Tangenten, Radius-/Steigungs-/Längenlimits sowie
  deterministische freie Verbindungen;
- identische Kontrollpunkte und Holzart in allen Zellzuständen, vollständige
  Ownership, atomaren Bau/Abbau und zustandsabhängige Weichen;
- Props, Rechte, Reichweite, Inventarbedarf und serverseitige Rekonstruktion;
- Use-Vorrang mit Standard- und geänderter Tastenbelegung vor Stationen,
  Wasserhahn, Kurbel und Bergfried bei weiterhin funktionierender Place-Bedienung;
- vier echte Holzmaterialslots, getrennte Kurven-/Holz-Cacheschlüssel und unveränderte
  Metallmaterialien;
- gleiche Fläche von Gesamtmesh und Zellstücken, Deckung mit gemeinsamen Hulls,
  Sekundärzellen ohne geladenen Anker und einen vollständigen freien Kurven-Ghost;
- ein gemeinsames Crafting-Rezept mit realem Gridmatching in allen vier Holzarten,
  vier Kreativvarianten und das Verbergen alter Formitems;
- RG13-Roundtrips, alte Regionformate und bestätigte Netzwerkzustände.

Der vollständige Workspace-Lauf umfasst zwölf Testsuiten und bestätigt **4.186
bestandene Tests, null Fehler und neun bestehende ignorierte Tests**. Darin sind
die echten UDP-Prüfungen mit zwei Clients, Auswahl eines erreichbaren Startports,
normalem Teleport zum entfernten Ende, Materialverbrauch, Replikation, Late Join
und sekundärem Survival-Abbau enthalten. Der schlanke UDP-Testclient verwendet
einen echten Spielerkörper und die normalen Teleport-/Reconciliation-Systeme;
Teleportflags und Zeitlimits werden nicht umgangen. Eine separate RPC-Prüfung
sichert alle vier Spezialmodi an einer fremden Claimkante ab.

Alle 325 eingebauten Rezeptdateien sind gültiges JSON; es bleibt genau eine
`rail_*.json`-Rezeptdatei. Sämtliche 169 Itemdefinitionen sind eindeutig, die drei
neuen Holzvarianten besitzen vollständige Metadaten. Der reale Kreativkatalogtest
lädt und prüft diese Dateien ebenfalls. Der kopflose Assetexport und die
Quell-Provenienzaktualisierung sind ausgeführt. Die binären Legacy-Meshes sind
unverändert.

Clippy aller Workspace-Targets mit `-D warnings`, Formatprüfung und Diff sind
sauber. Der abschließende Workspace-Build hat aktuelle Client-/Serverdateien
erfolgreich erstellt. Die vollständigen Logs und SHA-256-Prüfsummen stehen in
[verification.json](verification.json). Eine grafische Sichtprüfung ist auf
Nutzerwunsch ausdrücklich nicht Teil dieses Abschlusses.
