//! One bounded render target displays the real authored assembly in the workshop.
use super::*;
use bevy::{
    camera::{Exposure, RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    render::render_resource::TextureFormat,
};
use terra_core::train_blueprint::WorkshopPreviewRequest;
const LAYER: usize = 28;
#[derive(Resource, Default)]
pub struct WorkshopPreviewImage {
    pub image: Handle<Image>,
    pub ready: bool,
}
#[derive(Resource, Default)]
struct Preview {
    key: WorkshopPreviewRequest,
    task: Option<(
        WorkshopPreviewRequest,
        Task<Result<PreparedGeometry, String>>,
    )>,
    root: Option<Entity>,
    camera: Option<Entity>,
    light: Option<Entity>,
    environment: Handle<Image>,
    meshes: Vec<Handle<Mesh>>,
}
fn studio_environment() -> Image {
    use bevy::render::render_resource::{
        Extent3d, TextureDimension, TextureViewDescriptor, TextureViewDimension,
    };
    let mut data = Vec::with_capacity(16 * 16 * 6 * 4);
    for face in 0..6 {
        for y in 0..16 {
            for _ in 0..16 {
                let light = match face {
                    2 => 240,
                    3 => 60,
                    _ => 190 - y * 5,
                } as u8;
                data.extend_from_slice(&[
                    light,
                    light.saturating_sub(5),
                    light.saturating_sub(12),
                    255,
                ]);
            }
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    );
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    image
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<WorkshopPreviewImage>()
        .init_resource::<Preview>()
        .init_resource::<WorkshopPreviewRequest>()
        .add_systems(Update, synchronize.after(super::synchronize));
}
#[allow(clippy::too_many_arguments)]
fn synchronize(
    mut commands: Commands,
    request: Res<WorkshopPreviewRequest>,
    blocks: Res<BlockAssets>,
    materials: Res<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut image: ResMut<WorkshopPreviewImage>,
    mut state: ResMut<Preview>,
    mut cameras: Query<&mut Camera>,
    mut roots: Query<&mut Visibility>,
) {
    if let Some(camera) = state.camera
        && let Ok(mut camera) = cameras.get_mut(camera)
    {
        camera.is_active = request.visible;
    }
    if let Some(root) = state.root
        && let Ok(mut visibility) = roots.get_mut(root)
    {
        *visibility = if request.visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if !request.visible {
        return;
    }
    if state.environment == Handle::default() {
        state.environment = images.add(studio_environment());
    }
    if image.image == Handle::default() {
        image.image = images.add(Image::new_target_texture(
            1024,
            576,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    }
    if let Some((key, task)) = state.task.as_mut() {
        let Some(result) = future::block_on(future::poll_once(task)) else {
            return;
        };
        let key = key.clone();
        state.task = None;
        if key != *request {
            return;
        }
        state.key = key;
        let prepared = match result {
            Ok(prepared) => prepared,
            Err(error) => {
                warn!(%error, "Workshop preview geometry failed");
                return;
            }
        };
        if let Some(root) = state.root.take() {
            commands.entity(root).despawn();
        }
        if let Some(camera) = state.camera.take() {
            commands.entity(camera).despawn();
        }
        if let Some(light) = state.light.take() {
            commands.entity(light).despawn();
        }
        for mesh in state.meshes.drain(..) {
            meshes.remove(mesh.id());
        }
        let root = commands
            .spawn((
                Name::new("Workshop vehicle preview"),
                Transform::default(),
                Visibility::Visible,
            ))
            .id();
        let mut low = Vec3::splat(f32::INFINITY);
        let mut high = Vec3::splat(f32::NEG_INFINITY);
        for batch in prepared.batches {
            for point in &batch.positions {
                let point = Vec3::from_array(*point);
                low = low.min(point);
                high = high.max(point);
            }
            let mesh = meshes.add(crate::previews::batch_mesh(&batch));
            state.meshes.push(mesh.clone());
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(blocks.materials[batch.material].clone()),
                Transform::default(),
                RenderLayers::layer(LAYER),
                ChildOf(root),
            ));
        }
        if !low.is_finite() || !high.is_finite() {
            commands.entity(root).despawn();
            return;
        }
        let center = (low + high) * 0.5;
        let diagonal = (high - low).length().max(3.0);
        let transform = Transform::from_translation(
            center + Vec3::new(0.9, 0.55, 1.25).normalize() * diagonal * 2.0,
        )
        .looking_at(center, Vec3::Y);
        let inverse = transform.rotation.inverse();
        let mut projected_low = Vec3::splat(f32::INFINITY);
        let mut projected_high = Vec3::splat(f32::NEG_INFINITY);
        for corner in 0..8 {
            let point = Vec3::new(
                if corner & 1 == 0 { low.x } else { high.x },
                if corner & 2 == 0 { low.y } else { high.y },
                if corner & 4 == 0 { low.z } else { high.z },
            );
            let projected = inverse * (point - center);
            projected_low = projected_low.min(projected);
            projected_high = projected_high.max(projected);
        }
        let span = projected_high - projected_low;
        state.camera = Some(
            commands
                .spawn((
                    Name::new("Workshop preview camera"),
                    Camera3d::default(),
                    Camera {
                        order: -13,
                        clear_color: ClearColorConfig::Custom(Color::NONE),
                        ..default()
                    },
                    RenderTarget::Image(image.image.clone().into()),
                    Projection::Orthographic(OrthographicProjection {
                        scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                            viewport_height: (span.y.max(span.x / (1024.0 / 576.0)) * 1.12)
                                .max(1.0),
                        },
                        near: 0.01,
                        far: diagonal * 5.0 + 10.0,
                        ..OrthographicProjection::default_3d()
                    }),
                    transform,
                    RenderLayers::layer(LAYER),
                    AmbientLight {
                        brightness: 900.0,
                        ..default()
                    },
                    Exposure { ev100: 12.8 },
                    bevy::light::EnvironmentMapLight {
                        diffuse_map: state.environment.clone(),
                        specular_map: state.environment.clone(),
                        intensity: 18000.0,
                        ..default()
                    },
                    (bevy::camera::Hdr, Tonemapping::AcesFitted, Msaa::Off),
                ))
                .id(),
        );
        state.light = Some(
            commands
                .spawn((
                    DirectionalLight {
                        illuminance: 100_000.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(6.0, 12.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
                    RenderLayers::layer(LAYER),
                ))
                .id(),
        );
        state.root = Some(root);
        image.ready = true;
    }
    if state.task.is_none() && state.key != *request {
        let Some(templates) = blocks.templates.clone() else {
            return;
        };
        let palettes = openings::palettes(&blocks, &materials);
        let key = request.clone();
        let parts = key.parts.clone();
        let frozen = key.blocks.clone();
        state.task = Some((
            key,
            AsyncComputeTaskPool::get().spawn(async move {
                compose_geometry(&parts, &frozen, &templates, [0; 3], &palettes, false)
            }),
        ));
    }
}
