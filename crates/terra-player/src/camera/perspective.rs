use super::{
    CameraInputState, CameraLook, CameraPerspective, PlayerBody, PlayerInputBlock, TerraCamera,
};
use avian3d::prelude::*;
use bevy::prelude::*;
use terra_core::{GameAction, GameplayInput};
type Bodies<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform), (With<PlayerBody>, Without<TerraCamera>)>;
type FollowCameras<'w, 's> = Query<
    'w,
    's,
    (&'static mut Transform, &'static mut CameraLook),
    (With<TerraCamera>, Without<PlayerBody>),
>;

pub(crate) fn toggle(
    input: GameplayInput,
    blocked: Res<PlayerInputBlock>,
    capture: Res<CameraInputState>,
    mut perspective: ResMut<CameraPerspective>,
) {
    if !blocked.0 && capture.captured && input.just_pressed(GameAction::Camera) {
        *perspective = perspective.next();
    }
}
pub(crate) fn boom(
    perspective: Res<CameraPerspective>,
    bodies: Bodies,
    mut cameras: FollowCameras,
    spatial: SpatialQuery,
    session: Option<Res<terra_network::client::ClientSession>>,
) {
    for (mut camera, mut look) in &mut cameras {
        if let Some(sleep) = session.as_ref().and_then(|session| session.sleeping()) {
            let rotation =
                Quat::from_rotation_y(f32::from(sleep.facing) * std::f32::consts::FRAC_PI_2);
            camera.translation = sleep.center() + rotation * Vec3::new(2.1, 1.8, -1.4);
            camera.rotation = Transform::from_translation(camera.translation)
                .looking_at(sleep.center(), Vec3::Y)
                .rotation;
            look.presented(camera.rotation);
            continue;
        }
        look.sync_external(camera.rotation);
        camera.rotation = look.rotation();
        if !perspective.is_third_person() {
            look.presented(camera.rotation);
            continue;
        }
        let Some((body, _)) = bodies.iter().next() else {
            look.presented(camera.rotation);
            continue;
        };
        let eye = camera.translation;
        let outward = if *perspective == CameraPerspective::ThirdPersonFront {
            camera.forward()
        } else {
            -camera.forward()
        };
        let filter = SpatialQueryFilter::from_excluded_entities([body]);
        let distance = spatial
            .cast_shape(
                &Collider::sphere(0.2),
                eye,
                Quat::IDENTITY,
                outward,
                &ShapeCastConfig::from_max_distance(3.5),
                &filter,
            )
            .map_or(3.5, |hit| (hit.distance - 0.08).max(0.0));
        camera.translation = eye + outward * distance;
        if *perspective == CameraPerspective::ThirdPersonFront {
            // Even a wall touching the eye must not make look_at use a zero direction.
            camera.rotation = Transform::default().looking_to(-*outward, Vec3::Y).rotation;
        }
        look.presented(camera.rotation);
    }
}
