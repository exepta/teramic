//! Recorded animal cues and a bounded, body-listened environmental soundscape.
mod acoustics;
mod actions;
mod ambience;
mod beds;
mod chests;
mod clips;
mod eating;
mod furniture;
mod plumbing;
mod scene;
mod smoke;
mod source;
mod spiders;
mod stalker;
mod thunder;
mod worm;

use bevy::{audio::AddAudioSource, prelude::*};
pub(crate) use chests::ChestAudioStatus;
pub(crate) use plumbing::HydraulicAudioStatus;

/// Readiness of the existing shared asynchronous recording bank, not a second decode.
#[derive(Resource, Default)]
pub(crate) struct RecordedAudioStartup {
    pub ready: bool,
    pub error: Option<String>,
}

pub(crate) fn install(app: &mut App) {
    crate::ui_audio::install(app);
    crate::footsteps::install(app);
    crate::water_contacts::install(app);
    app.add_audio_source::<source::RecordedSoundscape>()
        .init_resource::<scene::Soundscape>()
        .init_resource::<ChestAudioStatus>()
        .init_resource::<plumbing::HydraulicAudioStatus>()
        .init_resource::<RecordedAudioStartup>()
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
    worm::install(app);
    app.add_systems(Update, thunder::update.after(scene::update));
    app.add_systems(
        Update,
        eating::update.after(terra_player::PlayerSet::Interaction),
    );
    app.add_systems(Update, furniture::update.after(scene::update));
    app.add_systems(Update, plumbing::update.after(scene::update));
}
