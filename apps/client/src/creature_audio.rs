//! Recorded animal cues and a bounded, body-listened environmental soundscape.
mod acoustics;
mod actions;
mod chests;
mod clips;
mod eating;
mod furniture;
mod scene;
mod smoke;
mod source;
mod spiders;
mod stalker;
mod thunder;

use bevy::{audio::AddAudioSource, prelude::*};
pub(crate) use chests::ChestAudioStatus;

pub(crate) fn install(app: &mut App) {
    crate::ui_audio::install(app);
    crate::footsteps::install(app);
    crate::water_contacts::install(app);
    app.add_audio_source::<source::RecordedSoundscape>()
        .init_resource::<scene::Soundscape>()
        .init_resource::<ChestAudioStatus>()
        .add_systems(Startup, scene::load)
        .add_systems(
            Update,
            (
                scene::update,
                scene::contacts,
                scene::water_contacts,
                actions::update,
            )
                .chain()
                .after(terra_core::footsteps::FootstepSet)
                .after(terra_core::footsteps::WaterMotionSet),
        );
    smoke::install(app);
    app.add_systems(Update, thunder::update.after(scene::update));
    app.add_systems(
        Update,
        eating::update.after(terra_player::PlayerSet::Interaction),
    );
    app.add_systems(Update, furniture::update.after(scene::update));
}
