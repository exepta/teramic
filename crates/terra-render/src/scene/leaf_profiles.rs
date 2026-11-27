use crate::meshing::shadows::{LeafProfileCaster, PROFILE_ERROR_METRES};
use bevy::{
    camera::visibility::{CascadesVisibleEntities, CubemapVisibleEntities, VisibleMeshEntities},
    light::Cascades,
    prelude::*,
};

pub(crate) fn install(app: &mut App) {
    app.add_systems(
        PostUpdate,
        select
            .after(bevy::light::check_dir_light_mesh_visibility)
            .after(bevy::light::check_point_light_mesh_visibility),
    );
}

fn select(
    time: Option<Res<Time>>,
    cameras: Query<&Camera, With<terra_player::TerraCamera>>,
    mut directional: Query<(
        &Cascades,
        &mut CascadesVisibleEntities,
        Has<super::daylight::Sun>,
    )>,
    mut points: Query<&mut CubemapVisibleEntities>,
    mut spots: Query<&mut VisibleMeshEntities, With<SpotLight>>,
    profiles: Query<(&LeafProfileCaster, &GlobalTransform)>,
) {
    if profiles.is_empty() {
        return;
    }
    let time = time.as_ref().map_or(0., |t| t.elapsed_secs_wrapped());
    for (cascades, mut visible, sun) in &mut directional {
        for (view, lists) in &mut visible.entities {
            let main = cameras
                .get(*view)
                .is_ok_and(|c| c.is_active && c.sub_camera_view.is_none());
            let parts = cascades.cascades.get(view);
            for (index, list) in lists.iter_mut().enumerate() {
                let texel = parts
                    .and_then(|p| p.get(index))
                    .map_or(0., |c| c.texel_size);
                list.entities.retain(|&e| {
                    let Ok((role, transform)) = profiles.get(e) else {
                        return true;
                    };
                    let roundoff = 16.
                        * f32::EPSILON
                        * (transform.translation().abs().max_element() + time * 2.3 + 32.);
                    let coarse = sun
                        && main
                        && index > 0
                        && texel.is_finite()
                        && transform.affine().matrix3 == bevy::math::Mat3A::IDENTITY
                        && PROFILE_ERROR_METRES + roundoff < texel * 0.25;
                    coarse == role.0
                });
            }
        }
    }
    // Local lights can intersect a leaf or magnify its thickness arbitrarily.
    let full = |e: &Entity| profiles.get(*e).map_or(true, |(role, _)| !role.0);
    for mut cubemap in &mut points {
        for side in cubemap.iter_mut() {
            side.entities.retain(full);
        }
    }
    for mut list in &mut spots {
        list.entities.retain(full);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::light::cascade::Cascade;

    #[test]
    fn profile_casters_require_a_coarse_sun_cascade_and_restore_exact_geometry_elsewhere() {
        let mut app = App::new();
        app.add_systems(Update, select);
        let main = app
            .world_mut()
            .spawn((Camera::default(), terra_player::TerraCamera))
            .id();
        let other = app.world_mut().spawn(Camera::default()).id();
        let full = app
            .world_mut()
            .spawn((GlobalTransform::IDENTITY, LeafProfileCaster(false)))
            .id();
        let profile = app
            .world_mut()
            .spawn((GlobalTransform::IDENTITY, LeafProfileCaster(true)))
            .id();
        let entries = || VisibleMeshEntities {
            entities: vec![full, profile],
        };
        let mut cascades = Cascades::default();
        let mut visible = CascadesVisibleEntities::default();
        for view in [main, other] {
            cascades.cascades.insert(
                view,
                [0.25, 0.08, 0.128]
                    .map(|texel_size| Cascade {
                        texel_size,
                        ..default()
                    })
                    .into(),
            );
            visible.entities.insert(view, vec![entries(); 3]);
        }
        let moon = app
            .world_mut()
            .spawn((cascades.clone(), visible.clone()))
            .id();
        let sun = app
            .world_mut()
            .spawn((super::super::daylight::Sun, cascades, visible))
            .id();
        let mut faces = CubemapVisibleEntities::default();
        for f in faces.iter_mut() {
            *f = entries();
        }
        let point = app.world_mut().spawn(faces).id();
        let spot = app
            .world_mut()
            .spawn((SpotLight::default(), entries()))
            .id();
        app.update();
        for (entity, view) in [(sun, main), (moon, main), (sun, other)] {
            let lists = &app
                .world()
                .get::<CascadesVisibleEntities>(entity)
                .unwrap()
                .entities[&view];
            assert_eq!(lists[0].entities, [full]);
            assert_eq!(lists[1].entities, [full]);
            assert_eq!(
                lists[2].entities,
                [if entity == sun && view == main {
                    profile
                } else {
                    full
                }]
            );
        }
        assert!(
            app.world()
                .get::<CubemapVisibleEntities>(point)
                .unwrap()
                .iter()
                .all(|face| face.entities == [full])
        );
        assert_eq!(
            app.world()
                .get::<VisibleMeshEntities>(spot)
                .unwrap()
                .entities,
            [full]
        );
    }
}
