use super::*;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;
use terra_core::GameConfig;
use terra_core::TerraCorePlugin;

#[test]
fn fixed_tick_physics_moves_a_dynamic_body_downwards() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin))
        .add_plugins(TerraCorePlugin(GameConfig::default()))
        .add_plugins(TerraPhysicsPlugin)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 60.0,
        )));
    let body = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::sphere(0.5),
            Transform::from_xyz(0.0, 10.0, 0.0),
        ))
        .id();
    app.finish();
    app.cleanup();
    for _ in 0..60 {
        app.update();
    }
    let position = app.world().get::<Position>(body).unwrap().0;
    assert!(position.y < 8.0 && position.y > 3.0, "{position:?}");
    assert_eq!(
        app.world().resource::<Time<Fixed>>().timestep(),
        Duration::from_secs_f64(1.0 / 60.0)
    );
}

#[test]
fn accepting_a_server_correction_preserves_every_interpolated_visual_pose() {
    use crate::character::CharacterPresentation;
    for fraction in [0.0, 0.25, 0.5, 0.9, 1.0] {
        let current = Vec3::new(12.0, 4.0, -9.0);
        let corrected = current + Vec3::new(-0.09, 0.025, 0.035);
        let mut pose = CharacterPresentation {
            previous: current - Vec3::X * 0.09,
            correction: Vec3::new(0.03, -0.01, 0.0),
        };
        let before = pose.visual_position(current, fraction);
        pose.reconcile(current, corrected);
        assert!(pose.visual_position(corrected, fraction).distance(before) < 0.00001);
    }
}

#[test]
fn teleport_corrections_snap_instead_of_dragging_the_player_through_walls() {
    use crate::character::CharacterPresentation;
    let corrected = Vec3::new(-250.0, 82.0, 44.0);
    let mut pose = CharacterPresentation {
        previous: Vec3::ZERO,
        correction: Vec3::splat(0.1),
    };
    pose.reconcile(Vec3::ONE, corrected);
    assert_eq!(pose.visual_position(corrected, 0.37), corrected);
}
