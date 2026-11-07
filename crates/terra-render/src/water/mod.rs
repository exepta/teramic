pub(crate) mod biome;
pub(crate) mod environment;
pub(crate) mod material;
pub use environment::UnderwaterState;
mod contacts;
mod ink;
pub use ink::InkExposure;
pub(crate) mod mist;
mod raymarch;
pub(crate) mod spray;
mod storage;
#[cfg(test)]
mod tests;
mod upload;

use bevy::prelude::*;
use terra_world::streaming::StreamingSet;
pub use upload::WaterStatus;

pub(super) fn install(app: &mut App) {
    storage::install(app);
    contacts::install(app);
    raymarch::install(app);
    app.add_plugins(MaterialPlugin::<material::WaterMaterial>::default())
        .init_resource::<upload::WaterTiles>()
        .init_resource::<WaterStatus>()
        .init_resource::<UnderwaterState>()
        .init_resource::<InkExposure>()
        .init_resource::<mist::MistAssets>()
        .add_systems(Startup, material::setup_water)
        .add_systems(Startup, spray::setup)
        .add_systems(Update, spray::update.after(upload::upload_water))
        .add_systems(
            Update,
            mist::update
                .after(upload::upload_water)
                .after(crate::scene::caves::quality::ApplyCaveQuality),
        )
        .add_systems(
            Update,
            material::update_water_settings
                .after(crate::weather::WeatherSet::Local)
                .after(environment::update)
                .after(contacts::UpdateContacts),
        )
        .add_systems(Update, upload::upload_water.after(StreamingSet::Receive))
        .add_systems(
            Update,
            environment::update
                .after(crate::scene::update_view_distance)
                .after(terra_player::PlayerSet::Movement),
        )
        .add_systems(Update, ink::update.after(environment::update));
}
