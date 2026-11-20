use std::{
    collections::{BTreeMap, HashMap},
    sync::OnceLock,
};

pub(super) fn labels(english: bool) -> BTreeMap<String, String> {
    [
 ("title","Building Editor","Gebäude-Editor"),("resume","Resume","Fortsetzen"),("options","Options","Optionen"),("load","Load JSON","JSON laden"),("save","Save JSON","JSON speichern"),("quit","Quit editor","Editor beenden"),("new","New building","Neues Gebäude"),("items","Inventory / E","Inventarkatalog / E"),("menu","Menu / Esc","Menü / Esc"),("all","All items","Alle Items"),("search","Search items…","Items suchen…"),("inventory","Real item catalog","Spielinventar"),("back","Back","Zurück"),("parent","Parent directory","Übergeordnet"),("directory","Directory","Verzeichnis"),("name","Building name","Gebäudename"),("filename","JSON filename (without extension)","JSON-Dateiname (ohne Endung)"),("write","Save building","Gebäude speichern"),("confirm_overwrite","Replace this JSON file?","Diese JSON-Datei überschreiben?"),("overwrite","Confirm overwrite","Überschreiben bestätigen"),("cancel","Cancel","Abbrechen"),("discard","Discard unsaved changes?","Ungespeicherte Änderungen verwerfen?"),("discard_yes","Discard changes","Änderungen verwerfen"),("quit_question","Quit and discard unsaved changes?","Beenden und ungespeicherte Änderungen verwerfen?"),("speed","Flight speed (m/s)","Fluggeschwindigkeit (m/s)"),("sensitivity","Mouse sensitivity","Mausempfindlichkeit"),("platform_size","Support platform size","Größe der Bauhilfe"),("platform_hint","The protected Struct support platform can only be resized here. It is never included in the building JSON.","Die geschützte Struct-Bauhilfe lässt sich nur hier vergrößern. Sie gehört niemals zum Gebäude-JSON."),("platform_schema_hint","This changes only the support platform; the gold schema-1 building outline stays unchanged.","Dies ändert nur die Bauhilfe; der goldene Schema-1-Bauumriss bleibt unverändert."),("language","Language","Sprache"),("window","Window","Fenster"),("windowed","Windowed","Fenstermodus"),("borderless","Borderless fullscreen","Randloses Vollbild"),("vsync","Toggle VSync","VSync umschalten"),("default","Load default village house","Standard-Dorfhaus laden"),("previous","Previous","Zurück"),("next","Next","Weiter"),("blocks","authored blocks","Gebäudeblöcke"),("modified","Unsaved changes","Ungespeicherte Änderungen"),("saved","Unmodified","Unverändert"),("controls","Mouse: look · WASD: fly · Space/Shift: up/down · Left/right click: break/place","Maus: umsehen · WASD: fliegen · Leertaste/Shift: steigen/sinken · Links-/Rechtsklick: abbauen/setzen"),("envelope","Gold outline: 9×12×11 authoring envelope; axes mark the anchor. Foundation at Y −2, floor at Y −1, rooms above Y 0. The protected Struct platform at Y −3 is resized in Options and excluded from JSON.","Goldener Umriss: 9×12×11 Bauvolumen; die Achsen markieren den Anker. Fundament auf Y −2, Boden auf Y −1, Räume ab Y 0. Die geschützte Struct-Fläche auf Y −3 wird in den Optionen vergrößert und nicht exportiert."),("phases","Build phases and exact material costs are derived from your placed blocks. Doors/beds cost one item per pair; a double chest costs two chests.","Bauphasen und genaue Materialkosten entstehen aus den gesetzten Blöcken. Türen/Betten kosten ein Item pro Paar, Doppeltruhen zwei Truhen."),("filehint","Choose a folder or a JSON file. Existing files require confirmation before replacement.","Ordner oder JSON-Datei wählen. Vorhandene Dateien werden erst nach Bestätigung ersetzt."),
 ].into_iter().map(|(key,en,de)|(key.into(),if english{en}else{de}.into())).collect()
}
// Reuse the game's source translations rather than maintaining an alternate item catalog.
pub(super) fn item_name(source: &str, english: bool) -> String {
    static TRANSLATIONS: OnceLock<HashMap<String, String>> = OnceLock::new();
    if english {
        return source.into();
    }
    let catalog = TRANSLATIONS.get_or_init(|| {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-ui/locales/source");
        let mut result = HashMap::new();
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if entry.path().extension().is_none_or(|e| e != "json") {
                    continue;
                }
                if let Ok(source) = std::fs::read_to_string(entry.path())
                    && let Ok(rows) = serde_json::from_str::<Vec<serde_json::Value>>(&source)
                {
                    for row in rows {
                        if let (Some(key), Some(value)) = (
                            row.get("source").and_then(|v| v.as_str()),
                            row.get("de").and_then(|v| v.as_str()),
                        ) {
                            result.insert(key.into(), value.into());
                        }
                    }
                }
            }
        }
        result
    });
    if let Some(translated) = catalog.get(source) {
        return translated.clone();
    }
    // The real inventory appends its canonical species slug to some native
    // names. Translate the existing authored base label, preserving that identity.
    if let Some((base, suffix)) = source.rsplit_once(" (")
        && let Some(species) = suffix.strip_suffix(')')
        && matches!(species, "oak" | "birch" | "spruce" | "cherry")
        && let Some(translated) = catalog.get(base)
    {
        let title = match species {
            "oak" => "Oak",
            "birch" => "Birch",
            "spruce" => "Spruce",
            _ => "Cherry",
        };
        let wood = catalog.get(title).map_or(species, String::as_str);
        return format!("{translated} ({wood})");
    }
    source.into()
}

#[cfg(test)]
mod tests {
    use super::item_name;
    #[test]
    fn real_inventory_wood_suffixes_keep_their_identity_and_translate_the_authored_base() {
        assert_eq!(
            item_name("Birch Planks (birch)", false),
            "Birkenbretter (Birke)"
        );
        assert_eq!(
            item_name("Spruce Planks (spruce)", false),
            "Fichtenbretter (Fichte)"
        );
        assert_eq!(
            item_name("Birch Planks (birch)", true),
            "Birch Planks (birch)"
        );
        assert_eq!(
            item_name("A player name (custom)", false),
            "A player name (custom)"
        );
    }
}
