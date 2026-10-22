//! Short native GPU regression for climate metadata versus chunk fade opacity.
use crate::weather::surfaces::{WetExtension, WetMaterial};
use bevy::{
    camera::{RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::{
        render_resource::TextureFormat,
        view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    },
};
use std::{path::PathBuf, time::Instant};

#[derive(Resource)]
struct Probe {
    material: Handle<WetMaterial>,
    image: Handle<Image>,
    phase: usize,
    requested: bool,
    started: Instant,
    changed: Instant,
}
const OPACITY: [f32; 3] = [0.0, 0.25, 1.0];
pub(super) fn install(app: &mut App) {
    if std::env::args().any(|arg| arg == "--smoke-test-wet-opacity") {
        app.add_systems(Startup, setup).add_systems(Last, capture);
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<WetMaterial>>,
) {
    let image = images.add(Image::new_target_texture(
        256,
        128,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let material = materials.add(WetMaterial {
        base: StandardMaterial {
            base_color: Color::srgba(0.0, 1.0, 0.0, 0.0),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        },
        extension: WetExtension::default(),
    });
    for (index, metadata) in [0.0, 0.5, 1.0].into_iter().enumerate() {
        let mut mesh = Mesh::from(Rectangle::new(0.8, 0.8));
        let vertices = mesh.count_vertices();
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, vec![[0.0_f32; 2]; vertices]);
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_COLOR,
            vec![[1.0_f32, 1.0, 1.0, metadata]; vertices],
        );
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(index as f32 - 1.0, 0.0, 0.0),
            RenderLayers::layer(58),
        ));
    }
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -20,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        RenderTarget::Image(image.clone().into()),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                viewport_height: 1.6,
            },
            near: 0.01,
            far: 10.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 0.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(58),
        Msaa::Off,
        Tonemapping::None,
    ));
    commands.insert_resource(Probe {
        material,
        image,
        phase: 0,
        requested: false,
        started: Instant::now(),
        changed: Instant::now(),
    });
}

fn capture(
    mut commands: Commands,
    mut probe: ResMut<Probe>,
    previews: Res<crate::RenderBlockPreviews>,
    mut exit: MessageWriter<bevy::app::AppExit>,
) {
    if probe.started.elapsed().as_secs_f32() > 30.0 {
        error!("Wet opacity GPU regression timed out");
        exit.write(bevy::app::AppExit::error());
        return;
    }
    if probe.phase >= OPACITY.len() {
        if previews.pending == 0 && probe.changed.elapsed().as_secs_f32() > 0.5 {
            exit.write(bevy::app::AppExit::Success);
        }
        return;
    }
    if probe.requested || probe.changed.elapsed().as_secs_f32() < 1.0 {
        return;
    }
    probe.requested = true;
    let phase = probe.phase;
    let root = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-wet-opacity"));
    if let Err(error) = std::fs::create_dir_all(&root) {
        error!(%error,"Cannot create opacity capture directory");
        exit.write(bevy::app::AppExit::error());
        return;
    }
    commands
        .spawn(Screenshot::image(probe.image.clone()))
        .observe(save_to_disk(root.join(format!("wet-opacity-{phase}.png"))))
        .observe(
            move |event: On<ScreenshotCaptured>,
                  mut state: ResMut<Probe>,
                  mut materials: ResMut<Assets<WetMaterial>>,
                  mut exit: MessageWriter<bevy::app::AppExit>| {
                if state.phase != phase {
                    return;
                }
                let Some(bytes) = event.image.data.as_deref() else {
                    exit.write(bevy::app::AppExit::error());
                    return;
                };
                let samples: Vec<_> = [48_usize, 128, 208]
                    .into_iter()
                    .filter_map(|x| bytes.get((64 * 256 + x) * 4..(64 * 256 + x) * 4 + 4))
                    .map(|pixel| pixel[1])
                    .collect();
                let valid = samples.len() == 3
                    && samples.iter().all(|&g| match phase {
                        0 => g < 4,
                        1 => (90..220).contains(&g),
                        _ => g > 245,
                    });
                if !valid {
                    error!(
                        phase,
                        ?samples,
                        "Climate metadata overrode real GPU material opacity"
                    );
                    exit.write(bevy::app::AppExit::error());
                    return;
                }
                info!(
                    phase,
                    opacity = OPACITY[phase],
                    ?samples,
                    "Wet terrain opacity GPU pixels verified for climate alpha 0/0.5/1"
                );
                state.phase += 1;
                state.requested = false;
                state.changed = Instant::now();
                if state.phase < OPACITY.len()
                    && let Some(mut material) = materials.get_mut(&state.material)
                {
                    material.base.base_color.set_alpha(OPACITY[state.phase]);
                }
            },
        );
}
