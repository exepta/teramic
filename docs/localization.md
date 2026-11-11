# Deutsch und Englisch (F72)

Unter **Einstellungen → Gameplay/Spiel → Sprache** stehen **Deutsch** und
**English** zur Auswahl. Die Auswahl wird wie die anderen Einstellungen erst
mit **Uebernehmen** aktiv und gespeichert. Verwerfen behaelt die bisherige
Sprache. Auch bereits geoeffnete Ansichten wechseln ihre Texte.

Die Grundeinstellung ist Deutsch. `GameplayConfig.language` akzeptiert in TOML
`"de"` und `"en"`; bisherige Dateien ohne dieses Feld bleiben gueltig.
Zusaetzlich liegt die Auswahl fuer jede lokale Profil-UUID getrennt unter
`<Menukonfiguration>/profile-preferences/<uuid>/language.toml`. Ein Profilwechsel
laedt dessen Auswahl. Serveridentitaet, Spielerinventar und Weltseed werden
dadurch nicht veraendert.

## Kataloge und Darstellung

Die Integration verwendet TiltUIs reales `fluent`-Feature, `UiLocalization`,
`UiFluentConfig` und `UiFluentArgs`. Deutsch und Englisch besitzen dieselben
Nachrichtenschluessel. Fluent uebernimmt Fallback und Pluralregeln; die
Arbeitskraefte-Anzeige verwendet tatsaechlich einen numerischen Pluralparameter.
Fehlende Sprachschluessel fallen auf Englisch zurueck. Nicht bekannte Texte
behalten ihren Originalwert.

Statische Templates verwenden `i18n.*`. Dynamische Ansichten werden als
`LocalizedUi` fuer die Darstellung projiziert. Die urspruenglichen typisierten
Ansichten, Befehle, Auswahlwerte, Inventarslots und Netzwerkdaten bleiben
kanonisch. Auch die Inventar-/TMI- und Rezeptsuche prueft uebersetzte Namen und
Beschreibungen, weiterhin aber auch Originalnamen und Registry-IDs.

Uebersetzt sind Menues, Einstellungen, HUD-/Kartentexte, Berufe, Hinweise und
Tooltips sowie die enthaltenen Block-, Item-, Rezept- und Prop-Bezeichnungen
und Beschreibungen. Vorhandene autoritative Fehlermeldungen und Systemantworten
werden pro Client dargestellt; die Sprache des Servers wird nicht umgeschaltet.
Entwicklerlogs und interne technische Kennungen werden nicht umbenannt.

Spielerchat, Profil-/Spielernamen, benannte Welten, Server, Zuege, Wegpunkte und
Siedlungen werden bewusst erhalten. Fremde Item-Namespaces behalten ohne
eigene Inhaltsuebersetzung ihren Namen und ihre Beschreibung. Es werden keine
fremden Identifikatoren durch ein zufaellig gleichlautendes UI-Wort ersetzt.
Captures in formatierten Meldungen behalten ihre Zeichen unveraendert; grosse
64-Bit-Seeds werden nicht durch eine Gleitkommakonvertierung gerundet.

## Texte ergaenzen

1. Ein passendes JSON unter `src-ui/locales/source/` erweitern. Jeder Eintrag
   enthaelt `source`, `de` und `en`; Rust-Formatparameter muessen in beiden
   Sprachen vollstaendig erhalten bleiben.
2. `python3 tools/localization/build_catalogs.py --migrate-templates` ausfuehren.
3. Die generierten `de-DE.ftl`, `en-US.ftl` und `index.json` mit einchecken.
4. Sprach-/Katalogtests und betroffene UI-Tests ausfuehren. Lange Texte auch
   nativ in einer kleinen Fensterbreite pruefen.

Die Schluessel werden deterministisch aus der Quelle erzeugt und bleiben bei
Dateiumzug und Neusortierung stabil. Der Generator verwirft fehlende
Uebersetzungen, doppelte widerspruechliche Quellen und veraenderte Parameter.
Er laesst Befehls-, Wert- und ID-Bindings unveraendert. Neue einzelne komplexe
Meldungen koennen auch direkt als Fluent-Nachricht mit Argumenten formuliert
werden. Mehrdeutige Texte brauchen konkrete Kontextquellen, beispielsweise
die Gruendung einer Siedlung gegenueber einem gefundenen Ort.

Native Abnahme aktivieren: `TERRA_LOCALIZATION_GALLERY=1` und
`TERRA_MENU_SMOKE_DIR=/absoluter/isolierter/testpfad` mit
`terra-client --smoke-test-menu`. Der Test ruft die vorhandenen TiltUI-Handler
fuer Auswahl/Uebernehmen auf, prueft die gespeicherte Profilpraeferenz und
erstellt Aufnahmen in beiden Sprachen. Er arbeitet mit eigenen Konfigurationen.
