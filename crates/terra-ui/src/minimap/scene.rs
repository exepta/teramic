//! A second view of the live world, sharing gameplay meshes and animation.
use super::{Atlas, fullmap::FullMapState};
use bevy::{
    camera::{
        CameraProjection, Exposure, Hdr, RenderTarget, ScalingMode,
        visibility::{RenderLayers, VisibilityRange},
    },
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::view::ColorGrading,
};
use terra_core::miniature::{CREATURE_LAYER, MiniatureMesh, MiniatureView, WORLD_LAYER};

// No geometry uses this layer. The hidden gameplay view still needs the sun
// and moon to keep its shared atmosphere/environment cubemap illuminated.
const ENVIRONMENT_ONLY_LAYER: usize = 26;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct SceneSet;

#[derive(Resource, Default, Clone, Debug)]
pub struct Map3dRenderStatus {
    pub active: bool,
    pub camera: Option<Entity>,
    pub terrain_meshes: usize,
    pub prop_meshes: usize,
    pub water_meshes: usize,
    pub creature_meshes: usize,
    pub player_meshes: usize,
    pub shadow_cascades: usize,
    pub shadow_depth_band: Option<[f32; 2]>,
    pub environment_map_shared: bool,
}

#[derive(Resource, Default)]
struct MapScene {
    camera: Option<Entity>,
    was_active: bool,
}

#[derive(Component)]
struct MiniatureCamera;

#[derive(Component)]
struct SavedRange(VisibilityRange);

#[derive(Component)]
struct SavedGameplayLayers(Option<RenderLayers>);

#[derive(Component)]
struct SavedMiniatureCascades {
    original: bevy::light::CascadeShadowConfig,
    applied: bevy::light::CascadeShadowConfig,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<MapScene>()
        .init_resource::<MiniatureView>()
        .init_resource::<Map3dRenderStatus>()
        .add_systems(Update, publish.after(super::update))
        .add_systems(
            PostUpdate,
            (world_lights, gameplay_view, synchronize.in_set(SceneSet))
                .before(bevy::transform::TransformSystems::Propagate)
                .before(bevy::camera::CameraUpdateSystems),
        )
        .add_systems(
            PostUpdate,
            miniature_environment
                .after(synchronize)
                .before(bevy::camera::CameraUpdateSystems),
        )
        .add_systems(
            PostUpdate,
            miniature_shadows
                .after(bevy::transform::TransformSystems::Propagate)
                .after(bevy::camera::CameraUpdateSystems)
                .after(bevy::camera::visibility::VisibilitySystems::CalculateBounds)
                .before(bevy::light::SimulationLightSystems::UpdateDirectionalLightCascades),
        );
}

fn same_environment(a: &EnvironmentMapLight, b: &EnvironmentMapLight) -> bool {
    a.diffuse_map == b.diffuse_map
        && a.specular_map == b.specular_map
        && a.intensity == b.intensity
        && a.rotation == b.rotation
        && a.affects_lightmapped_mesh_diffuse == b.affects_lightmapped_mesh_diffuse
}

fn miniature_environment(
    mut commands: Commands,
    map: Res<FullMapState>,
    mut status: ResMut<Map3dRenderStatus>,
    source: Query<Option<&EnvironmentMapLight>, With<terra_player::TerraCamera>>,
    cameras: Query<(Entity, Option<&EnvironmentMapLight>), With<MiniatureCamera>>,
) {
    let environment = if map.three_dimensional_active() {
        source.iter().flatten().next()
    } else {
        None
    };
    status.environment_map_shared = false;
    for (entity, current) in &cameras {
        if let Some(environment) = environment {
            if current.is_none_or(|current| !same_environment(current, environment)) {
                // Share the already-filtered gameplay cubemaps. Adding either
                // generator component here would allocate/filter another probe.
                commands.entity(entity).insert(environment.clone());
            }
            status.environment_map_shared = true;
        } else if current.is_some() {
            commands.entity(entity).remove::<EnvironmentMapLight>();
        }
    }
}

fn same_cascades(
    a: &bevy::light::CascadeShadowConfig,
    b: &bevy::light::CascadeShadowConfig,
) -> bool {
    a.bounds == b.bounds
        && a.overlap_proportion == b.overlap_proportion
        && a.minimum_distance == b.minimum_distance
}

fn miniature_cascades(
    original: &bevy::light::CascadeShadowConfig,
    depth: [f32; 2],
) -> bevy::light::CascadeShadowConfig {
    let count = original.bounds.len().clamp(1, 3);
    // Quantized padding keeps wind and tiny camera moves from resizing cascades.
    let near = (((depth[0] - 8.0) / 16.0).floor() * 16.0).max(0.1);
    let far = (((depth[1] + 8.0) / 16.0).ceil() * 16.0).max(near + 16.0);
    let width = far - near;
    bevy::light::CascadeShadowConfig {
        minimum_distance: near,
        bounds: (1..=count)
            .map(|i| near + width * i as f32 / count as f32)
            .collect(),
        // Bevy overlaps absolute eye-distance splits. Scale that overlap to the
        // occupied band rather than spending it on the empty kilometre above it.
        overlap_proportion: (original.overlap_proportion * width / far).clamp(0.0, 0.2),
    }
}

fn occupied_depth(
    reference: &super::raster::Projection,
    level: i64,
    geometry: &Query<(
        &MiniatureMesh,
        &bevy::camera::primitives::Aabb,
        &GlobalTransform,
    )>,
) -> Option<[f32; 2]> {
    use bevy::math::Vec3A;
    let camera = camera_transform(reference, level);
    let eye = Vec3A::from(camera.translation);
    let forward = Vec3A::from(*camera.forward());
    let right = Vec3A::from(*camera.right());
    let up = Vec3A::from(*camera.up());
    let half_width = reference.span() * 0.5;
    let half_height = half_width * reference.aspect();
    let mut depth = [f32::INFINITY, f32::NEG_INFINITY];
    for (kind, bounds, transform) in geometry {
        if !matches!(
            kind,
            MiniatureMesh::Terrain | MiniatureMesh::Water | MiniatureMesh::Prop
        ) {
            continue;
        }
        let affine = transform.affine();
        let relative = affine.transform_point3a(bounds.center) - eye;
        if relative.dot(right).abs() > half_width + bounds.relative_radius(&right, &affine.matrix3)
            || relative.dot(up).abs() > half_height + bounds.relative_radius(&up, &affine.matrix3)
        {
            continue;
        }
        let center = relative.dot(forward);
        let radius = bounds.relative_radius(&forward, &affine.matrix3);
        if center + radius > 0.1 {
            depth[0] = depth[0].min(center - radius);
            depth[1] = depth[1].max(center + radius);
        }
    }
    (depth[0].is_finite() && depth[1].is_finite()).then_some(depth)
}

#[allow(clippy::type_complexity)]
fn miniature_shadows(
    mut commands: Commands,
    map: Res<FullMapState>,
    atlas: Res<Atlas>,
    mut status: ResMut<Map3dRenderStatus>,
    geometry: Query<(
        &MiniatureMesh,
        &bevy::camera::primitives::Aabb,
        &GlobalTransform,
    )>,
    mut lights: Query<(
        Entity,
        &DirectionalLight,
        &mut bevy::light::CascadeShadowConfig,
        Option<&SavedMiniatureCascades>,
        Option<&RenderLayers>,
    )>,
) {
    let depth = if map.three_dimensional_active() {
        atlas
            .full
            .reference
            .as_ref()
            .filter(|p| p.is_three_dimensional())
            .and_then(|reference| occupied_depth(reference, atlas.full.reference_level, &geometry))
    } else {
        None
    };
    status.shadow_depth_band = depth;
    status.shadow_cascades = 0;
    for (entity, light, mut config, saved, layers) in &mut lights {
        let world_light = layers.is_none_or(|layers| layers.intersects(&RenderLayers::default()));
        if let Some(depth) =
            depth.filter(|_| world_light && light.shadow_maps_enabled && !config.bounds.is_empty())
        {
            // Preserve external quality changes even if configuration is reloaded
            // while the map is open; only our last applied value is temporary.
            let original = saved
                .filter(|saved| same_cascades(&config, &saved.applied))
                .map_or_else(|| config.clone(), |saved| saved.original.clone());
            let adapted = miniature_cascades(&original, depth);
            status.shadow_cascades += adapted.bounds.len();
            if !same_cascades(&config, &adapted) || saved.is_none() {
                *config = adapted.clone();
                commands.entity(entity).insert(SavedMiniatureCascades {
                    original,
                    applied: adapted,
                });
            }
        } else if let Some(saved) = saved {
            if same_cascades(&config, &saved.applied) {
                *config = saved.original.clone();
            }
            commands.entity(entity).remove::<SavedMiniatureCascades>();
        }
    }
}

fn publish(map: Res<FullMapState>, mut view: ResMut<MiniatureView>) {
    let active = map.three_dimensional_active();
    if view.active != active {
        view.active = active;
    }
}

#[allow(clippy::type_complexity)]
fn world_lights(
    mut commands: Commands,
    lights: Query<
        (Entity, Option<&RenderLayers>, Has<DirectionalLight>),
        Or<(Added<DirectionalLight>, Added<PointLight>)>,
    >,
) {
    for (entity, layers, directional) in &lights {
        let layers = layers.cloned().unwrap_or_default();
        if layers.intersects(&RenderLayers::default()) {
            let layers = layers.with(WORLD_LAYER).with(CREATURE_LAYER);
            let layers = if directional {
                layers.with(ENVIRONMENT_ONLY_LAYER)
            } else {
                layers
            };
            commands.entity(entity).insert(layers);
        }
    }
}

#[allow(clippy::type_complexity)]
fn gameplay_view(
    mut commands: Commands,
    map: Res<FullMapState>,
    cameras: Query<
        (Entity, Option<&RenderLayers>, Option<&SavedGameplayLayers>),
        With<terra_player::TerraCamera>,
    >,
) {
    let active = map.three_dimensional_active();
    for (entity, layers, saved) in &cameras {
        if active && saved.is_none() {
            // Keep UI and atmosphere lighting alive without drawing world meshes
            // underneath the opaque FullMap. NONE would blacken the shared IBL.
            commands.entity(entity).insert((
                SavedGameplayLayers(layers.cloned()),
                RenderLayers::layer(ENVIRONMENT_ONLY_LAYER),
            ));
        } else if !active && let Some(saved) = saved {
            let mut camera = commands.entity(entity);
            if let Some(layers) = &saved.0 {
                camera.insert(layers.clone());
            } else {
                camera.remove::<RenderLayers>();
            }
            camera.remove::<SavedGameplayLayers>();
        }
    }
}

fn camera_depth(reference: &super::raster::Projection) -> f32 {
    let half_z = reference.span() * reference.aspect() / 0.60 * 0.5 + 96.0;
    0.8 * half_z + 0.6 * 484.0 + 512.0
}

fn camera_transform(reference: &super::raster::Projection, level: i64) -> Transform {
    let center = reference.world_at([reference.size() as f32 * 0.5; 2]);
    let target = Vec3::new(center[0] as f32, level as f32, center[1] as f32);
    Transform::from_translation(target + Vec3::new(0.0, 0.6, 0.8) * camera_depth(reference))
        .looking_at(target, Vec3::Y)
}

fn projection(reference: &super::raster::Projection) -> Projection {
    let mut projection = OrthographicProjection {
        scaling_mode: ScalingMode::Fixed {
            width: reference.span(),
            height: reference.span() * reference.aspect(),
        },
        near: 0.1,
        far: camera_depth(reference) * 2.0,
        ..OrthographicProjection::default_3d()
    };
    // Fixed orthographic extents are known immediately. Keep the frustum valid
    // even on the spawn frame, before Bevy resolves the image render target.
    projection.update(reference.span(), reference.span() * reference.aspect());
    Projection::Orthographic(projection)
}

type GameplayView = (
    &'static Exposure,
    &'static AmbientLight,
    &'static Tonemapping,
    &'static ColorGrading,
);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn synchronize(
    mut commands: Commands,
    atlas: Res<Atlas>,
    map: Res<FullMapState>,
    mut scene: ResMut<MapScene>,
    mut status: ResMut<Map3dRenderStatus>,
    game: Query<GameplayView, (With<terra_player::TerraCamera>, Without<MiniatureCamera>)>,
    mut cameras: Query<
        (
            &mut Camera,
            &mut RenderTarget,
            &mut Projection,
            &mut Transform,
            &mut RenderLayers,
            &mut Exposure,
            &mut AmbientLight,
            &mut Tonemapping,
            &mut ColorGrading,
        ),
        With<MiniatureCamera>,
    >,
    meshes: Query<(
        Entity,
        &MiniatureMesh,
        Option<&VisibilityRange>,
        Option<&SavedRange>,
    )>,
) {
    let active = map.three_dimensional_active();
    *status = Map3dRenderStatus {
        active,
        camera: scene.camera,
        ..default()
    };
    if !active && !scene.was_active {
        return;
    }
    scene.was_active = active;
    for (entity, kind, range, saved) in &meshes {
        // Distance LOD normally uses the gameplay camera. A bird's-eye camera is
        // much higher, so retain already-loaded prop batches while this view is
        // open. Neither terrain generation nor streaming expands.
        if active
            && let Some(range) = range
            && saved.is_none()
        {
            commands
                .entity(entity)
                .insert(SavedRange(range.clone()))
                .remove::<VisibilityRange>();
        } else if !active && let Some(saved) = saved {
            commands
                .entity(entity)
                .insert(saved.0.clone())
                .remove::<SavedRange>();
        }
        match kind {
            MiniatureMesh::Terrain => status.terrain_meshes += 1,
            MiniatureMesh::Water => status.water_meshes += 1,
            MiniatureMesh::Prop => status.prop_meshes += 1,
            MiniatureMesh::Creature => status.creature_meshes += 1,
            MiniatureMesh::Player => status.player_meshes += 1,
            MiniatureMesh::Drop => (),
        }
    }
    if let Some((mut camera, ..)) = scene.camera.and_then(|entity| cameras.get_mut(entity).ok()) {
        camera.is_active = active;
    }
    let Some(reference) = atlas
        .full
        .reference
        .as_ref()
        .filter(|p| p.is_three_dimensional())
    else {
        return;
    };
    if !active {
        return;
    }
    let layers = if map.mobs {
        RenderLayers::layer(WORLD_LAYER).with(CREATURE_LAYER)
    } else {
        RenderLayers::layer(WORLD_LAYER)
    };
    let (exposure, ambient, tonemapping, grading) = game.iter().next().map_or_else(
        || {
            (
                Exposure { ev100: 13.7 },
                AmbientLight::default(),
                Tonemapping::AcesFitted,
                ColorGrading::default(),
            )
        },
        |(e, a, t, g)| (*e, a.clone(), *t, g.clone()),
    );
    let transform = camera_transform(reference, atlas.full.reference_level);
    if let Some((
        mut camera,
        mut target,
        mut p,
        mut current,
        mut mask,
        mut e,
        mut a,
        mut t,
        mut g,
    )) = scene.camera.and_then(|entity| cameras.get_mut(entity).ok())
    {
        camera.is_active = true;
        *target = RenderTarget::Image(atlas.full.terrain.clone().into());
        *p = projection(reference);
        *current = transform;
        *mask = layers;
        *e = exposure;
        *a = ambient;
        *t = tonemapping;
        *g = grading;
    } else {
        scene.camera = Some(
            commands
                .spawn((
                    Name::new("FullMap live-world miniature camera"),
                    MiniatureCamera,
                    Camera3d::default(),
                    Camera {
                        is_active: true,
                        order: -9,
                        clear_color: ClearColorConfig::Custom(Color::srgb_u8(20, 26, 25)),
                        ..default()
                    },
                    RenderTarget::Image(atlas.full.terrain.clone().into()),
                    projection(reference),
                    transform,
                    layers,
                    (Hdr, exposure, ambient, tonemapping, grading),
                    Msaa::Sample4,
                    bevy::core_pipeline::prepass::DepthPrepass,
                    bevy::pbr::ScreenSpaceTransmission {
                        steps: 1,
                        ..default()
                    },
                    bevy::light::ShadowFilteringMethod::Gaussian,
                ))
                .id(),
        );
        status.camera = scene.camera;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_camera_matches_marker_projection_at_real_world_coordinates() {
        for center in [[1120.5, -902.25], [-16384.0, -315.0]] {
            for aspect in [0.5, 1.0, 1.5, 3.0] {
                let p =
                    super::super::raster::Projection::at(center, 384, 0.0, false, true, 640, false)
                        .with_aspect(aspect)
                        .three_dimensional(true);
                let transform = camera_transform(&p, 72);
                let Projection::Orthographic(projection) = projection(&p) else {
                    unreachable!()
                };
                let view_from_world = transform.to_matrix().inverse();
                for offset in [
                    Vec3::ZERO,
                    Vec3::new(-21.0, 48.0, 35.0),
                    Vec3::new(50.0, -96.0, -63.0),
                ] {
                    let world = Vec3::new(center[0] as f32, 72.0, center[1] as f32) + offset;
                    let view = view_from_world.transform_point3(world);
                    let clip = projection.get_clip_from_view() * view.extend(1.0);
                    let actual = Vec2::new(
                        (clip.x / clip.w + 1.0) * 320.0,
                        (1.0 - clip.y / clip.w) * 320.0,
                    );
                    let expected = Vec2::from_array(p.project(world.to_array(), 72));
                    assert!(
                        actual.distance(expected) < 0.02,
                        "{actual:?} != {expected:?}"
                    );
                    assert!(-view.z > projection.near && -view.z < projection.far);
                }
            }
        }
    }

    #[test]
    fn miniature_uses_world_layers_not_preview_or_first_person_layers() {
        let view = RenderLayers::layer(WORLD_LAYER).with(CREATURE_LAYER);
        assert!(view.intersects(&RenderLayers::default().with(WORLD_LAYER)));
        assert!(view.intersects(&RenderLayers::default().with(CREATURE_LAYER)));
        for layer in [0, ENVIRONMENT_ONLY_LAYER, 29, 30, 31, 61, 62] {
            assert!(!view.intersects(&RenderLayers::layer(layer)));
        }
    }

    #[test]
    fn live_view_shares_existing_mesh_and_restores_gameplay_distance_culling() {
        let mut app = App::new();
        let mut atlas = Atlas::default();
        atlas.full.reference = Some(
            super::super::raster::Projection::at(
                [-1120.0, -700.0],
                384,
                0.0,
                false,
                true,
                640,
                false,
            )
            .three_dimensional(true),
        );
        atlas.full.reference_level = 64;
        app.insert_resource(atlas)
            .init_resource::<FullMapState>()
            .init_resource::<MapScene>()
            .init_resource::<Map3dRenderStatus>()
            .add_systems(Update, synchronize);
        let mut map = app.world_mut().resource_mut::<FullMapState>();
        map.open = true;
        map.three_dimensional = true;
        let handle = Handle::<Mesh>::default();
        let prop = app
            .world_mut()
            .spawn((
                MiniatureMesh::Prop,
                Mesh3d(handle.clone()),
                RenderLayers::default().with(WORLD_LAYER),
                VisibilityRange::abrupt(0.0, 96.0),
            ))
            .id();
        app.update();
        assert_eq!(app.world().get::<Mesh3d>(prop).unwrap().0, handle);
        assert!(app.world().get::<VisibilityRange>(prop).is_none());
        assert!(app.world().get::<SavedRange>(prop).is_some());
        let status = app.world().resource::<Map3dRenderStatus>();
        assert_eq!(status.prop_meshes, 1);
        let camera = status.camera.unwrap();
        assert!(app.world().get::<Camera>(camera).unwrap().is_active);
        let world = app.world_mut();
        assert_eq!(world.query::<&Mesh3d>().iter(world).count(), 1);
        app.world_mut().resource_mut::<FullMapState>().layer =
            Some(terra_world::cartography::MapLayer::Cave { level: 48 });
        app.update();
        assert!(!app.world().get::<Camera>(camera).unwrap().is_active);
        assert!(
            app.world().get::<VisibilityRange>(prop).unwrap()
                == &VisibilityRange::abrupt(0.0, 96.0)
        );
        assert!(app.world().get::<SavedRange>(prop).is_none());
    }

    #[test]
    fn real_world_lights_include_miniature_without_leaking_preview_lights() {
        let mut app = App::new();
        app.add_systems(Update, world_lights);
        let sun = app
            .world_mut()
            .spawn((
                DirectionalLight::default(),
                RenderLayers::default().with(61),
            ))
            .id();
        let preview = app
            .world_mut()
            .spawn((PointLight::default(), RenderLayers::layer(31)))
            .id();
        let world_point = app.world_mut().spawn(PointLight::default()).id();
        app.update();
        let layers = app.world().get::<RenderLayers>(sun).unwrap();
        assert!(layers.intersects(&RenderLayers::layer(WORLD_LAYER)));
        assert!(layers.intersects(&RenderLayers::layer(61)));
        assert!(layers.intersects(&RenderLayers::layer(ENVIRONMENT_ONLY_LAYER)));
        let point_layers = app.world().get::<RenderLayers>(world_point).unwrap();
        assert!(point_layers.intersects(&RenderLayers::layer(WORLD_LAYER)));
        assert!(!point_layers.intersects(&RenderLayers::layer(ENVIRONMENT_ONLY_LAYER)));
        assert_eq!(
            app.world().get::<RenderLayers>(preview).unwrap(),
            &RenderLayers::layer(31)
        );
        let world = app.world_mut();
        assert_eq!(world.query::<&DirectionalLight>().iter(world).count(), 1);
        assert_eq!(world.query::<&PointLight>().iter(world).count(), 2);
    }

    #[test]
    fn miniature_suppresses_only_gameplay_world_and_restores_exact_layers() {
        let mut app = App::new();
        app.init_resource::<FullMapState>()
            .add_systems(Update, (world_lights, gameplay_view));
        let mut map = app.world_mut().resource_mut::<FullMapState>();
        map.open = true;
        map.three_dimensional = true;
        let original = RenderLayers::default().with(9);
        let camera = app
            .world_mut()
            .spawn((
                terra_player::TerraCamera,
                original.clone(),
                Camera::default(),
            ))
            .id();
        let implicit = app
            .world_mut()
            .spawn((terra_player::TerraCamera, Camera::default()))
            .id();
        let sun = app.world_mut().spawn(DirectionalLight::default()).id();
        app.update();
        assert!(app.world().get::<Camera>(camera).unwrap().is_active);
        assert_eq!(
            app.world().get::<RenderLayers>(camera).unwrap(),
            &RenderLayers::layer(ENVIRONMENT_ONLY_LAYER)
        );
        let hidden_layers = app.world().get::<RenderLayers>(camera).unwrap();
        assert!(hidden_layers.intersects(app.world().get::<RenderLayers>(sun).unwrap()));
        assert!(!hidden_layers.intersects(&RenderLayers::default().with(WORLD_LAYER)));
        assert!(!hidden_layers.intersects(&RenderLayers::default().with(CREATURE_LAYER)));
        app.world_mut().resource_mut::<FullMapState>().open = false;
        app.update();
        assert_eq!(app.world().get::<RenderLayers>(camera).unwrap(), &original);
        assert!(app.world().get::<RenderLayers>(implicit).is_none());
    }

    #[test]
    fn miniature_cascades_cover_the_occupied_band_without_increasing_quality_budget() {
        for count in 1..=4 {
            let original = bevy::light::CascadeShadowConfig {
                bounds: (1..=count).map(|i| i as f32 * 48.0).collect(),
                minimum_distance: 0.5,
                overlap_proportion: 0.2,
            };
            for depth in [[912.5, 1174.0], [763.0, 764.0], [0.25, 41.0]] {
                let actual = miniature_cascades(&original, depth);
                assert!(actual.minimum_distance <= depth[0]);
                assert!(actual.bounds.last().is_some_and(|end| *end >= depth[1]));
                assert!(actual.bounds.len() <= original.bounds.len());
                assert!(actual.bounds.len() <= 3);
                assert!(actual.bounds.windows(2).all(|w| w[0] < w[1]));
                assert!(actual.overlap_proportion <= original.overlap_proportion);
            }
        }
    }

    #[test]
    fn miniature_shadows_fit_real_geometry_and_restore_exact_original_configuration() {
        use bevy::{camera::primitives::Aabb, math::Vec3A};
        let mut app = App::new();
        let mut atlas = Atlas::default();
        let reference = super::super::raster::Projection::at(
            [-1120.0, -700.0],
            384,
            0.0,
            false,
            true,
            640,
            false,
        )
        .three_dimensional(true);
        let depth = camera_depth(&reference);
        atlas.full.reference = Some(reference);
        atlas.full.reference_level = 64;
        app.insert_resource(atlas)
            .init_resource::<FullMapState>()
            .init_resource::<Map3dRenderStatus>()
            .add_systems(Update, miniature_shadows);
        {
            let mut map = app.world_mut().resource_mut::<FullMapState>();
            map.open = true;
            map.three_dimensional = true;
        }
        app.world_mut().spawn((
            MiniatureMesh::Terrain,
            Aabb {
                center: Vec3A::ZERO,
                half_extents: Vec3A::splat(20.0),
            },
            GlobalTransform::from_translation(Vec3::new(-1120.0, 64.0, -700.0)),
        ));
        app.world_mut().spawn((
            MiniatureMesh::Terrain,
            Aabb {
                center: Vec3A::ZERO,
                half_extents: Vec3A::splat(20.0),
            },
            GlobalTransform::from_translation(Vec3::new(10_000.0, 10_000.0, 10_000.0)),
        ));
        let original = bevy::light::CascadeShadowConfig {
            bounds: vec![16.0, 48.0, 128.0],
            minimum_distance: 0.75,
            overlap_proportion: 0.23,
        };
        let sun = app
            .world_mut()
            .spawn((
                DirectionalLight {
                    shadow_maps_enabled: true,
                    ..default()
                },
                original.clone(),
                RenderLayers::default().with(WORLD_LAYER),
            ))
            .id();
        let disabled = app
            .world_mut()
            .spawn((
                DirectionalLight {
                    shadow_maps_enabled: false,
                    ..default()
                },
                original.clone(),
            ))
            .id();
        app.update();
        let actual = app
            .world()
            .get::<bevy::light::CascadeShadowConfig>(sun)
            .unwrap();
        assert_eq!(actual.bounds.len(), original.bounds.len());
        assert!(actual.minimum_distance < depth - 28.0);
        assert!(actual.minimum_distance > depth - 60.0);
        assert!(*actual.bounds.last().unwrap() > depth + 28.0);
        let status = app.world().resource::<Map3dRenderStatus>();
        let band = status.shadow_depth_band.unwrap();
        assert!((band[0] - (depth - 28.0)).abs() < 0.01);
        assert!((band[1] - (depth + 28.0)).abs() < 0.01);
        assert_eq!(status.shadow_cascades, original.bounds.len());
        assert!(same_cascades(
            app.world()
                .get::<bevy::light::CascadeShadowConfig>(disabled)
                .unwrap(),
            &original
        ));
        assert!(
            app.world()
                .get::<SavedMiniatureCascades>(disabled)
                .is_none()
        );
        app.world_mut().resource_mut::<FullMapState>().open = false;
        app.update();
        assert!(same_cascades(
            app.world()
                .get::<bevy::light::CascadeShadowConfig>(sun)
                .unwrap(),
            &original
        ));
        assert!(app.world().get::<SavedMiniatureCascades>(sun).is_none());
        assert_eq!(
            app.world().resource::<Map3dRenderStatus>().shadow_cascades,
            0
        );
    }

    #[test]
    fn miniature_keeps_external_shadow_quality_changes_when_closed() {
        use bevy::{camera::primitives::Aabb, math::Vec3A};
        let mut app = App::new();
        let mut atlas = Atlas::default();
        atlas.full.reference = Some(
            super::super::raster::Projection::at([0.0, 0.0], 384, 0.0, false, true, 640, false)
                .three_dimensional(true),
        );
        app.insert_resource(atlas)
            .init_resource::<FullMapState>()
            .init_resource::<Map3dRenderStatus>()
            .add_systems(Update, miniature_shadows);
        {
            let mut map = app.world_mut().resource_mut::<FullMapState>();
            map.open = true;
            map.three_dimensional = true;
        }
        app.world_mut().spawn((
            MiniatureMesh::Terrain,
            Aabb {
                center: Vec3A::ZERO,
                half_extents: Vec3A::splat(20.0),
            },
            GlobalTransform::IDENTITY,
        ));
        let sun = app
            .world_mut()
            .spawn((
                DirectionalLight {
                    shadow_maps_enabled: true,
                    ..default()
                },
                bevy::light::CascadeShadowConfig::default(),
            ))
            .id();
        app.update();
        let updated = bevy::light::CascadeShadowConfig {
            bounds: vec![24.0, 96.0],
            minimum_distance: 2.0,
            overlap_proportion: 0.15,
        };
        app.world_mut().entity_mut(sun).insert(updated.clone());
        app.update();
        assert_eq!(
            app.world()
                .get::<bevy::light::CascadeShadowConfig>(sun)
                .unwrap()
                .bounds
                .len(),
            2
        );
        app.world_mut().resource_mut::<FullMapState>().open = false;
        app.update();
        assert!(same_cascades(
            app.world()
                .get::<bevy::light::CascadeShadowConfig>(sun)
                .unwrap(),
            &updated
        ));
    }

    #[test]
    fn miniature_shares_filtered_environment_handles_and_tracks_changes_and_removal() {
        let mut app = App::new();
        app.init_resource::<FullMapState>()
            .init_resource::<Map3dRenderStatus>()
            .init_resource::<Assets<Image>>()
            .add_systems(Update, miniature_environment);
        {
            let mut map = app.world_mut().resource_mut::<FullMapState>();
            map.open = true;
            map.three_dimensional = true;
        }
        let environment = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            EnvironmentMapLight {
                diffuse_map: images.add(Image::default()),
                specular_map: images.add(Image::default()),
                intensity: 1.05,
                rotation: Quat::IDENTITY,
                affects_lightmapped_mesh_diffuse: true,
            }
        };
        let main = app
            .world_mut()
            .spawn((terra_player::TerraCamera, environment.clone()))
            .id();
        let miniature = app.world_mut().spawn(MiniatureCamera).id();
        app.update();
        assert!(same_environment(
            app.world().get::<EnvironmentMapLight>(miniature).unwrap(),
            &environment
        ));
        assert!(
            app.world()
                .resource::<Map3dRenderStatus>()
                .environment_map_shared
        );
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 2);
        assert!(
            app.world()
                .get::<bevy::light::GeneratedEnvironmentMapLight>(miniature)
                .is_none()
        );
        assert!(
            app.world()
                .get::<bevy::light::AtmosphereEnvironmentMapLight>(miniature)
                .is_none()
        );
        let updated = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            EnvironmentMapLight {
                diffuse_map: images.add(Image::default()),
                specular_map: images.add(Image::default()),
                intensity: 0.08,
                rotation: Quat::from_rotation_y(0.7),
                affects_lightmapped_mesh_diffuse: false,
            }
        };
        app.world_mut().entity_mut(main).insert(updated.clone());
        app.update();
        assert!(same_environment(
            app.world().get::<EnvironmentMapLight>(miniature).unwrap(),
            &updated
        ));
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 4);
        app.world_mut()
            .entity_mut(main)
            .remove::<EnvironmentMapLight>();
        app.update();
        assert!(app.world().get::<EnvironmentMapLight>(miniature).is_none());
        assert!(
            !app.world()
                .resource::<Map3dRenderStatus>()
                .environment_map_shared
        );
        app.world_mut().entity_mut(main).insert(updated);
        app.update();
        assert!(app.world().get::<EnvironmentMapLight>(miniature).is_some());
        app.world_mut().resource_mut::<FullMapState>().open = false;
        app.update();
        assert!(app.world().get::<EnvironmentMapLight>(miniature).is_none());
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 4);
    }
}
