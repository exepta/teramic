//! Opt-in GPU gallery: cargo run -p terra-client --example profession_station_gallery -- output.png
use bevy::{
    asset::LoadState,
    camera::primitives::Aabb,
    gltf::{
        GltfAssetLabel, GltfExtras, GltfMaterialExtras, GltfMaterialName, GltfMeshExtras,
        GltfMeshName, GltfSceneExtras, GltfSceneName,
    },
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
    transform::components::TransformTreeChanged,
    world_serialization::{WorldInstance, WorldInstanceSpawner},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

const STATIONS: &str = "lumberjack smith market army railway mine butcher bishop farm";

#[derive(Resource)]
struct Gallery {
    output: PathBuf,
    started: Instant,
    settled: Option<Instant>,
    requested: bool,
}

fn main() -> AppExit {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("TERRA_CAPTURE_DIR").map_or_else(
                || PathBuf::from("profession-stations.png"),
                |directory| PathBuf::from(directory).join("profession-stations.png"),
            )
        });
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty())
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        eprintln!("Cannot create gallery output directory: {error}");
        return AppExit::error();
    }
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: terra_core::asset_directory().to_string_lossy().into_owned(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Terra profession stations — native GLB gallery".into(),
                        resolution: (1600, 1200).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        // Default features omit automatic reflection registration. These are the
        // components authored by the static glTF loader, including required ones.
        .register_type::<Transform>()
        .register_type::<GlobalTransform>()
        .register_type::<TransformTreeChanged>()
        .register_type::<Name>()
        .register_type::<ChildOf>()
        .register_type::<Children>()
        .register_type::<Visibility>()
        .register_type::<InheritedVisibility>()
        .register_type::<ViewVisibility>()
        .register_type::<Mesh3d>()
        .register_type::<MeshMaterial3d<StandardMaterial>>()
        .register_type::<Aabb>()
        .register_type::<GltfExtras>()
        .register_type::<GltfSceneExtras>()
        .register_type::<GltfSceneName>()
        .register_type::<GltfMeshExtras>()
        .register_type::<GltfMeshName>()
        .register_type::<GltfMaterialExtras>()
        .register_type::<GltfMaterialName>()
        .insert_resource(ClearColor(Color::srgb(0.16, 0.19, 0.23)))
        .insert_resource(Gallery {
            output,
            started: Instant::now(),
            settled: None,
            requested: false,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, capture_when_ready)
        .run()
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let pedestal = meshes.add(Cuboid::new(1.5, 0.12, 1.5));
    let stone = materials.add(Color::srgb(0.46, 0.44, 0.40));
    for (index, name) in STATIONS.split_ascii_whitespace().enumerate() {
        let x = (index % 3) as f32 * 2.25 - 2.25;
        let z = (index / 3) as f32 * 2.25 - 2.25;
        commands.spawn((
            Mesh3d(pedestal.clone()),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(x, -0.06, z),
        ));
        let scene = assets.load(
            GltfAssetLabel::Scene(0).from_asset(format!("terra/blocks/models/{name}_station.glb")),
        );
        // Bevy 0.20 calls the former SceneRoot component WorldAssetRoot.
        commands.spawn((
            WorldAssetRoot(scene),
            Transform::from_xyz(x - 0.5, 0., z - 0.5),
        ));
    }
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(30., 30.))),
        MeshMaterial3d(materials.add(Color::srgb(0.23, 0.25, 0.27))),
        Transform::from_xyz(0., -0.13, 0.),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 16_000.,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-4., 8., 6.).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Camera3d::default(),
        AmbientLight {
            brightness: 250.,
            ..default()
        },
        Transform::from_xyz(7.8, 8.8, 11.).looking_at(Vec3::new(0., 0.35, 0.), Vec3::Y),
    ));
    commands.spawn((Text::new("BACK: Lumberjack · Smith · Market\nMIDDLE: Army · Railway · Mine\nFRONT: Butcher · Bishop · Farmer"),
        TextFont { font_size: 20.0.into(), ..default() }, Node { position_type: PositionType::Absolute, left: px(20), bottom: px(20), ..default() }));
}

fn capture_when_ready(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut gallery: ResMut<Gallery>,
    scenes: Query<(&WorldAssetRoot, Option<&WorldInstance>)>,
    instances: Res<WorldInstanceSpawner>,
    mut exit: MessageWriter<AppExit>,
) {
    if gallery.started.elapsed() > Duration::from_secs(90) {
        error!("Gallery timed out before all nine models and the screenshot completed");
        exit.write(AppExit::error());
        return;
    }
    if gallery.requested {
        return;
    }
    if scenes
        .iter()
        .any(|(root, _)| matches!(assets.load_state(root.0.id()), LoadState::Failed(_)))
    {
        error!("Profession station GLB failed to load");
        exit.write(AppExit::error());
        return;
    }
    let ready = scenes
        .iter()
        .filter(|(root, instance)| {
            assets.is_loaded_with_dependencies(root.0.id())
                && instance.is_some_and(|instance| instances.instance_is_ready(**instance))
        })
        .count();
    if scenes.iter().count() != 9 || ready != 9 {
        gallery.settled = None;
        return;
    }
    // Let GPU uploads, material pipelines and shadows settle after scene instantiation.
    if gallery.settled.get_or_insert_with(Instant::now).elapsed() < Duration::from_secs(3) {
        return;
    }
    gallery.requested = true;
    commands.spawn(Screenshot::primary_window()).observe(
        |capture: On<ScreenshotCaptured>,
         gallery: Res<Gallery>,
         mut exit: MessageWriter<AppExit>| {
            let result = capture
                .image
                .clone()
                .try_into_dynamic()
                .map_err(|error| error.to_string())
                .and_then(|image| {
                    image
                        .to_rgb8()
                        .save(&gallery.output)
                        .map_err(|error| error.to_string())
                });
            match result {
                Ok(()) => {
                    info!(path = %gallery.output.display(), models = 9, "Profession gallery saved");
                    exit.write(AppExit::Success);
                }
                Err(error) => {
                    error!(%error, "Cannot save profession gallery screenshot");
                    exit.write(AppExit::error());
                }
            }
        },
    );
}
