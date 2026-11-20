use super::{LocalizedUi, catalog::Catalog};
use bevy::prelude::*;
use serde_json::Value;
use std::{collections::BTreeMap, sync::OnceLock};
use tilt_ui::{SharedValueRegistration, UiLocalization};

#[derive(Resource, Default)]
pub(super) struct Projection {
    pub(super) catalog: Option<Catalog>,
    locale: String,
    source: BTreeMap<String, Value>,
}

pub(super) fn refresh(world: &mut World) {
    if !world.contains_resource::<UiLocalization>() {
        return;
    }
    static REGISTRATIONS: OnceLock<Vec<&'static SharedValueRegistration>> = OnceLock::new();
    let registrations = REGISTRATIONS.get_or_init(|| {
        tilt_ui::inventory::iter::<SharedValueRegistration>
            .into_iter()
            .filter(|r| r.key != "LocalizedUi")
            .collect()
    });
    world.resource_scope(|world, mut projection: Mut<Projection>| {
        if projection.catalog.is_none() {
            match Catalog::embedded() {
                Ok(catalog) => projection.catalog = Some(catalog),
                Err(error) => {
                    error!(%error, "Invalid localization index");
                    return;
                }
            }
        }
        let Some(localization) = world.get_resource_ref::<UiLocalization>() else {
            return;
        };
        let locale = localization.locale().to_string();
        let changed = projection.locale != locale || localization.is_changed();
        let mut updates = Vec::new();
        for registration in registrations {
            if !changed
                && !(registration.changed)(world)
                && world
                    .resource::<LocalizedUi>()
                    .roots
                    .contains_key(registration.alias)
            {
                continue;
            }
            let Some(raw) = (registration.snapshot)(world) else {
                if world
                    .resource::<LocalizedUi>()
                    .roots
                    .contains_key(registration.alias)
                {
                    updates.push((registration.alias, None));
                }
                projection.source.remove(registration.key);
                continue;
            };
            if !changed && projection.source.get(registration.key) == Some(&raw) {
                continue;
            }
            let mut value = raw.clone();
            if let Some(catalog) = &projection.catalog {
                localize(&mut value, registration.key, "", catalog, &localization);
            }
            updates.push((registration.alias, Some(value)));
            projection.source.insert(registration.key.to_owned(), raw);
        }
        if updates.is_empty() && !changed {
            return;
        }
        let messages = changed.then(|| {
            projection
                .catalog
                .as_ref()
                .map(|catalog| {
                    catalog
                        .messages
                        .iter()
                        .map(|(alias, key)| {
                            (alias.clone(), super::translate(&localization, key, key))
                        })
                        .collect()
                })
                .unwrap_or_default()
        });
        projection.locale = locale;
        let mut view = world.resource_mut::<LocalizedUi>();
        for (alias, value) in updates {
            if let Some(value) = value {
                view.roots.insert(alias.to_owned(), value);
            } else {
                view.roots.remove(alias);
            }
        }
        if let Some(messages) = messages {
            view.messages = messages;
        }
    });
}

fn preserved(root: &str, path: &str, field: &str) -> bool {
    if matches!(
        field,
        "id" | "item_id"
            | "identifier"
            | "detail_identifier"
            | "detail_registry_name"
            | "registry_name"
            | "value"
            | "key"
            | "command"
            | "input"
            | "search"
            | "draft"
            | "author"
            | "token"
            | "coordinates"
            | "address"
            | "profile"
            | "profile_name"
            | "player"
            | "owner"
            | "nickname"
            | "world_name"
            | "server_name"
            | "train_name"
            | "waypoint_name"
            | "kingdom_name"
            | "founder_name"
            | "sender"
    ) || field.contains("style")
        || field.contains("preview")
        || field.ends_with("_id")
    {
        return true;
    }
    if matches!(root, "WorldsView" | "ServersView")
        && path == ".cards"
        && matches!(field, "title" | "name")
    {
        return true;
    }
    if root == "SessionView" && field == "title" {
        return true;
    }
    if root == "TrainView" && path.is_empty() && field == "name" {
        return true;
    }
    if root == "MultiplayerHud" && field == "name" {
        return true;
    }
    if matches!(root, "KingdomView" | "KingdomResourcesView")
        && matches!(field, "name" | "parent_name" | "founder")
    {
        return true;
    }
    if root == "BuildModeView" && path.is_empty() && field == "title" {
        return true;
    }
    if root == "FullMapView"
        && matches!(
            field,
            "name" | "diplomacy_name" | "diplomacy_actor" | "diplomacy_owner"
        )
    {
        return true;
    }
    if matches!(root, "DialogView" | "CharacterView")
        && matches!(
            field,
            "name" | "world_name" | "server_name" | "address" | "seed"
        )
    {
        return true;
    }
    if matches!(field, "name" | "title")
        && (root == "NpcWorkView"
            || root == "RealmTitleView"
            || path.contains("peers")
            || path.contains("members")
            || path.contains("waypoints")
            || path.contains("realms")
            || path.contains("relations"))
    {
        return true;
    }
    false
}

fn foreign_identifier(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .and_then(|identifier| identifier.split_once("::"))
        .is_some_and(|(namespace, name)| {
            !namespace.is_empty() && !name.is_empty() && namespace != "teramic"
        })
}

pub(super) fn localize(
    value: &mut Value,
    root: &str,
    path: &str,
    catalog: &Catalog,
    localization: &UiLocalization,
) {
    match value {
        Value::Object(fields) => {
            // Player-authored chat and mentions are never translated, even if they equal a UI label.
            if root == "ChatView"
                && fields
                    .get("author")
                    .and_then(Value::as_str)
                    .is_some_and(|author| !author.is_empty())
            {
                return;
            }
            let custom_train =
                root == "TrainView" && fields.get("custom_name") == Some(&Value::Bool(true));
            let kingdom_title = root == "KingdomView"
                && path.is_empty()
                && fields.get("bound") == Some(&Value::Bool(true));
            let custom_item = foreign_identifier(fields.get("identifier"));
            let custom_details = foreign_identifier(fields.get("detail_identifier"));
            for (field, value) in fields {
                // These captions use known prefixes around raw names. Fluent only
                // translates the prefix and keeps captured names verbatim.
                if root == "FullMapView"
                    && ((field == "diplomacy_owner"
                        && value.as_str().is_some_and(|text| {
                            text.starts_with("Founder: ")
                                || matches!(
                                    text,
                                    "NPC settlement affiliation" | "Independent settlement"
                                )
                        }))
                        || (field == "diplomacy_actor"
                            && value
                                .as_str()
                                .is_some_and(|text| text.starts_with("Claimed by "))))
                {
                    if let Value::String(text) = value {
                        *text = catalog.text(localization, text);
                    }
                    continue;
                }
                let custom_metadata = (custom_item
                    && matches!(field.as_str(), "name" | "description" | "label"))
                    || (custom_details
                        && matches!(field.as_str(), "detail_name" | "detail_description"));
                let protected = preserved(root, path, field)
                    || (custom_train && field == "label")
                    || (kingdom_title && field == "title")
                    || custom_metadata;
                if !protected {
                    localize(
                        value,
                        root,
                        &format!("{path}.{field}"),
                        catalog,
                        localization,
                    );
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                localize(value, root, path, catalog, localization);
            }
        }
        Value::String(text) => *text = catalog.text(localization, text),
        _ => {}
    }
}
