use super::{LocalizedUi, catalog::Catalog, preferences, projection};
use bevy::prelude::*;
use fluent_bundle::FluentArgs;
use serde_json::json;
use std::path::{Path, PathBuf};
use terra_core::{Config, GameLanguage, GameplayConfig, LanguagePreference, PlayerProfile};
use tilt_ui::UiLocalization;

const GERMAN: &str = include_str!("../../../../src-ui/locales/de-DE.ftl");
const ENGLISH: &str = include_str!("../../../../src-ui/locales/en-US.ftl");

#[test]
fn multiline_tooltip_statistics_translate_each_line_without_swallowing_following_fields() {
    let catalog = Catalog::embedded().unwrap();
    let source = "\nMaximum health: +2\nMaximum stamina: +3";
    assert_eq!(
        catalog.text(&localization("de-DE"), source),
        "\nMaximales Leben: +2\nMaximale Ausdauer: +3"
    );
    assert_eq!(catalog.text(&localization("en-US"), source), source);
}

#[test]
fn parameterized_messages_preserve_large_world_seeds_and_already_formatted_values() {
    let catalog = Catalog::embedded().unwrap();
    let pattern = catalog
        .patterns
        .iter()
        .find(|pattern| {
            pattern.arguments.len() == 1
                && pattern.literals.iter().any(|literal| literal == "Block ")
        })
        .unwrap();
    let source = format!(
        "{}18446744073709551615{}",
        pattern.literals[0], pattern.literals[1]
    );
    for language in ["de-DE", "en-US"] {
        assert!(
            catalog
                .text(&localization(language), &source)
                .contains("18446744073709551615")
        );
    }
}

pub(super) fn localization(locale: &str) -> UiLocalization {
    let mut localization = UiLocalization::new("en-US").unwrap();
    localization.insert_ftl("de-DE", GERMAN).unwrap();
    localization.insert_ftl("en-US", ENGLISH).unwrap();
    localization.set_locale(locale).unwrap();
    localization
}

#[test]
fn embedded_catalogs_parse_and_format_every_indexed_message_in_both_locales() {
    assert!(super::configuration().is_some());
    let catalog = Catalog::embedded().unwrap();
    assert!(!catalog.exact.is_empty());
    assert!(!catalog.messages.is_empty());
    assert!(!catalog.patterns.is_empty());
    for locale in ["de-DE", "en-US"] {
        let localization = localization(locale);
        for (source, key) in &catalog.exact {
            assert!(
                localization.translate(key, None).is_some(),
                "Missing or invalid {locale} translation for {source:?} ({key})"
            );
        }
        for key in catalog.messages.values() {
            assert!(
                localization.translate(key, None).is_some(),
                "Invalid static template message {locale}: {key}"
            );
        }
        for pattern in &catalog.patterns {
            let mut args = FluentArgs::new();
            for name in &pattern.arguments {
                args.set(name, "sample-value-42");
            }
            assert!(
                localization.translate(&pattern.id, Some(&args)).is_some(),
                "Invalid parameterized message {locale}: {}",
                pattern.id
            );
        }
    }
}

#[test]
fn fluent_plural_rules_and_missing_messages_use_the_real_fallback() {
    let mut localization = localization("de-DE");
    localization
        .insert_ftl(
            "en-US",
            &format!("{ENGLISH}\nlocalization-test-fallback = English fallback\n"),
        )
        .unwrap();
    assert_eq!(
        localization.translate("localization-test-fallback", None),
        Some("English fallback".into())
    );
    assert_eq!(
        localization.translate("missing-message-for-test", None),
        None
    );
    assert_eq!(
        super::translate(&localization, "missing-message-for-test", "Original source"),
        "Original source"
    );
    for (locale, singular, plural) in [
        ("de-DE", "Eine Arbeitskraft", "Arbeitskräfte"),
        ("en-US", "One worker", "workers"),
    ] {
        localization.set_locale(locale).unwrap();
        for count in [0, 1, 3] {
            let mut args = FluentArgs::new();
            args.set("count", count);
            let expected = if count == 1 {
                singular.to_owned()
            } else {
                format!("{count} {plural}")
            };
            assert_eq!(
                localization.translate("workers_count", Some(&args)),
                Some(expected)
            );
        }
    }
    localization.set_locale("fr-FR").unwrap();
    let catalog = Catalog::embedded().unwrap();
    let key = catalog.exact.get("Arbeitsaufträge").unwrap();
    assert_eq!(localization.translate(key, None).unwrap(), "Work orders");
    assert_eq!(
        catalog.text(&localization, "Unregistered user text: Καλημέρα"),
        "Unregistered user text: Καλημέρα"
    );
}

#[test]
fn live_projection_switches_labels_items_and_errors_without_mutating_canonical_views() {
    use crate::{inventory::InventoryView, menu::MenuView};
    let mut world = World::new();
    world.insert_resource(localization("de-DE"));
    world.init_resource::<LocalizedUi>();
    world.init_resource::<projection::Projection>();
    world.insert_resource(MenuView {
        title: "Arbeitsaufträge".into(),
        ..default()
    });
    world.insert_resource(InventoryView {
        title: "Player Inventory".into(),
        detail_name: "Iron Axe".into(),
        detail_identifier: "teramic::iron_axe#uses=37".into(),
        detail_description: "Haltbarkeit: 37 / 256".into(),
        error: "Cannot save graphics: disk-code-42".into(),
        search: "Save".into(),
        ..default()
    });
    let canonical_menu = serde_json::to_value(world.resource::<MenuView>()).unwrap();
    let canonical_inventory = serde_json::to_value(world.resource::<InventoryView>()).unwrap();
    for (locale, title, item, error) in [
        (
            "de-DE",
            "Arbeitsaufträge",
            "Eisenaxt",
            "Grafikeinstellungen konnten nicht gespeichert werden: disk-code-42",
        ),
        (
            "en-US",
            "Work orders",
            "Iron Axe",
            "Cannot save graphics: disk-code-42",
        ),
        (
            "de-DE",
            "Arbeitsaufträge",
            "Eisenaxt",
            "Grafikeinstellungen konnten nicht gespeichert werden: disk-code-42",
        ),
    ] {
        world
            .resource_mut::<UiLocalization>()
            .set_locale(locale)
            .unwrap();
        projection::refresh(&mut world);
        let projected = world.resource::<LocalizedUi>();
        assert_eq!(projected.roots["menuView"]["title"], title);
        let inventory = &projected.roots["inventoryView"];
        assert_eq!(inventory["detail_name"], item);
        assert_eq!(inventory["error"], error);
        assert_eq!(inventory["search"], "Save");
        assert_eq!(inventory["detail_identifier"], "teramic::iron_axe#uses=37");
        assert!(
            inventory["detail_description"]
                .as_str()
                .unwrap()
                .contains("37 / 256")
        );
        assert_eq!(
            serde_json::to_value(world.resource::<MenuView>()).unwrap(),
            canonical_menu
        );
        assert_eq!(
            serde_json::to_value(world.resource::<InventoryView>()).unwrap(),
            canonical_inventory
        );
    }
}

#[test]
fn player_authored_text_ids_and_input_values_are_preserved_even_when_they_match_labels() {
    let catalog = Catalog::embedded().unwrap();
    let localization = localization("de-DE");
    let mut settings = json!({
        "title": "Save", "id": "Save", "item_id": "Save", "identifier": "Save",
        "value": "Save", "key": "Save", "command": "Save", "input": "Save",
        "search": "Save", "draft": "Save", "preview_key": "Save", "layout_style": "Save",
        "token": "Save", "coordinates": "Save", "address": "Save",
        "profile": "Save", "profile_name": "Save", "amount": 7
    });
    let canonical = settings.clone();
    projection::localize(&mut settings, "OptionsView", "", &catalog, &localization);
    assert_eq!(settings["title"], "Speichern");
    for (field, value) in canonical.as_object().unwrap() {
        if field != "title" {
            assert_eq!(
                &settings[field], value,
                "Changed input or identifier: {field}"
            );
        }
    }
    let mut chat = json!({"draft":"Save", "rows": [
        {"id":"Save", "author":"Save", "text":"Save", "spans":[{"text":"Save"}]},
        {"id":"Save", "author":"", "text":"Save"}
    ]});
    let canonical_player_row = chat["rows"][0].clone();
    projection::localize(&mut chat, "ChatView", "", &catalog, &localization);
    assert_eq!(chat["draft"], "Save");
    assert_eq!(chat["rows"][0], canonical_player_row);
    assert_eq!(chat["rows"][1]["text"], "Speichern");
    assert_eq!(chat["rows"][1]["id"], "Save");
    for (root, user_fields) in [
        (
            "KingdomView",
            json!({"name":"Save", "parent_name":"Save", "founder":"Save", "members":[{"name":"Save"}]}),
        ),
        ("NpcWorkView", json!({"name":"Save"})),
        (
            "WorldsView",
            json!({"cards":[{"id":"Save", "title":"Save"}]}),
        ),
        (
            "ServersView",
            json!({"cards":[{"id":"Save", "title":"Save"}]}),
        ),
        (
            "DialogView",
            json!({"name":"Save", "world_name":"Save", "server_name":"Save"}),
        ),
        ("SessionView", json!({"player":"Save", "title":"Save"})),
        (
            "MultiplayerHud",
            json!({"names":[{"id":"Save", "name":"Save"}], "cursors":[{"name":"Save"}]}),
        ),
        (
            "FullMapView",
            json!({"diplomacy_name":"Save", "diplomacy_actor":"Save", "diplomacy_owner":"Save"}),
        ),
    ] {
        let mut projected = user_fields.clone();
        projection::localize(&mut projected, root, "", &catalog, &localization);
        assert_eq!(
            projected, user_fields,
            "Changed user-authored text in {root}"
        );
    }
}

#[test]
fn world_and_server_cards_preserve_names_while_menu_button_titles_translate() {
    let catalog = Catalog::embedded().unwrap();
    let localization = localization("de-DE");
    for root in ["WorldsView", "ServersView"] {
        let source = json!({
            "cards":[{"id":"Save", "title":"Save", "name":"Save"}],
            "buttons":[{"id":"Save", "title":"Save"}]
        });
        let mut projected = source.clone();
        projection::localize(&mut projected, root, "", &catalog, &localization);
        assert_eq!(projected["cards"], source["cards"]);
        assert_eq!(projected["buttons"][0]["title"], "Speichern");
        assert_eq!(projected["buttons"][0]["id"], "Save");
        assert_eq!(source["buttons"][0]["title"], "Save");
    }
}

#[test]
fn kingdom_founding_titles_translate_but_bound_names_and_diplomacy_names_remain_verbatim() {
    let catalog = Catalog::embedded().unwrap();
    let localization = localization("de-DE");
    let mut unbound = json!({"bound":false,"title":"Found a Village","name":"Save"});
    projection::localize(&mut unbound, "KingdomView", "", &catalog, &localization);
    assert_eq!(unbound["title"], "Dorf gründen");
    assert_eq!(unbound["name"], "Save");
    for title in ["Save", "Found a Village"] {
        let mut bound = json!({"bound":true,"title":title,"parent_name":"Save","founder":"Save"});
        let original = bound.clone();
        projection::localize(&mut bound, "KingdomView", "", &catalog, &localization);
        assert_eq!(bound, original);
    }
    let mut diplomacy = json!({
        "diplomacy_name":"Save",
        "diplomacy_owner":"Founder: Save",
        "diplomacy_actor":"Claimed by Save"
    });
    projection::localize(&mut diplomacy, "FullMapView", "", &catalog, &localization);
    assert_eq!(diplomacy["diplomacy_name"], "Save");
    assert_eq!(diplomacy["diplomacy_owner"], "Gründer: Save");
    assert_eq!(diplomacy["diplomacy_actor"], "Beansprucht von Save");
}

#[test]
fn explicit_foreign_item_namespaces_preserve_authored_metadata_without_disabling_builtin_labels() {
    let catalog = Catalog::embedded().unwrap();
    let localization = localization("de-DE");
    let mut inventory = json!({
        "detail_identifier":"example_mod::iron_axe",
        "detail_name":"Iron Axe",
        "detail_description":"Save",
        "error":"Save",
        "items":[
            {"identifier":"example_mod::axe","name":"Iron Axe","description":"Save"},
            {"identifier":"teramic::iron_axe","name":"Iron Axe","description":"Save"},
            {"identifier":"iron_axe","name":"Iron Axe","description":"Save"},
            {"identifier":"::axe","name":"Iron Axe","description":"Save"}
        ]
    });
    projection::localize(&mut inventory, "InventoryView", "", &catalog, &localization);
    assert_eq!(inventory["detail_name"], "Iron Axe");
    assert_eq!(inventory["detail_description"], "Save");
    assert_eq!(inventory["error"], "Speichern");
    assert_eq!(inventory["items"][0]["name"], "Iron Axe");
    assert_eq!(inventory["items"][0]["description"], "Save");
    for index in 1..4 {
        assert_eq!(inventory["items"][index]["name"], "Eisenaxt");
        assert_eq!(inventory["items"][index]["description"], "Speichern");
    }
    let mut builtin = json!({
        "detail_identifier":"teramic::iron_axe",
        "detail_name":"Iron Axe",
        "detail_description":"Save"
    });
    projection::localize(&mut builtin, "InventoryView", "", &catalog, &localization);
    assert_eq!(builtin["detail_name"], "Eisenaxt");
    assert_eq!(builtin["detail_description"], "Speichern");
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "teramic-locale-test-{}-{}",
            std::process::id(),
            PlayerProfile::default().uuid.simple()
        )))
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn profile_world(directory: &Path, profile: PlayerProfile) -> World {
    let mut world = World::new();
    world.init_resource::<GameplayConfig>();
    world.init_resource::<crate::menu::OptionsDraft>();
    world.init_resource::<preferences::ProfileLocale>();
    world.insert_resource(localization("en-US"));
    world.insert_resource(crate::menu::MenuPaths {
        configs: directory.to_owned(),
        worlds: terra_storage::WorldLibrary {
            root: directory.join("worlds"),
        },
    });
    world.insert_resource(profile);
    preferences::synchronize(&mut world);
    world
}

#[test]
fn profile_preferences_reload_and_switch_by_uuid_without_leaking_between_profiles() {
    let directory = TestDirectory::new();
    let first = PlayerProfile::default();
    let second = PlayerProfile {
        name: first.name.clone(),
        ..default()
    };
    assert_ne!(first.uuid, second.uuid);
    let mut world = profile_world(&directory.0, first.clone());
    assert_eq!(
        world.resource::<GameplayConfig>().language,
        GameLanguage::German
    );
    assert_eq!(
        world.resource::<UiLocalization>().locale().to_string(),
        "de-DE"
    );
    world
        .resource_mut::<crate::menu::OptionsDraft>()
        .gameplay
        .language = GameLanguage::English;
    preferences::save(&world).unwrap();
    drop(world);
    let mut reloaded = profile_world(&directory.0, first.clone());
    assert_eq!(
        reloaded.resource::<GameplayConfig>().language,
        GameLanguage::English
    );
    assert_eq!(
        reloaded.resource::<UiLocalization>().locale().to_string(),
        "en-US"
    );
    reloaded.insert_resource(second.clone());
    preferences::synchronize(&mut reloaded);
    assert_eq!(
        reloaded.resource::<GameplayConfig>().language,
        GameLanguage::German
    );
    assert_eq!(
        reloaded
            .resource::<crate::menu::OptionsDraft>()
            .gameplay
            .language,
        GameLanguage::German
    );
    preferences::save(&reloaded).unwrap();
    reloaded.insert_resource(first.clone());
    preferences::synchronize(&mut reloaded);
    assert_eq!(
        reloaded.resource::<GameplayConfig>().language,
        GameLanguage::English
    );
    assert_eq!(*reloaded.resource::<PlayerProfile>(), first);
    let profile_path = |profile: &PlayerProfile| {
        directory
            .0
            .join("profile-preferences")
            .join(profile.uuid.simple().to_string())
            .join("language.toml")
    };
    assert_eq!(
        LanguagePreference::load(profile_path(&first))
            .unwrap()
            .language,
        GameLanguage::English
    );
    assert_eq!(
        LanguagePreference::load(profile_path(&second))
            .unwrap()
            .language,
        GameLanguage::German
    );
    // The local preference file never modifies or replaces the portable profile.
    assert!(!directory.0.join("profile.toml").exists());
}
