pub(crate) mod biome;
pub(crate) mod environment;
pub(crate) mod material;
pub use environment::UnderwaterState;
mod contacts;
mod ink;
mod surf;
mod vents;
pub use ink::InkExposure;
pub(crate) mod mist;
mod raymarch;
#[cfg(test)]
pub(crate) mod spray;
mod storage;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod upload;

use bevy::prelude::*;
/// Status of the unified reconstructed liquid renderer.
#[derive(Resource, Default)]
pub struct WaterStatus {
    pub ready: bool,
    pub initial_ready: bool,
    pub visible_tiles: usize,
    pub triangles: usize,
    pub spray_particles: usize,
    pub mist_volumes: usize,
    #[cfg(test)]
    pub(super) scanned_sections: usize,
}

pub(super) fn install(app: &mut App) {
    storage::install(app);
    surf::install(app);
    contacts::install(app);
    app.add_systems(Update, vents::emit.before(contacts::UpdateContacts));
    raymarch::install(app);
    // The body-aware uploader owns standing water exclusively. Preserve its
    // depth, current, biome and reflection optics without reviving the previous
    // upload/spray systems or deforming a reservoir with camera-local particles.
    app.add_plugins(MaterialPlugin::<material::WaterMaterial>::default())
        .init_resource::<WaterStatus>()
        .init_resource::<UnderwaterState>()
        .init_resource::<InkExposure>()
        .add_systems(Startup, material::setup_water)
        .add_systems(
            Update,
            material::update_water_settings
                .after(crate::weather::WeatherSet::Local)
                .after(environment::update)
                .after(contacts::UpdateContacts),
        )
        .add_systems(
            Update,
            environment::update
                .after(crate::scene::update_view_distance)
                .after(terra_player::PlayerSet::Movement),
        )
        .add_systems(Update, ink::update.after(environment::update));
}
