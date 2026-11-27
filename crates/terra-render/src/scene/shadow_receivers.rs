//! Refine the sun's square cascade footprints using the actual camera receivers.
use bevy::{
    camera::{
        CameraProjection,
        primitives::Aabb,
        visibility::{CascadesVisibleEntities, NoFrustumCulling, VisibleMeshEntities},
    },
    light::{CascadeShadowConfig, Cascades, ShadowFilteringMethod},
    math::Vec3A,
    prelude::*,
    tasks::{ComputeTaskPool, TaskPool},
};
use terra_core::miniature::MiniatureMesh;
use terra_player::TerraCamera;

pub(crate) fn install(app: &mut App) {
    // Explicit native comparison modes use the same graphics settings and camera.
    let mode = std::env::args()
        .any(|a| a.starts_with("--smoke-test"))
        .then(|| std::env::var("TERRA_SHADOW_RECEIVERS").ok())
        .flatten();
    if mode.as_deref() == Some("0") {
        return;
    }
    app.insert_resource(ReceiverCulling {
        directional: mode.as_deref() != Some("footprint"),
    });
    app.add_systems(
        PostUpdate,
        prune.after(bevy::light::check_dir_light_mesh_visibility),
    );
}

#[derive(Resource)]
struct ReceiverCulling {
    directional: bool,
}

struct Bounds {
    center: Vec3A,
    radius: Vec3A,
}

impl Bounds {
    fn relative(aabb: &Aabb, transform: &GlobalTransform, origin: Vec3A) -> Self {
        let model = transform.affine();
        let m = model.matrix3;
        Self {
            center: model.transform_point3a(aabb.center) - origin,
            radius: m.x_axis.abs() * aabb.half_extents.x
                + m.y_axis.abs() * aabb.half_extents.y
                + m.z_axis.abs() * aabb.half_extents.z,
        }
    }
}

/// Each SIMD lane tests one independent interval. Unused lanes accept all bounds.
#[derive(Clone, Copy)]
struct Intervals {
    axis: [Vec4; 3],
    absolute_axis: [Vec4; 3],
    min: Vec4,
    max: Vec4,
}

impl Intervals {
    const EMPTY: Self = Self {
        axis: [Vec4::ZERO; 3],
        absolute_axis: [Vec4::ZERO; 3],
        min: Vec4::NEG_INFINITY,
        max: Vec4::INFINITY,
    };

    fn intersects(&self, bounds: &Bounds) -> bool {
        let c = self.axis[0] * bounds.center.x
            + self.axis[1] * bounds.center.y
            + self.axis[2] * bounds.center.z;
        let r = self.absolute_axis[0] * bounds.radius.x
            + self.absolute_axis[1] * bounds.radius.y
            + self.absolute_axis[2] * bounds.radius.z;
        !((c + r).cmplt(self.min) | (c - r).cmpgt(self.max)).any()
    }
}

struct Footprint {
    // Six projected edge directions and five distinct frustum-face normals.
    intervals: [Intervals; 3],
    count: usize,
    origin: Vec3A,
}

impl Footprint {
    fn new(
        projection: &PerspectiveProjection,
        viewport: UVec2,
        camera: &GlobalTransform,
        near: f32,
        far: f32,
        light_rotation: Quat,
        padding: f32,
        directional: bool,
    ) -> Option<Self> {
        if viewport.min_element() == 0
            || !(near >= 0. && far > near && far.is_finite() && padding.is_finite())
            || !(0.0..std::f32::consts::PI).contains(&projection.fov)
            || !(projection.aspect_ratio > 0. && projection.aspect_ratio.is_finite())
            || projection.near_clip_plane.truncate() != Vec3::NEG_Z
            || !light_rotation.is_normalized()
        {
            return None;
        }
        let right = Vec3A::from(light_rotation * Vec3::X);
        let up = Vec3A::from(light_rotation * Vec3::Y);
        let origin = camera.translation_vec3a();
        // Include two screen pixels on each edge for raster coverage and TAA jitter.
        let overscan = Vec2::ONE + Vec2::splat(4.) / viewport.as_vec2();
        let mut corners = projection.get_frustum_corners(-near, -far);
        for corner in &mut corners {
            corner.x *= overscan.x;
            corner.y *= overscan.y;
            *corner = camera.affine().matrix3 * *corner;
            if !corner.is_finite() || !origin.is_finite() {
                return None;
            }
        }
        let mut region = Self {
            intervals: [Intervals::EMPTY; 3],
            count: 0,
            origin,
        };
        let sun = Vec3A::from(light_rotation * Vec3::Z);
        let mut add = |direction: Vec3A, one_sided: bool| {
            let Some(axis) = direction.try_normalize() else {
                return;
            };
            let delta = axis.dot(sun);
            if one_sided && delta.abs() < 0.0001 {
                return;
            }
            let dots = corners.map(|corner| corner.dot(axis));
            // Every possible blocker is a receiver displaced towards the sun.
            // Its far end along a non-perpendicular axis is therefore unbounded.
            let min = if one_sided && delta < 0. {
                f32::NEG_INFINITY
            } else {
                dots.into_iter().fold(f32::INFINITY, f32::min) - padding
            };
            let max = if one_sided && delta > 0. {
                f32::INFINITY
            } else {
                dots.into_iter().fold(f32::NEG_INFINITY, f32::max) + padding
            };
            let pack = &mut region.intervals[region.count / 4];
            let lane = region.count % 4;
            for i in 0..3 {
                pack.axis[i][lane] = axis[i];
                pack.absolute_axis[i][lane] = axis[i].abs();
            }
            pack.min[lane] = min;
            pack.max[lane] = max;
            region.count += 1;
        };
        if directional {
            // The silhouette of a projected frustum has edges parallel to its
            // two rim directions or its four perspective rays. Extruding their
            // normals gives the exact light-space XY footprint at any sun angle.
            let vertical = corners[5] - corners[4];
            let horizontal = corners[6] - corners[5];
            add(sun.cross(vertical), false);
            add(sun.cross(horizontal), false);
            add(horizontal.cross(vertical), true);
            for i in 0..4 {
                let ray = corners[i + 4] - corners[i];
                let rim = corners[4 + (i + 1) % 4] - corners[4 + i];
                add(sun.cross(ray), false);
                add(ray.cross(rim), true);
            }
        } else {
            for axis in [right, up, right + up, right - up] {
                add(axis, false);
            }
        }
        Some(region)
    }

    fn intersects(&self, bounds: &Bounds) -> bool {
        self.intervals[..self.count.div_ceil(4)]
            .iter()
            .all(|pack| pack.intersects(bounds))
    }
}

fn near_bound(config: &CascadeShadowConfig, index: usize) -> f32 {
    // The first cascade can also be sampled by fragments closer than the
    // configured shadow minimum. Each following cascade starts in the blend.
    index.checked_sub(1).map_or(0., |previous| {
        config.bounds[previous] * (1. - config.overlap_proportion)
    })
}

#[derive(Default)]
struct ReceiverWork {
    footprints: Vec<Option<Footprint>>,
    keep: Vec<Vec<u8>>,
}

type Casters<'w, 's> = Query<
    'w,
    's,
    (
        &'static Aabb,
        &'static GlobalTransform,
        Option<&'static MiniatureMesh>,
        Has<crate::meshing::shadows::ShadowProxy>,
    ),
    (With<Mesh3d>, Without<NoFrustumCulling>),
>;

fn intersects_caster(hull: &Footprint, entity: Entity, casters: &Casters) -> bool {
    let Ok((bounds, model, kind, shadow)) = casters.get(entity) else {
        return true;
    };
    if !(shadow || matches!(kind, Some(MiniatureMesh::Terrain))) {
        return true;
    }
    hull.intersects(&Bounds::relative(bounds, model, hull.origin))
}

impl ReceiverWork {
    fn retain(
        &mut self,
        lists: &mut [VisibleMeshEntities],
        pool: Option<&TaskPool>,
        intersects: impl Fn(&Footprint, Entity) -> bool + Sync,
    ) {
        const CHUNK: usize = 1024;
        let candidates: usize = lists
            .iter()
            .zip(&self.footprints)
            .filter(|(_, hull)| hull.is_some())
            .map(|(list, _)| list.entities.len())
            .sum();
        let Some(pool) = pool.filter(|p| p.thread_num() > 1 && candidates >= 2 * CHUNK) else {
            for (list, region) in lists.iter_mut().zip(&self.footprints) {
                if let Some(hull) = region {
                    list.entities.retain(|&e| intersects(hull, e));
                }
            }
            return;
        };

        // Work directly on the candidate lists Bevy just populated. No world-wide
        // change scan or stale bounds; the shared ECS references remain read-only.
        self.keep.resize_with(lists.len(), Vec::new);
        pool.scope(|scope| {
            let intersects = &intersects;
            for ((list, region), keep) in lists.iter().zip(&self.footprints).zip(&mut self.keep) {
                let Some(hull) = region else { continue };
                keep.resize(list.entities.len(), 0);
                for (entities, out) in list.entities.chunks(CHUNK).zip(keep.chunks_mut(CHUNK)) {
                    scope.spawn(async move {
                        for (&e, result) in entities.iter().zip(out) {
                            *result = u8::from(intersects(hull, e));
                        }
                    });
                }
            }
        });
        for ((list, region), keep) in lists.iter_mut().zip(&self.footprints).zip(&self.keep) {
            if region.is_some() {
                let mut flags = keep.iter();
                list.entities.retain(|_| flags.next() == Some(&1));
            }
        }
    }
}

fn prune(
    culling: Res<ReceiverCulling>,
    cameras: Query<
        (
            Entity,
            &Camera,
            &Projection,
            &GlobalTransform,
            &ShadowFilteringMethod,
        ),
        (With<TerraCamera>, Without<bevy::light::VolumetricFog>),
    >,
    mut lights: Query<
        (
            &DirectionalLight,
            &GlobalTransform,
            &CascadeShadowConfig,
            &Cascades,
            &mut CascadesVisibleEntities,
        ),
        With<super::daylight::Sun>,
    >,
    casters: Casters,
    mut work: Local<ReceiverWork>,
) {
    let Ok((
        view,
        camera,
        Projection::Perspective(projection),
        eye,
        ShadowFilteringMethod::Gaussian,
    )) = cameras.single()
    else {
        return;
    };
    let Some(viewport) = camera.physical_viewport_size() else {
        return;
    };
    if !camera.is_active || camera.sub_camera_view.is_some() {
        return;
    }
    for (light, transform, config, cascades, mut visible) in &mut lights {
        if !light.shadow_maps_enabled || !(0.0..=1.0).contains(&config.overlap_proportion) {
            continue;
        }
        let Some(parts) = cascades.cascades.get(&view) else {
            continue;
        };
        let Some(lists) = visible.entities.get_mut(&view) else {
            continue;
        };
        if parts.len() != lists.len() || parts.len() != config.bounds.len() {
            continue;
        }
        let rotation = transform.rotation();
        work.footprints.clear();
        for (index, cascade) in parts.iter().enumerate() {
            // Gaussian's hardware bilinear taps, receiver normal bias (scaled
            // by sqrt(2) in Bevy), and floating point error at world coordinates.
            // Depth bias is world-space and changes the one-sided receiver planes.
            let padding = cascade.texel_size
                * (8. + light.shadow_normal_bias.abs() * std::f32::consts::SQRT_2)
                + 16.
                    * f32::EPSILON
                    * (eye.translation().abs().max_element() + config.bounds[index] + 1.);
            work.footprints.push(Footprint::new(
                projection,
                viewport,
                eye,
                near_bound(config, index),
                config.bounds[index],
                rotation,
                if culling.directional {
                    padding + light.shadow_depth_bias.abs()
                } else {
                    padding
                },
                culling.directional,
            ));
        }
        work.retain(
            lists,
            ComputeTaskPool::try_get().map(|pool| &**pool),
            |hull, entity| intersects_caster(hull, entity, &casters),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projection() -> PerspectiveProjection {
        PerspectiveProjection {
            fov: std::f32::consts::FRAC_PI_2,
            aspect_ratio: 16. / 9.,
            ..default()
        }
    }

    fn frame(near: f32, far: f32, padding: f32) -> Footprint {
        // Sun directly above the world, its shadow area spans world X and Z.
        Footprint::new(
            &projection(),
            UVec2::new(2560, 1440),
            &GlobalTransform::IDENTITY,
            near,
            far,
            Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
            padding,
            true,
        )
        .unwrap()
    }

    fn caster(hull: &Footprint, center: Vec3, radius: f32) -> bool {
        hull.intersects(&Bounds::relative(
            &Aabb {
                center: Vec3A::ZERO,
                half_extents: Vec3A::splat(radius),
            },
            &GlobalTransform::from_translation(center),
            hull.origin,
        ))
    }

    #[test]
    fn parallel_lists_match_ordered_live_bounds_for_overlapping_cascades_and_streamed_entities() {
        use bevy::ecs::system::SystemState;
        let mut world = World::new();
        let terrain = world
            .spawn((
                Mesh3d::default(),
                Aabb::default(),
                MiniatureMesh::Terrain,
                GlobalTransform::from_translation(Vec3::new(0., 6., -12.)),
            ))
            .id();
        let rock = world
            .spawn((
                Mesh3d::default(),
                Aabb::default(),
                crate::meshing::shadows::ShadowProxy,
                GlobalTransform::from_translation(Vec3::new(80.0, 0., -12.)),
            ))
            .id();
        let foreign = world
            .spawn((
                Mesh3d::default(),
                Aabb::default(),
                GlobalTransform::from_translation(Vec3::splat(1e4)),
            ))
            .id();
        let skipped = world
            .spawn((
                Mesh3d::default(),
                Aabb::default(),
                MiniatureMesh::Terrain,
                GlobalTransform::from_translation(Vec3::splat(1e4)),
                NoFrustumCulling,
            ))
            .id();
        let mut queries = SystemState::<Casters>::new(&mut world);
        let pool = bevy::tasks::TaskPoolBuilder::new().num_threads(2).build();
        let mut work = ReceiverWork {
            footprints: vec![
                Some(frame(0., 16., 0.04)),
                None,
                Some(frame(12., 50., 0.04)),
            ],
            ..default()
        };
        for moved in [false, true] {
            if moved {
                *world.get_mut::<GlobalTransform>(terrain).unwrap() =
                    GlobalTransform::from_translation(Vec3::splat(100.));
                world.get_mut::<Aabb>(rock).unwrap().half_extents = Vec3A::splat(100.);
            }
            let casters = queries.get(&world).unwrap();
            let hits = |hull: &Footprint, e| intersects_caster(hull, e, &casters);
            let mut lists: Vec<_> = [1801, 9, 2093]
                .into_iter()
                .enumerate()
                .map(|(k, count)| VisibleMeshEntities {
                    entities: [terrain, foreign, skipped, rock, Entity::PLACEHOLDER]
                        .into_iter()
                        .cycle()
                        .skip(k)
                        .take(count)
                        .collect(),
                })
                .collect();
            let mut reference = lists.clone();
            work.retain(&mut reference, None, hits);
            work.retain(&mut lists, Some(&pool), hits);
            for (list, expected) in lists.iter().zip(&reference) {
                assert_eq!(list.entities, expected.entities);
            }
            assert_eq!(lists[0].entities.contains(&terrain), !moved);
            assert_eq!(lists[0].entities.contains(&rock), moved);
            for e in [foreign, skipped, Entity::PLACEHOLDER] {
                assert!(lists[0].entities.contains(&e));
            }
            assert_eq!(lists[1].entities.len(), 9);
        }
    }

    #[test]
    fn offscreen_tree_casting_onto_a_visible_receiver_is_kept() {
        let area = frame(0., 16., 0.04);
        assert!(!caster(&area, Vec3::new(8., -100., -12.), 0.5));
        for height in [0., 300.] {
            assert!(caster(&area, Vec3::new(8., height, -12.), 0.5));
        }
        assert!(!caster(&area, Vec3::new(40., 300., -12.), 0.5));
        assert!(!caster(&area, Vec3::new(24., 300., -1.), 0.5));
    }

    #[test]
    fn parallel_plane_intervals_match_independent_scalar_plane_tests() {
        let area = frame(12., 50., 0.04);
        for x in (-80..80).step_by(5) {
            for y in (-60..60).step_by(7) {
                for z in (-64..32).step_by(5) {
                    let bounds = Bounds {
                        center: Vec3A::new(x as f32, y as f32, z as f32),
                        radius: Vec3A::new(0.6, 3.5, 1.1),
                    };
                    let scalar = (0..area.count).all(|i| {
                        let p = &area.intervals[i / 4];
                        let j = i % 4;
                        let axis = Vec3A::new(p.axis[0][j], p.axis[1][j], p.axis[2][j]);
                        let c = axis.dot(bounds.center);
                        let r = axis.abs().dot(bounds.radius);
                        c + r >= p.min[j] && c - r <= p.max[j]
                    });
                    assert_eq!(area.intersects(&bounds), scalar);
                }
            }
        }
    }

    #[test]
    fn entering_a_new_cascade_recomputes_which_offscreen_casters_can_reach_the_camera() {
        let sun = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        let receiver = Vec3::new(0., 0., -12.);
        let volume = |eye: &GlobalTransform, directional| {
            Footprint::new(
                &projection(),
                UVec2::splat(1024),
                eye,
                0.,
                16.,
                sun,
                0.1,
                directional,
            )
            .unwrap()
        };
        let buried = receiver - Vec3::Y * 40.;
        let first = GlobalTransform::IDENTITY;
        assert!(caster(&volume(&first, false), buried, 0.5));
        assert!(!caster(&volume(&first, true), buried, 0.5));
        assert!(caster(&volume(&first, true), receiver + Vec3::Y * 80., 0.5));
        // A teleport to a cave puts the former downstream caster above the new receiver.
        let cave = GlobalTransform::from_translation(-Vec3::Y * 48.);
        assert!(caster(&volume(&cave, true), buried, 0.5));
        // Wind-expanded bounds touching the last possible receiver still cast.
        assert!(caster(&volume(&first, true), receiver - Vec3::Y * 13., 1.));
    }

    #[test]
    fn both_cascades_keep_the_blend_and_the_filter_margin() {
        let config = CascadeShadowConfig {
            bounds: vec![16., 40.],
            overlap_proportion: 0.25,
            minimum_distance: 1.,
        };
        let first = frame(near_bound(&config, 0), 16., 0.25);
        let second = frame(near_bound(&config, 1), 40., 0.25);
        for z in [-12., -14., -16.] {
            assert!(caster(&first, Vec3::new(0., 20., z), 0.));
            assert!(caster(&second, Vec3::new(0., 20., z), 0.));
        }
        assert!(caster(&first, Vec3::new(0., 20., 0.24), 0.));
        assert!(caster(&second, Vec3::new(0., 20., -11.8), 0.));
        assert!(!caster(&second, Vec3::new(0., 20., -10.), 0.));
        // Even when its center is outside the last cascade, a wind-expanded
        // AABB that reaches the receiver's PCF footprint must survive.
        assert!(caster(&second, Vec3::new(0., 20., -40.8), 0.6));
    }

    #[test]
    fn uncertain_projection_and_empty_viewports_fall_back_to_the_engine_list() {
        let create = |projection: PerspectiveProjection, viewport| {
            Footprint::new(
                &projection,
                viewport,
                &GlobalTransform::IDENTITY,
                0.,
                16.,
                Quat::IDENTITY,
                0.,
                true,
            )
        };
        assert!(create(projection(), UVec2::ZERO).is_none());
        let oblique = PerspectiveProjection {
            near_clip_plane: Vec4::new(0.1, 0., -0.99, 0.1),
            ..projection()
        };
        assert!(create(oblique, UVec2::splat(1024)).is_none());
        assert!(
            create(
                PerspectiveProjection {
                    aspect_ratio: f32::NAN,
                    ..projection()
                },
                UVec2::splat(1024)
            )
            .is_none()
        );
    }

    #[test]
    fn every_shadow_ray_through_a_screen_sample_stays_inside_the_real_bevy_cascade_footprint() {
        use bevy::light::{DirectionalLightShadowMap, cascade::build_directional_light_cascades};
        let mut app = App::new();
        app.init_resource::<DirectionalLightShadowMap>()
            .add_systems(Update, build_directional_light_cascades);
        let config = CascadeShadowConfig {
            bounds: vec![16., 40., 102., 256.],
            overlap_proportion: 0.25,
            minimum_distance: 0.1,
        };
        let view = app
            .world_mut()
            .spawn((Camera::default(), Projection::Perspective(projection())))
            .id();
        let sun = app
            .world_mut()
            .spawn((
                DirectionalLight {
                    shadow_maps_enabled: true,
                    ..default()
                },
                config.clone(),
            ))
            .id();
        for (yaw, pitch, roll, azimuth, altitude) in [
            (0.2, -0.1, 0.0, 1.1, 0.2),
            (-1.6, -0.65, 0.2, -2.0, 0.75),
            (2.4, 0.8, -0.4, 1.4, 1.33),
            (0.7, -1.1, 0.3, -0.3, 0.04),
        ] {
            let eye = GlobalTransform::from(
                Transform::from_xyz(137.5, 68.3, -280.6).with_rotation(Quat::from_euler(
                    EulerRot::YXZ,
                    yaw,
                    pitch,
                    roll,
                )),
            );
            let light = Quat::from_euler(EulerRot::YXZ, azimuth, altitude, 0.);
            app.world_mut().entity_mut(view).insert(eye);
            app.world_mut()
                .entity_mut(sun)
                .insert(GlobalTransform::from_rotation(light));
            app.update();
            let actual = &app.world().get::<Cascades>(sun).unwrap().cascades[&view];
            for (i, cascade) in actual.iter().enumerate() {
                let start = near_bound(&config, i);
                let end = config.bounds[i];
                let region = Footprint::new(
                    &projection(),
                    UVec2::new(2560, 1440),
                    &eye,
                    start,
                    end,
                    light,
                    8. * cascade.texel_size,
                    true,
                )
                .unwrap();
                // Derive the ray and filter axes independently from the actual
                // light matrices that Bevy sends to its world-to-shadow shader.
                let ray = cascade.world_from_cascade.z_axis.truncate();
                let filter_shift = (cascade.world_from_cascade.x_axis
                    + cascade.world_from_cascade.y_axis)
                    .truncate()
                    * (2. * cascade.texel_size);
                for depth in [start, start * 0.9 + end * 0.1, 0.5 * (start + end), end] {
                    for x in [-1., -0.3, 0.25, 1.] {
                        for y in [-1., -0.1, 1.] {
                            let sample = eye.transform_point(Vec3::new(
                                projection().aspect_ratio * depth * x,
                                depth * y,
                                -depth,
                            ));
                            let caster_center = sample + ray * 240.;
                            let xy = |p: Vec3| {
                                (cascade.clip_from_world * p.extend(1.))
                                    .truncate()
                                    .truncate()
                            };
                            assert!(xy(sample).abs_diff_eq(xy(caster_center), 0.0001));
                            assert!(caster(&region, caster_center + filter_shift, 0.01));
                        }
                    }
                }
            }
        }
    }
}
