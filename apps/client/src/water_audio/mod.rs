//! One spatially mixed water stream; no sound entities per voxel or camera listener.
pub(crate) mod scene;
mod smoke;
mod source;
pub(crate) use source::WaterAudioSource;

use bevy::{audio::AddAudioSource, prelude::*};

pub(crate) struct WaterAudioPlugin;

impl Plugin for WaterAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<source::WaterAudioSource>()
            .init_resource::<scene::WaterSoundscape>()
            .init_resource::<scene::WaterSoundStatus>()
            .add_systems(
                Update,
                scene::update.after(terra_player::PlayerSet::Movement),
            );
        smoke::install(app);
    }
}
