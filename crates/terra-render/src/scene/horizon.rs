//! A camera-centered haze shell joins finite streaming to the planetary sky.
//! It draws behind terrain/water and never writes depth, shadows or gameplay data.
use bevy::{
    camera::visibility::NoFrustumCulling,
    light::{NotShadowCaster, NotShadowReceiver},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use terra_core::GameConfig;
use terra_player::TerraCamera;

// Reserve the back of the transparent phase independently of proxy mesh centers.
// Finite headroom avoids overflow when Bevy adds its view-space center distance.
pub(crate) const BACKGROUND_SORT_BIAS: f32 = -1.0e20;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
struct HazeExtension {
    #[uniform(100)]
    color: Vec4,
    #[uniform(101)]
    moon: Vec4,
    #[uniform(102)]
    sky: Vec4,
}
impl MaterialExtension for HazeExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/terra_horizon.wesl".into()
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        if let Some(depth) = &mut descriptor.depth_stencil {
            // The bias sorts the camera-centered shell behind transparent
            // terrain; raster depth must still come from its actual far surface.
            depth.bias = default();
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}
type HazeMaterial = ExtendedMaterial<StandardMaterial, HazeExtension>;
#[derive(Component)]
struct Haze;

pub(crate) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<HazeMaterial>::default())
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, update.before(TransformSystems::Propagate));
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<HazeMaterial>>,
) {
    commands.spawn((
        Name::new("Distant horizon haze"),
        Haze,
        Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(32, 16))),
        MeshMaterial3d(materials.add(ExtendedMaterial {
            base: StandardMaterial {
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                fog_enabled: false,
                depth_bias: BACKGROUND_SORT_BIAS,
                ..default()
            },
            extension: HazeExtension {
                color: Vec4::ZERO,
                moon: Vec4::ZERO,
                sky: Vec4::ZERO,
            },
        })),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
        NotShadowReceiver,
        NoFrustumCulling,
    ));
}

type CameraData = (
    &'static Transform,
    &'static Projection,
    &'static DistanceFog,
);
type MainCamera = (With<TerraCamera>, Without<Haze>);
fn update(
    config: Res<GameConfig>,
    camera: Query<CameraData, MainCamera>,
    mut haze: Query<
        (
            &mut Transform,
            &mut Visibility,
            &MeshMaterial3d<HazeMaterial>,
        ),
        With<Haze>,
    >,
    mut materials: ResMut<Assets<HazeMaterial>>,
    daylight: Res<super::daylight::Daylight>,
    underwater: Res<crate::water::UnderwaterState>,
) {
    let Some((camera, projection, fog)) = camera.iter().next() else {
        return;
    };
    let Projection::Perspective(projection) = projection else {
        return;
    };
    static LEGACY: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        std::env::var_os("TERRA_BENCH_DISABLE_HORIZON").is_some()
            && std::env::args().any(|arg| arg.starts_with("--smoke-test"))
    });
    let legacy = *LEGACY;
    for (mut transform, mut visibility, handle) in &mut haze {
        *visibility = if legacy {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if transform.translation != camera.translation {
            transform.translation = camera.translation;
        }
        let scale = Vec3::splat(projection.far * 0.96);
        if transform.scale != scale {
            transform.scale = scale;
        }
        let mut color = fog.color.to_linear().to_vec4();
        color.w = if config.client.fog_distance == 0.0 {
            0.0
        } else {
            1.0
        };
        let moon = (-daylight.sun).extend((1.0 - daylight.daylight) * (1.0 - underwater.blend));
        let sky = Vec4::new(
            daylight.rotation,
            1.0 - underwater.blend,
            underwater.blend,
            underwater.depth.max(0.0),
        );
        // Cloud centers can lie farther along the view than projection.far.
        // Keep the celestial background first without changing pipeline keys on motion.
        let sort_bias = BACKGROUND_SORT_BIAS;
        if materials.get(&handle.0).is_some_and(|material| {
            material.extension.color != color
                || material.extension.moon != moon
                || material.extension.sky != sky
                || material.base.depth_bias != sort_bias
        }) && let Some(mut material) = materials.get_mut(&handle.0)
        {
            material.extension.color = color;
            material.extension.moon = moon;
            material.extension.sky = sky;
            material.base.depth_bias = sort_bias;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_sort_bias_and_material_pipeline_key_do_not_follow_the_camera() {
        let mut app = App::new();
        app.init_resource::<GameConfig>()
            .init_resource::<super::super::daylight::Daylight>()
            .init_resource::<crate::water::UnderwaterState>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<HazeMaterial>>()
            .add_systems(Startup, setup)
            .add_systems(Update, update);
        let camera = app
            .world_mut()
            .spawn((
                TerraCamera,
                Transform::from_xyz(1150.5, 145.0, 480.5),
                Projection::Perspective(PerspectiveProjection {
                    far: 64.0,
                    ..default()
                }),
                DistanceFog::default(),
            ))
            .id();
        let mut keys = std::collections::HashSet::new();
        for (height, far) in [
            (145.0, 64.0),
            (-128.0, 32.0),
            (256.0, 272.0),
            (400.0, 1024.0),
            (100_000.0, 64.0),
        ] {
            *app.world_mut().get_mut::<Transform>(camera).unwrap() =
                Transform::from_xyz(1150.5, height, 480.5)
                    .looking_to(Vec3::new(0.0, 0.9, 0.4358899), Vec3::Y);
            *app.world_mut().get_mut::<Projection>(camera).unwrap() =
                Projection::Perspective(PerspectiveProjection { far, ..default() });
            app.update();
            let materials = app.world().resource::<Assets<HazeMaterial>>();
            assert_eq!(materials.len(), 1);
            let (_, material) = materials.iter().next().unwrap();
            assert_eq!(material.base.depth_bias, BACKGROUND_SORT_BIAS);
            assert!(material.base.depth_bias.is_finite());
            keys.insert(bevy::pbr::StandardMaterialKey::from(&material.base));
        }
        assert_eq!(keys.len(), 1);
    }
}
