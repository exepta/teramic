//! Central physics defaults: meters, downward gravity, shared fixed tick.
use avian3d::prelude::*;
use bevy::prelude::*;
use terra_core::{GameConfig, VOXEL_SCALE_METERS};

pub struct TerraPhysicsPlugin;
impl Plugin for TerraPhysicsPlugin {
    fn build(&self, app: &mut App) {
        let tick_rate = app.world().resource::<GameConfig>().server.tick_rate;
        app.insert_resource(Time::<Fixed>::from_hz(f64::from(tick_rate)))
            .init_resource::<crate::CharacterProfile>()
            .insert_resource(Gravity(Vec3::NEG_Y * 9.81))
            .add_plugins(PhysicsPlugins::default().with_length_unit(VOXEL_SCALE_METERS));
        crate::terrain::install(app);
        crate::trains::install(app);
        crate::current::install(app);
    }
}
