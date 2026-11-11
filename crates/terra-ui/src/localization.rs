//! Fluent text belongs to presentation. Canonical state, commands and user text stay intact.
mod catalog;
mod preferences;
mod projection;
#[cfg(test)]
mod tests;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use tilt_ui::{UiFluentConfig, UiLocalization, html_shared};

#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct LocalizedUi {
    #[serde(flatten)]
    pub roots: BTreeMap<String, Value>,
    pub messages: BTreeMap<String, String>,
}

pub(super) fn configuration() -> Option<UiFluentConfig> {
    match UiFluentConfig::new("en-US")
        .and_then(|c| c.with_catalog("de-DE", "locales/de-DE.ftl"))
        .and_then(|c| c.with_catalog("en-US", "locales/en-US.ftl"))
    {
        Ok(config) => Some(config),
        Err(error) => {
            error!(%error, "Cannot configure Fluent catalogs");
            None
        }
    }
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<terra_core::GameplayConfig>()
        .init_resource::<LocalizedUi>()
        .init_resource::<projection::Projection>()
        .init_resource::<preferences::ProfileLocale>()
        .add_systems(
            Update,
            (
                preferences::synchronize,
                projection::refresh,
                update_arguments,
            )
                .chain()
                .before(tilt_ui::TiltUiComponentRuntimeSet::Reactivity),
        );
}

fn update_arguments(
    view: Option<Res<crate::NpcWorkView>>,
    args: Option<ResMut<tilt_ui::UiFluentArgs>>,
) {
    if let (Some(view), Some(mut args)) = (view, args) {
        args.set("workers_count", "count", view.workers.len() as u32);
    }
}

pub(super) fn save_profile_language(world: &World) -> Result<(), String> {
    preferences::save(world)
}

fn translate(localization: &UiLocalization, key: &str, fallback: &str) -> String {
    localization
        .translate(key, None)
        .unwrap_or_else(|| fallback.to_owned())
}

/// Formats an engine-authored presentation string; never use this for player-authored text.
pub fn localized_text(world: &World, source: &str) -> String {
    match (
        world
            .get_resource::<projection::Projection>()
            .and_then(|projection| projection.catalog.as_ref()),
        world.get_resource::<UiLocalization>(),
    ) {
        (Some(catalog), Some(localization)) => catalog.text(localization, source),
        _ => source.to_owned(),
    }
}

/// Read-only counterpart of `localized_text` for ordinary UI search systems.
#[derive(SystemParam)]
pub(crate) struct SearchText<'w> {
    projection: Option<Res<'w, projection::Projection>>,
    localization: Option<Res<'w, UiLocalization>>,
}

impl SearchText<'_> {
    pub(crate) fn available(&self) -> bool {
        self.projection
            .as_ref()
            .is_some_and(|projection| projection.catalog.is_some())
            && self.localization.is_some()
    }

    pub(crate) fn changed(&self) -> bool {
        self.localization
            .as_ref()
            .is_some_and(|localization| localization.is_changed())
    }

    pub(crate) fn text(&self, source: &str) -> String {
        match (
            self.projection
                .as_ref()
                .and_then(|projection| projection.catalog.as_ref()),
            self.localization.as_deref(),
        ) {
            (Some(catalog), Some(localization)) => catalog.text(localization, source),
            _ => source.to_owned(),
        }
    }

    pub(crate) fn matches(&self, query: &str, source: &str) -> bool {
        source.to_lowercase().contains(query) || self.text(source).to_lowercase().contains(query)
    }
}

#[cfg(test)]
pub(crate) fn install_test_language(world: &mut World, locale: &str) {
    world.insert_resource(tests::localization(locale));
    world.init_resource::<LocalizedUi>();
    world.init_resource::<projection::Projection>();
    projection::refresh(world);
}
