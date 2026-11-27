use bevy::{ecs::system::SystemParam, prelude::*};
use std::sync::LazyLock;
use terra_core::{AudioConfig, Config};
use terra_ui::menu::{MenuPage, MenuState, OptionsDraft};

/// The options editor owns its draft until Apply, including visits to the HUD layout editor.
#[derive(SystemParam)]
pub(crate) struct AudioPreferences<'w> {
    menu: Option<Res<'w, MenuState>>,
    draft: Option<Res<'w, OptionsDraft>>,
    saved: Option<Res<'w, AudioConfig>>,
}

impl AudioPreferences<'_> {
    pub(crate) fn current(&self) -> &AudioConfig {
        if self
            .menu
            .as_ref()
            .is_some_and(|menu| matches!(menu.page, MenuPage::Options | MenuPage::HudLayout))
            && let Some(draft) = &self.draft
            && draft.audio.validate().is_ok()
        {
            return &draft.audio;
        }
        static DEFAULT: LazyLock<AudioConfig> = LazyLock::new(AudioConfig::default);
        self.saved.as_deref().unwrap_or(&DEFAULT)
    }

    pub(crate) fn master_gain(&self) -> f32 {
        let config = self.current();
        if config.muted {
            0.0
        } else {
            config.master_volume
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::SystemState;

    #[test]
    fn active_mix_follows_valid_edits_and_restores_saved_levels_when_leaving_options() {
        let mut world = World::new();
        let mut preferences = SystemState::<AudioPreferences>::new(&mut world);
        assert_eq!(preferences.get(&world).unwrap().master_gain(), 1.0);
        let saved = AudioConfig {
            master_volume: 0.8,
            ..default()
        };
        let edited = AudioConfig {
            master_volume: 0.4,
            ambience_volume: 0.0,
            animals_volume: 0.0,
            footsteps_volume: 0.0,
            water_volume: 0.0,
            river_volume: 0.0,
            waterfall_volume: 0.0,
            ..default()
        };
        world.insert_resource(saved.clone());
        world.insert_resource(OptionsDraft {
            audio: edited.clone(),
            ..default()
        });
        world.init_resource::<MenuState>();
        assert_eq!(preferences.get(&world).unwrap().current(), &saved);
        for page in [MenuPage::Options, MenuPage::HudLayout] {
            world.resource_mut::<MenuState>().page = page;
            assert_eq!(preferences.get(&world).unwrap().current(), &edited);
            assert_eq!(preferences.get(&world).unwrap().master_gain(), 0.4);
        }
        world.resource_mut::<OptionsDraft>().audio.muted = true;
        assert_eq!(preferences.get(&world).unwrap().master_gain(), 0.0);
        world.resource_mut::<OptionsDraft>().audio.water_volume = f32::NAN;
        assert_eq!(preferences.get(&world).unwrap().current(), &saved);
        world.resource_mut::<OptionsDraft>().audio = edited.clone();
        world.resource_mut::<MenuState>().page = MenuPage::Pause;
        assert_eq!(preferences.get(&world).unwrap().current(), &saved);
        world.insert_resource(edited.clone());
        assert_eq!(preferences.get(&world).unwrap().current(), &edited);
    }
}
