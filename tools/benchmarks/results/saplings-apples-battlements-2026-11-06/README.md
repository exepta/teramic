# Setzlinge, Apfelbäume und verbundene Zinnen (200)

Die Umsetzung folgt den [fünf Setzlingsarten](../../../../docs/references/tree-saplings.png),
dem [Apfelbaum samt Apfel/Butzen](../../../../docs/references/apple-tree-items.png)
und den [markierten Mauerverbindungen](../../../../docs/references/battlements-connections.png).

Auf ausdrücklichen Nutzerwunsch werden keine Grafiktests, Spielfenster oder
GPU-Galerien gestartet. Die Abnahme verwendet ausschließlich kopflose Prüfungen.

## Bot-Reiche

| Haltung | Bauen/Abbauen | Türen/Fenster | Andere Interaktionen |
| --- | --- | --- | --- |
| Feindlich | gesperrt | gesperrt | gesperrt |
| Neutral | gesperrt | erlaubt | gesperrt |
| Freundlich | erlaubt | erlaubt | erlaubt |

Die Grundhaltung gilt auch für Spieler ohne eigenes Reich. Eine erklärte
Feindschaft überstimmt die Freundlichkeit; ein angenommenes Bündnis macht ein
neutrales Bot-Reich freundlich. Erfasste Spielerclaims und übernommene Siedlungen
verwenden weiterhin ihre Mitgliederrollen. Handelszugriffe auf Villager prüfen
Inventarrechte. Persönliche Gegenstände im eigenen Inventar bleiben benutzbar.

## Setzlinge und Obst

F59 umfasst Oak, Spruce, Birch, Cherry und Apple mit unterscheidbaren
Setzlingsmodellen, Platzierung, Wachstumsbedingungen und tatsächlichen
artgerechten Bäumen. Freier Raum und Rechte werden vor dem Wachstum geprüft;
Spielerbauten werden erhalten. Fortschritt, Baumart und genaue Holz-/Laubzellen
bleiben gespeichert. Manuell ersetzte Zellen verlieren ihre Baumzuordnung.

| Art | Aktive Wachstumssekunden | Wurzelabstand |
| --- | --- | --- |
| Oak | 900 | 5 Blöcke |
| Spruce | 1500 | 4 Blöcke |
| Birch | 1100 | 4 Blöcke |
| Cherry | 1200 | 5 Blöcke |
| Apple | 1400 | 5 Blöcke |

Grass, Dirt und Farmland tragen Setzlinge. Ein freier Tageslichtschacht oder
unverschattetes nahes Fackel-/Laternenlicht ermöglicht Fortschritt. Bei fehlenden
Chunks, blockiertem Raum, Wasser oder entzogenen Rechten pausiert das Wachstum.
Vollständige Fällung liefert einen passenden Setzling; attribuiertes Laub hat eine
zusätzliche Chance von 1/16. Die Baumzuordnung unterscheidet insbesondere Oak und Spruce,
deren natürliches Laub dieselbe Materialnummer verwenden kann.
Bei historischen Fichten und Kirschbäumen mit Oak-Holz bleibt die tatsächliche
Generatorart maßgeblich. Sowohl Blattdrops als auch vollständige Fällungen
liefern dadurch weiterhin artgerechte Setzlinge.

F60 ergänzt den Apfelbaum mit Blüten, unreifen und reifen Früchten,
Jahreszeiten und erneutem Fruchtansatz. Die Ernte erfolgt über den Server;
gepflückte Fruchtplätze und Revisionen bleiben gespeichert. Apfel und Butzen
besitzen passende Modelle und Inventareinträge. Äpfel sind Nahrungsmittel und
Rezeptzutaten; Setzlinge sind auch im Überlebensmodus erhältlich.

Der Apfelbaum verwendet vorhandenes Oak-Holz und eigenes gelbgrünes Laub (195).
Natürliche Vorkommen ersetzen 14 Prozent geeigneter Oak-/Autumn-Kandidaten in
warmgemäßigten Plains-/Forest-Gebieten. Im Frühjahr erscheinen Blüten, im frühen
Sommer grüne Früchte, ab dem mittleren Sommer und im Herbst reife Äpfel; im
Winter ruht der Fruchtansatz. Acht gespeicherte Plätze verhindern erneute Ernte
desselben Apfels. Ohne Jahreszeiten gilt ein vier Tage langer Fruchtzyklus.
Rechtsklick erntet einzeln; volles Inventar, falsche Revision, verdeckte Frucht,
fehlendes Laub oder fehlende Rechte ändern weder Inventar noch Baum.

Beim Verzehr bleibt ein echter Butzen übrig. Ein Apfel oder zwei Butzen ergeben
zusammen mit Dirt einen Apple-Setzling. Modelle für ganze und abgegessene Äpfel
werden für Weltfrüchte, Icons, Hand und Drops wiederverwendet. Bestehende Ess- und
Laubgeräusche begleiten Verzehr und bestätigte Ernte.

Neue natürliche Apfelvorkommen verwenden die gespeicherte Ökologierevision 3.
Welten mit älteren Revisionen behalten ihre Generierung. Platzierte Setzlinge
und gepflanzte Bäume funktionieren auch dort.

Wachstum bearbeitet höchstens 16 Timer und einen Geometrieversuch je zwei
Sekunden. Fruchtsimulation besucht höchstens 32 Baumdatensätze je 250 ms und speichert
die Slotgeometrie. Unveränderte Ticks verändern weder ChunkStore noch normale
Pflanzenlisten. Geerntete Früchte werden sofort aus der Pick-Geometrie entfernt.
Enge gespeicherte Baumgrenzen vermeiden volle Offsetscans bei gewöhnlichen
Blockänderungen. Entfernte Stämme können durch gleiches Spielerholz keine alte
Fruchtidentität wiederbeleben.

Protokoll 108 synchronisiert die nächsten 256 lebenden Baumdatensätze. Index ID11
speichert Arten, Fortschritt, Zellen, Grenzen und teilweise geerntete Früchte;
vorherige Indizes werden migriert. Unveränderte Spielstände ohne Baumdaten
können weiterhin ID10 verwenden. PLY7 bleibt erhalten.

## Mauerverbindungen

Die vorhandenen Kronen- und Zinnenblöcke verbinden sich anhand der tatsächlichen
Form und Ausrichtung ihrer Nachbarn. Das Eckstück besitzt zwei Kronen, um die
markierten Öffnungen zu schließen. Nachbarprofile werden gemeinsam für Mesh,
Auswahl, Kollision und Wasser verwendet. Bestehende Materialnummern und Posen
bleiben erhalten; Sektionsgrenzen und Änderungen an Nachbarn werden berücksichtigt.

## Prüfung

4.012 Workspace-Library-/Bin-Tests bestanden, keine fehlgeschlagen, neun
bestehende Tests ignoriert. Alle zwölf Testsuiten sind vollständig durchgelaufen.

| Prüfung | Ergebnis |
| --- | --- |
| `cargo test --workspace --lib --bins --locked --no-fail-fast -j1 -- --test-threads=4` | [4.012 bestanden](logs/workspace-tests.log) |
| `cargo build --workspace --locked -j1` | [Erfolgreich](logs/workspace-build.log) |
| `cargo clippy --workspace --all-targets --locked -j1 -- -D warnings` | [Ohne Warnungen](logs/workspace-clippy.log) |
| `cargo fmt --all -- --check` und `git diff --check` | Erfolgreich |
| Tatsächlicher TiltUI-Parser | [60 Stylesheets gültig](logs/ui-stylesheets.log) |

Die Tests prüfen echte Servertransaktionen, UDP-Clients und spätere Beitritte,
Datei-Roundtrips mit teilweise geernteten Früchten, alle fünf Wachstumsformen,
blockierten Raum und entzogene Rechte. Die [gezielte Legacy-Prüfung](logs/legacy-species.log)
prüft historische Fichten und rosa/weiße Kirschbäume über tatsächliche Blattdrops
und vollständige Fällungen; sie ist auch im Gesamtergebnis enthalten.

Mauertests lesen die ausgelieferten glTF-/BIN-Positionen und gleichen sie auf der
CPU mit Auswahl und Kollisionsprofilen ab. Alle 16 Anschlussmasken und 24 Posen,
Nachbaränderungen, Sektionsgrenzen und Wasserräume werden geprüft.

Die Cargo-Läufe verwenden einen Job, niedrige Prozess-/I/O-Priorität und entfernen
`DISPLAY`/`WAYLAND_DISPLAY` aus ihrer Umgebung. Es wurden keine Grafikgeräte,
Spielfenster oder Laufzeit-Screenshots für diese Abnahme gestartet.

[Maschinenlesbare Ergebnisse und Quell-/Log-Prüfsummen](verification.json)
werden durch [collect-verification.py](collect-verification.py) aus den finalen
Logs erzeugt. F59/F60 sind abgeschlossen; NPC-Arbeitsgebiete und produktive
Baumfäller bleiben unter F58/F61 offen.
