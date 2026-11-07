//! Nonspatial, rate-limited UI feedback. Gameplay audio continues to use body listeners.
use bevy::prelude::*;
use terra_core::footsteps::UiSound;

pub(crate) fn install(app: &mut App) {
    app.add_systems(Update, play);
}
fn play(
    mut messages: MessageReader<UiSound>,
    mut clicks: MessageReader<bevy::picking::events::PointerClick>,
    ui: Query<
        (),
        (
            With<tilt_ui::TiltButton>,
            Without<bevy::ui::InteractionDisabled>,
        ),
    >,
    parents: Query<&ChildOf>,
    (time, config, assets): (
        Res<Time>,
        Option<Res<terra_core::AudioConfig>>,
        Res<AssetServer>,
    ),
    mut commands: Commands,
    mut last: Local<[f64; 2]>,
) {
    let mut cues: Vec<_> = messages.read().copied().collect();
    for click in clicks.read() {
        let mut entity = click.entity;
        for _ in 0..10 {
            if ui.contains(entity) {
                cues.push(UiSound::Click);
                break;
            }
            let Ok(parent) = parents.get(entity) else {
                break;
            };
            entity = parent.parent();
        }
    }
    let gain = ui_gain(config.as_deref());
    if gain == 0.0 {
        return;
    }
    for cue in cues {
        let (index, name, volume, delay) = match cue {
            UiSound::Click => (0, "click", 0.13, 0.07),
            UiSound::Mention => (1, "mention", 0.35, 0.8),
        };
        let now = time.elapsed_secs_f64();
        if now - last[index] < delay {
            continue;
        }
        last[index] = now;
        commands.spawn((
            AudioPlayer::new(assets.load(format!("audio/ui/{name}.wav"))),
            bevy::audio::PlaybackSettings::DESPAWN
                .with_volume(bevy::audio::Volume::Linear(gain * volume)),
        ));
    }
}

fn ui_gain(config: Option<&terra_core::AudioConfig>) -> f32 {
    config.map_or(1.0, |c| if c.muted { 0.0 } else { c.master_volume })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::audio::{AudioSource, Decodable};
    #[test]
    fn category_sliders_never_change_ui_feedback_but_master_and_mute_do() {
        let mut config = terra_core::AudioConfig {
            master_volume: 0.4,
            ambience_volume: 0.0,
            animals_volume: 0.0,
            footsteps_volume: 0.0,
            water_volume: 0.0,
            river_volume: 0.0,
            waterfall_volume: 0.0,
            ..default()
        };
        assert_eq!(ui_gain(None), 1.0);
        assert_eq!(ui_gain(Some(&config)), 0.4);
        config.muted = true;
        assert_eq!(ui_gain(Some(&config)), 0.0);
    }
    #[test]
    fn short_ui_cues_decode_with_quiet_faded_boundaries() {
        for name in ["click", "mention"] {
            let bytes =
                std::fs::read(terra_core::asset_directory().join(format!("audio/ui/{name}.wav")))
                    .unwrap();
            let source = AudioSource {
                bytes: bytes.into(),
            };
            let decoder =
                <AudioSource as Decodable>::Decoder::try_from(std::io::Cursor::new(source))
                    .unwrap();
            let samples: Vec<f32> = decoder.collect();
            assert!(samples.len() > 1000 && samples.len() < 24000);
            assert!(samples.iter().all(|s| s.is_finite() && s.abs() <= 0.5));
            assert!(samples.first().unwrap().abs() < 0.001 && samples.last().unwrap().abs() < 0.01);
        }
    }
}
