//! The existing recorded chewing Foley follows the acknowledged local consumption.
use bevy::{
    audio::{AudioSink, AudioSinkPlayback, PlaybackSettings, Volume},
    prelude::*,
};
use terra_network::client::{ClientSession, SelectedInventorySlot};

#[derive(Component)]
pub(super) struct EatingSound {
    drinking: bool,
}

fn consumption_clip(drinking: bool) -> &'static str {
    if drinking {
        "audio/animals/drink.wav"
    } else {
        "audio/animals/eat.wav"
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    mut commands: Commands,
    assets: Res<AssetServer>,
    session: Option<Res<ClientSession>>,
    selected: Option<Res<SelectedInventorySlot>>,
    blocked: Option<Res<terra_player::PlayerInputBlock>>,
    keys: terra_core::GameplayInput,
    preferences: crate::audio_preferences::AudioPreferences,
    mut playing: Query<(Entity, &EatingSound, Option<&mut AudioSink>)>,
) {
    let drinking = session
        .as_ref()
        .and_then(|session| session.consumption)
        .is_some_and(|progress| terra_core::brewing::drink_seconds(progress.item).is_some());
    let active = session.as_ref().is_some_and(|session| {
        session.ready
            && !session.dead()
            && session.game_mode != terra_core::GameMode::Spectator
            && session.consumption.is_some_and(|consumption| {
                selected.as_ref().and_then(|slot| slot.0) == Some(consumption.slot)
                    && session
                        .inventory
                        .slot(usize::from(consumption.slot))
                        .is_some_and(|stack| stack.item == consumption.item)
            })
    }) && !blocked.as_ref().is_some_and(|blocked| blocked.0)
        && keys.consume_pressed();
    let gain = preferences.master_gain() * 0.12;
    if !active || gain <= 0.0 {
        for (entity, _, sink) in &mut playing {
            if let Some(sink) = sink {
                sink.stop();
            }
            commands.entity(entity).despawn();
        }
        return;
    }
    let mut matching = false;
    for (entity, sound, sink) in &mut playing {
        if sound.drinking != drinking || matching {
            if let Some(sink) = sink {
                sink.stop();
            }
            commands.entity(entity).despawn();
        } else {
            matching = true;
            if let Some(mut sink) = sink {
                sink.set_volume(Volume::Linear(gain));
            }
        }
    }
    if !matching {
        commands.spawn((
            EatingSound { drinking },
            AudioPlayer::new(assets.load(consumption_clip(drinking))),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(gain)),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drinking_uses_existing_water_foley_instead_of_chewing() {
        assert_eq!(consumption_clip(true), "audio/animals/drink.wav");
        assert_eq!(consumption_clip(false), "audio/animals/eat.wav");
        for clip in [consumption_clip(true), consumption_clip(false)] {
            assert!(terra_core::asset_directory().join(clip).is_file());
        }
    }
}
