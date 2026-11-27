use crate::water_audio::WaterAudioSource;
use bevy::{
    audio::{AudioSinkPlayback, Volume},
    prelude::*,
};
use terra_core::{AudioConfig, Config};
use terra_ui::menu::{AudioAudition, MenuDialog, MenuPage, MenuState, OptionsDraft, OptionsTab};

#[derive(Component)]
pub(super) struct Preview {
    channel: AudioAudition,
    until: f64,
}

const HOLD_SECONDS: f64 = 1.4;

pub(super) fn update(
    mut edits: MessageReader<AudioAudition>,
    time: Res<Time<Real>>,
    (menu, draft): (Res<MenuState>, Res<OptionsDraft>),
    mut previews: Query<(
        Entity,
        &mut Preview,
        &mut PlaybackSettings,
        Option<&mut AudioSink>,
    )>,
    assets: Res<AssetServer>,
    mut water: ResMut<Assets<WaterAudioSource>>,
    mut commands: Commands,
) {
    let mut requested = edits.read().last().copied();
    if requested.is_none() && previews.is_empty() {
        return;
    }
    let audible = menu.page == MenuPage::Options
        && menu.dialog == MenuDialog::None
        && draft.tab == OptionsTab::Audio
        && !draft.audio.muted
        && draft.audio.validate().is_ok();
    let now = time.elapsed_secs_f64();
    for (entity, mut preview, mut playback, sink) in &mut previews {
        let gain = channel_gain(preview.channel, &draft.audio);
        if !audible || gain == 0.0 || requested.is_some_and(|c| c != preview.channel) {
            commands.entity(entity).try_despawn();
            continue;
        }
        if requested == Some(preview.channel) {
            preview.until = now + HOLD_SECONDS;
            requested = None;
        }
        let remaining = preview.until - now;
        if remaining <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        let volume = Volume::Linear(gain * (remaining / 0.18).min(1.0) as f32);
        playback.volume = volume;
        if let Some(mut sink) = sink {
            sink.set_volume(volume);
        }
    }
    let Some(channel) = requested.filter(|c| audible && channel_gain(*c, &draft.audio) > 0.0)
    else {
        return;
    };
    let volume = Volume::Linear(channel_gain(channel, &draft.audio));
    let mut player = commands.spawn((
        Preview {
            channel,
            until: now + HOLD_SECONDS,
        },
        PlaybackSettings::DESPAWN.with_volume(volume),
    ));
    let clip = |path| AudioPlayer::new(assets.load(path));
    let mut water_voice = |gains| AudioPlayer(water.add(WaterAudioSource::isolated(gains)));
    match channel {
        AudioAudition::Master => player.insert(clip("audio/ui/mention.wav")),
        AudioAudition::Ambience => player.insert(clip("audio/environment/birds.wav")),
        AudioAudition::Animals => player.insert(clip("audio/animals/sheep-bleat.wav")),
        AudioAudition::Footsteps => player.insert(clip("audio/footsteps/grass.wav")),
        AudioAudition::Water => player.insert(water_voice([0.35, 0.0, 0.0])),
        AudioAudition::River => player.insert(water_voice([0.0, 0.35, 0.0])),
        AudioAudition::Waterfall => player.insert(water_voice([0.0, 0.0, 0.22])),
    };
}

fn channel_gain(channel: AudioAudition, audio: &AudioConfig) -> f32 {
    let category = match channel {
        AudioAudition::Master => 1.0,
        AudioAudition::Ambience => audio.ambience_volume,
        AudioAudition::Animals => audio.animals_volume,
        AudioAudition::Footsteps => audio.footsteps_volume,
        AudioAudition::Water => audio.ambience_volume * audio.water_volume,
        AudioAudition::River => audio.ambience_volume * audio.river_volume,
        AudioAudition::Waterfall => audio.ambience_volume * audio.waterfall_volume,
    };
    audio.master_volume * category
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, audio::Decodable};
    use std::time::Duration;

    fn voice(world: &mut World) -> Option<(Entity, f32)> {
        world
            .query_filtered::<(Entity, &PlaybackSettings), With<Preview>>()
            .iter(world)
            .map(|(entity, settings)| (entity, settings.volume.to_linear()))
            .next()
    }

    #[test]
    fn dragging_updates_the_same_audible_voice_until_mute_timeout_or_menu_exit() {
        let mut app = App::new();
        app.add_plugins((bevy::app::TaskPoolPlugin::default(), AssetPlugin::default()))
            .init_asset::<WaterAudioSource>()
            .init_resource::<Time<Real>>()
            .insert_resource(MenuState {
                page: MenuPage::Options,
                ..default()
            })
            .insert_resource(OptionsDraft {
                tab: OptionsTab::Audio,
                ..default()
            })
            .add_message::<AudioAudition>()
            .add_systems(PostUpdate, update);
        let world = app.world_mut();
        world.resource_mut::<OptionsDraft>().audio.ambience_volume = 0.5;
        world.write_message(AudioAudition::Water);
        world.write_message(AudioAudition::River);
        app.update();
        let world = app.world_mut();
        let (entity, gain) = voice(world).unwrap();
        assert_eq!(gain, 0.5);
        assert_eq!(
            world
                .query_filtered::<Entity, With<Preview>>()
                .iter(world)
                .count(),
            1
        );
        let handle = &world
            .get::<AudioPlayer<WaterAudioSource>>(entity)
            .unwrap()
            .0;
        let water = world.resource::<Assets<WaterAudioSource>>();
        assert!(
            water
                .get(handle)
                .unwrap()
                .decoder()
                .take(48_000)
                .any(|x| x.abs() > 0.005)
        );
        world.resource_mut::<OptionsDraft>().audio.river_volume = 0.25;
        world.write_message(AudioAudition::River);
        app.update();
        assert_eq!(voice(app.world_mut()), Some((entity, 0.125)));
        app.world_mut().resource_mut::<OptionsDraft>().audio.muted = true;
        app.update();
        assert!(voice(app.world_mut()).is_none());
        app.world_mut().resource_mut::<OptionsDraft>().audio.muted = false;
        app.world_mut().write_message(AudioAudition::Waterfall);
        app.update();
        assert!(voice(app.world_mut()).is_some());
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_secs(2));
        app.update();
        assert!(voice(app.world_mut()).is_none());
        app.world_mut().write_message(AudioAudition::Water);
        app.update();
        assert!(voice(app.world_mut()).is_some());
        app.world_mut().resource_mut::<MenuState>().page = MenuPage::Main;
        app.update();
        assert!(voice(app.world_mut()).is_none());
    }
}
