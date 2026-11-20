//! Streaming distances describe ground around the actor, not the detached camera.
use terra_core::ClientConfig;
use terra_player::ConstructionCamera;

#[derive(Clone, Copy, Debug)]
pub(crate) struct CameraRange {
    pub offset: f32,
    pub far: f32,
    pub fog: (f32, f32),
}

impl CameraRange {
    pub fn limited(
        config: &ClientConfig,
        construction: Option<&ConstructionCamera>,
        limit: Option<u8>,
    ) -> Self {
        let offset = construction
            .filter(|camera| camera.active)
            .map_or(0.0, |camera| {
                camera.pose().translation.distance(camera.center)
            });
        let range = f32::from(
            limit
                .map_or(config.view_distance, |limit| {
                    config.view_distance.min(limit)
                })
                .max(1),
        ) * 16.0;
        let fog = super::surface_fog_range(range, config.fog_distance);
        Self {
            offset,
            far: range + 16.0 + offset,
            fog: (fog.0 + offset, fog.1 + offset),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;
    use terra_core::GameConfig;
    use terra_player::TerraCamera;
    use terra_world::{
        edits::EditJournal,
        streaming::{ChunkStore, WorldTerrain},
    };

    #[test]
    fn startup_range_tracks_actual_loaded_radius_without_losing_far_view_setting() {
        let config = ClientConfig {
            view_distance: 32,
            ..Default::default()
        };
        let initial = CameraRange::limited(&config, None, Some(15));
        let next = CameraRange::limited(&config, None, Some(16));
        let full = CameraRange::limited(&config, None, None);
        assert_eq!(initial.far, 256.0);
        assert_eq!(next.far, 272.0);
        assert_eq!(full.far, 528.0);
        assert!(initial.fog.1 < next.fog.1 && next.fog.1 < full.fog.1);
        assert_eq!(config.view_distance, 32);
    }

    fn assert_ranges(app: &App, camera: Entity, far: f32, fog: (f32, f32)) {
        let Projection::Perspective(projection) = app.world().get::<Projection>(camera).unwrap()
        else {
            panic!("Perspective construction camera")
        };
        assert!((projection.far - far).abs() < 0.0001);
        let FogFalloff::Linear { start, end } =
            &app.world().get::<DistanceFog>(camera).unwrap().falloff
        else {
            panic!("Linear surface fog")
        };
        assert!((start - fog.0).abs() < 0.0001, "{start} != {}", fog.0);
        assert!((end - fog.1).abs() < 0.0001, "{end} != {}", fog.1);
    }

    #[test]
    fn construction_camera_keeps_view_two_ground_visible_at_every_zoom_and_restores_fog() {
        let mut config = GameConfig::default();
        config.client.view_distance = 2;
        config.client.fog_distance = 1.0;
        config.world.water = false;
        let mut app = App::new();
        app.insert_resource(config)
            .init_resource::<Time>()
            .init_resource::<super::super::daylight::Daylight>()
            .init_resource::<super::super::caves::CaveState>()
            .init_resource::<crate::water::UnderwaterState>()
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .insert_resource(WorldTerrain(
                terra_world::TerraGenerator::new(Default::default()).unwrap(),
            ))
            .add_systems(
                Update,
                (
                    super::super::update_view_distance,
                    crate::water::environment::update,
                )
                    .chain(),
            );
        let camera = app
            .world_mut()
            .spawn((
                TerraCamera,
                Transform::default(),
                Projection::default(),
                DistanceFog::default(),
                super::super::surface_grading(),
                AmbientLight::default(),
                super::super::caves::shader::CaveSettings::default(),
            ))
            .id();
        app.update();
        assert_ranges(&app, camera, 48.0, (20.8, 29.76));
        app.insert_resource(ConstructionCamera {
            active: true,
            center: Vec3::new(101.0, 12.0, -43.0),
            ..default()
        });
        for height in 18..=72 {
            let pose = {
                let mut construction = app.world_mut().resource_mut::<ConstructionCamera>();
                construction.height = height as f32;
                construction.pose()
            };
            *app.world_mut().get_mut::<Transform>(camera).unwrap() = pose;
            let offset = height as f32 * (1.0_f32 + 0.55 * 0.55).sqrt();
            for _ in 0..2 {
                // The second frame takes the stable-environment path without any
                // changed config, daylight or construction resource.
                app.update();
                assert_ranges(&app, camera, 48.0 + offset, (20.8 + offset, 29.76 + offset));
            }
        }
        {
            let mut config = app.world_mut().resource_mut::<GameConfig>();
            config.client.view_distance = 4;
            config.client.fog_distance = 0.5;
        }
        let offset = 72.0 * (1.0_f32 + 0.55 * 0.55).sqrt();
        app.update();
        assert_ranges(&app, camera, 80.0 + offset, (20.8 + offset, 29.76 + offset));
        app.world_mut().resource_mut::<ConstructionCamera>().active = false;
        app.update();
        assert_ranges(&app, camera, 80.0, (20.8, 29.76));
        app.world_mut().resource_mut::<ConstructionCamera>().active = true;
        app.update();
        assert_ranges(&app, camera, 80.0 + offset, (20.8 + offset, 29.76 + offset));
        app.world_mut().remove_resource::<ConstructionCamera>();
        app.update();
        app.update();
        assert_ranges(&app, camera, 80.0, (20.8, 29.76));
    }
}
