//! The existing recorded chewing Foley follows the acknowledged local consumption.
use bevy::{
    audio::{AudioSink, AudioSinkPlayback, PlaybackSettings, Volume},
    prelude::*,
};
use terra_network::client::{ClientSession, SelectedInventorySlot};

#[derive(Component)]
pub(super) struct EatingSound;

#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    mut commands: Commands,
    assets: Res<AssetServer>,
    session: Option<Res<ClientSession>>,
    selected: Option<Res<SelectedInventorySlot>>,
    blocked: Option<Res<terra_player::PlayerInputBlock>>,
    keys: terra_core::GameplayInput,
    config: Option<Res<terra_core::AudioConfig>>,
    mut playing: Query<(Entity, Option<&mut AudioSink>), With<EatingSound>>,
) {
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
    let gain = config.as_ref().map_or(1.0, |config| {
        if config.muted {
            0.0
        } else {
            config.master_volume
        }
    }) * 0.12;
    if !active || gain <= 0.0 {
        for (entity, sink) in &mut playing {
            if let Some(sink) = sink {
                sink.stop();
            }
            commands.entity(entity).despawn();
        }
        return;
    }
    if playing.is_empty() {
        commands.spawn((
            EatingSound,
            AudioPlayer::new(assets.load("audio/animals/eat.wav")),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(gain)),
        ));
    } else {
        for (_, sink) in &mut playing {
            if let Some(mut sink) = sink {
                sink.set_volume(Volume::Linear(gain));
            }
        }
    }
}
