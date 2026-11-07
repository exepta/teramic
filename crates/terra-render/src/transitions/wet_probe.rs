//! Native pixel regression for wet opacity and the real depth/normal/motion passes.
use crate::weather::surfaces::{WetExtension, WetMaterial};
use bevy::{
    asset::RenderAssetUsages,
    camera::{RenderTarget, visibility::RenderLayers},
    core_pipeline::{
        prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass},
        tonemapping::Tonemapping,
    },
    prelude::*,
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat},
        view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    },
};
use std::{path::PathBuf, time::Instant};

#[derive(Resource)]
struct Probe {
    material: Handle<WetMaterial>,
    image: Handle<Image>,
    normal: Handle<Image>,
    camera: Entity,
    phase: usize,
    requested: bool,
    started: Instant,
    changed: Instant,
}
const OPACITY: [f32; 3] = [0.0, 0.25, 1.0];
const PHASE_COUNT: usize = 9;

#[derive(Clone, Copy)]
struct Phase {
    alpha_mode: AlphaMode,
    opacity: f32,
    normal_motion: bool,
    normal_map: u8,
}

fn phase(index: usize) -> Phase {
    let mut phase = Phase {
        alpha_mode: AlphaMode::Opaque,
        opacity: 1.0,
        normal_motion: false,
        normal_map: 0,
    };
    match index {
        0..=2 => {
            phase.alpha_mode = AlphaMode::Blend;
            phase.opacity = OPACITY[index];
        }
        4 | 8 => {
            phase.alpha_mode = AlphaMode::Mask(0.0);
            phase.opacity = 0.0;
        }
        5 => {
            phase.alpha_mode = AlphaMode::Mask(0.4);
            phase.opacity = 0.0;
        }
        _ => {}
    }
    phase.normal_motion = index >= 6;
    phase.normal_map = match index {
        7 => 1,
        8 => 2,
        _ => 0,
    };
    phase
}

fn pixels_match(index: usize, samples: &[u8]) -> bool {
    samples.len() == 3
        && samples.iter().all(|&green| match index {
            0 | 5 => green < 4,
            1 => (90..220).contains(&green),
            _ => green > 245,
        })
}
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
    let normal = images.add(Image::new_fill(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[128, 128, 255, 255],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
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
            Mesh::ATTRIBUTE_TANGENT,
            vec![[1.0_f32, 0.0, 0.0, 1.0]; vertices],
        );
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
    let camera = commands
        .spawn((
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
            DepthPrepass,
            Tonemapping::None,
        ))
        .id();
    commands.insert_resource(Probe {
        material,
        image,
        normal,
        camera,
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
    if probe.started.elapsed().as_secs_f32() > 120.0 {
        error!("Wet opacity GPU regression timed out");
        exit.write(bevy::app::AppExit::error());
        return;
    }
    if probe.phase >= PHASE_COUNT {
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
                  mut commands: Commands,
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
                if !pixels_match(phase, &samples) {
                    error!(
                        phase,
                        ?samples,
                        "Wet color/depth/normal/motion GPU regression failed"
                    );
                    exit.write(bevy::app::AppExit::error());
                    return;
                }
                info!(
                    phase,
                    alpha_mode = ?self::phase(phase).alpha_mode,
                    opacity = self::phase(phase).opacity,
                    normal_motion = self::phase(phase).normal_motion,
                    normal_map = self::phase(phase).normal_map,
                    ?samples,
                    "Wet terrain GPU pixels verified for climate alpha 0/0.5/1"
                );
                state.phase += 1;
                state.requested = false;
                state.changed = Instant::now();
                if state.phase < PHASE_COUNT
                    && let Some(mut material) = materials.get_mut(&state.material)
                {
                    let next = self::phase(state.phase);
                    material.base.base_color.set_alpha(next.opacity);
                    material.base.alpha_mode = next.alpha_mode;
                    material.base.normal_map_texture =
                        (next.normal_map != 0).then(|| state.normal.clone());
                    material.base.normal_map_channel = if next.normal_map == 2 {
                        bevy::mesh::UvChannel::Uv1
                    } else {
                        bevy::mesh::UvChannel::Uv0
                    };
                    if next.normal_motion {
                        commands
                            .entity(state.camera)
                            .insert((NormalPrepass, MotionVectorPrepass));
                    }
                }
            },
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_probe_covers_opaque_depth_mask_zero_and_both_normal_mapping_uv_sets() {
        let opaque = phase(3);
        assert_eq!(opaque.alpha_mode, AlphaMode::Opaque);
        assert!(!opaque.normal_motion);
        assert_eq!(opaque.normal_map, 0);
        let zero = phase(4);
        assert_eq!(zero.alpha_mode, AlphaMode::Mask(0.0));
        assert_eq!(zero.opacity, 0.0);
        assert!(pixels_match(4, &[255; 3]));
        assert!(!pixels_match(4, &[0; 3]));
        assert_eq!(phase(5).alpha_mode, AlphaMode::Mask(0.4));
        assert!(pixels_match(5, &[0; 3]));
        assert!(!pixels_match(5, &[255; 3]));
        for index in 6..PHASE_COUNT {
            assert!(phase(index).normal_motion);
            assert_eq!(phase(index).normal_map, (index - 6) as u8);
            assert!(pixels_match(index, &[255; 3]));
            assert!(!pixels_match(index, &[0; 3]));
        }
    }
}
