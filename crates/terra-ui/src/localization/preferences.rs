use bevy::prelude::*;
use std::path::PathBuf;
use terra_core::{Config, GameLanguage, GameplayConfig, LanguagePreference, PlayerProfile};
use tilt_ui::{UiLang, UiLocalization};

#[derive(Resource, Default)]
pub(super) struct ProfileLocale {
    uuid: Option<terra_core::PlayerUuid>,
    initialized: bool,
}

fn directory(world: &World) -> PathBuf {
    world
        .get_resource::<crate::menu::MenuPaths>()
        .map_or_else(terra_core::menu_config_directory, |paths| {
            paths.configs.clone()
        })
}

fn path(world: &World) -> Option<PathBuf> {
    let profile = world.get_resource::<PlayerProfile>()?;
    Some(
        directory(world)
            .join("profile-preferences")
            .join(profile.uuid.simple().to_string())
            .join("language.toml"),
    )
}

pub(super) fn save(world: &World) -> Result<(), String> {
    let Some(path) = path(world) else {
        return Ok(());
    };
    let language = world
        .get_resource::<crate::menu::OptionsDraft>()
        .map_or_else(
            || world.resource::<GameplayConfig>().language,
            |draft| draft.gameplay.language,
        );
    LanguagePreference { language }
        .save(path)
        .map_err(|error| format!("Cannot save language preference: {error}"))
}

pub(super) fn synchronize(world: &mut World) {
    let uuid = world
        .get_resource::<PlayerProfile>()
        .map(|profile| profile.uuid);
    if world.resource::<ProfileLocale>().uuid != uuid {
        let fallback = if world.resource::<ProfileLocale>().initialized {
            GameLanguage::default()
        } else {
            world.resource::<GameplayConfig>().language
        };
        let language = path(world)
            .filter(|path| path.exists())
            .and_then(|path| match LanguagePreference::load(path) {
                Ok(preference) => Some(preference.language),
                Err(error) => {
                    warn!(%error, "Cannot load profile language preference");
                    None
                }
            })
            .unwrap_or(fallback);
        world.resource_mut::<GameplayConfig>().language = language;
        if let Some(mut draft) = world.get_resource_mut::<crate::menu::OptionsDraft>() {
            draft.gameplay.language = language;
        }
        let mut state = world.resource_mut::<ProfileLocale>();
        state.uuid = uuid;
        state.initialized = true;
    }
    let locale = world.resource::<GameplayConfig>().language.locale();
    if world
        .get_resource::<UiLang>()
        .is_none_or(|lang| lang.0 != locale)
    {
        world.insert_resource(UiLang::new(locale));
    }
    if world
        .get_resource::<UiLocalization>()
        .is_some_and(|localization| *localization.locale() != locale)
        && let Some(mut localization) = world.get_resource_mut::<UiLocalization>()
        && let Err(error) = localization.set_locale(locale)
    {
        error!(%error, "Cannot select UI language");
    }
}
