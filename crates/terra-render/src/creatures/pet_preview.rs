//! One bounded portrait scene; its shared creature meshes are never duplicated.
use super::{Models, instance_scale, model};
use bevy::{
    camera::{Exposure, RenderTarget, visibility::RenderLayers},
    mesh::VertexAttributeValues,
    prelude::*,
    render::render_resource::TextureFormat,
};
const LAYER: usize = 29;

#[derive(Resource, Default)]
pub struct RenderPetPortrait {
    pub request: Option<u64>,
    pub image: Handle<Image>,
    pub id: Option<u64>,
    pub ready: bool,
}

#[derive(Component)]
struct PetPreviewScene;
#[derive(Resource, Default)]
struct Preview {
    id: Option<u64>,
    appearance: Option<super::Appearance>,
    camera: Option<Entity>,
    frames: u8,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<Preview>()
        .init_resource::<RenderPetPortrait>()
        .add_systems(Update, update);
}

#[allow(clippy::too_many_arguments)]
fn update(
    mut commands: Commands,
    animals: Option<Res<terra_network::client::ReplicatedCreatures>>,
    models: Res<Models>,
    meshes: Res<Assets<Mesh>>,
    mut target: ResMut<RenderPetPortrait>,
    mut images: ResMut<Assets<Image>>,
    mut state: ResMut<Preview>,
    scene: Query<Entity, With<PetPreviewScene>>,
    mut cameras: Query<&mut Camera>,
) {
    let animal = target
        .request
        .and_then(|id| animals.as_ref()?.creatures.get(&id))
        .map(|t| &t.current);
    let Some(animal) = animal else {
        if state.id.take().is_some() {
            state.appearance = None;
            for entity in &scene {
                commands.entity(entity).try_despawn();
            }
            state.camera = None;
            target.ready = false;
            target.id = None;
        }
        return;
    };
    if state.id == Some(animal.id) && state.appearance == Some(super::Appearance::of(animal)) {
        if state.frames < 4 {
            state.frames += 1;
        } else if let Some(entity) = state.camera
            && let Ok(mut camera) = cameras.get_mut(entity)
        {
            camera.is_active = false;
            target.ready = true;
        }
        return;
    }
    let Some(parts) = models.parts.get(&super::Appearance::of(animal).0) else {
        target.ready = false;
        return;
    };
    for entity in &scene {
        commands.entity(entity).try_despawn();
    }
    if target.image == Handle::default() {
        target.image = images.add(Image::new_target_texture(
            384,
            384,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    }
    target.id = Some(animal.id);
    target.ready = false;
    state.id = Some(animal.id);
    state.appearance = Some(super::Appearance::of(animal));
    state.frames = 0;
    let scale = instance_scale(animal);
    let root = commands
        .spawn((
            PetPreviewScene,
            Transform::from_scale(scale),
            Visibility::Visible,
        ))
        .id();
    let mut minimum = Vec3::splat(f32::INFINITY);
    let mut maximum = Vec3::splat(f32::NEG_INFINITY);
    for (part, offset, mesh) in parts {
        if matches!(
            part,
            model::Part::Eyelids | model::Part::Proboscis | model::Part::Silk
        ) {
            continue;
        }
        if let Some(VertexAttributeValues::Float32x3(positions)) = meshes
            .get(mesh)
            .and_then(|m| m.attribute(Mesh::ATTRIBUTE_POSITION))
        {
            for position in positions {
                let position = (Vec3::from_array(*position) + *offset) * scale;
                minimum = minimum.min(position);
                maximum = maximum.max(position);
            }
        }
        commands.spawn((
            PetPreviewScene,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(models.material_for(animal.kind, animal.variant, *part)),
            Transform::from_translation(*offset),
            RenderLayers::layer(LAYER),
            ChildOf(root),
        ));
    }
    let size = Vec3::from_array(animal.size().map(|v| v as f32));
    let (center, height) = portrait_frame(minimum, maximum, size);
    state.camera = Some(
        commands
            .spawn((
                PetPreviewScene,
                Camera3d::default(),
                Camera {
                    order: -20,
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                    ..default()
                },
                RenderTarget::Image(target.image.clone().into()),
                Projection::Orthographic(OrthographicProjection {
                    scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                        viewport_height: height,
                    },
                    near: 0.01,
                    far: 30.0,
                    ..OrthographicProjection::default_3d()
                }),
                Transform::from_translation(center + Vec3::new(3.0, 1.3, -5.0))
                    .looking_at(center, Vec3::Y),
                RenderLayers::layer(LAYER),
                AmbientLight {
                    brightness: 900.0,
                    ..default()
                },
                Exposure { ev100: 13.7 },
                Msaa::Off,
            ))
            .id(),
    );
    commands.spawn((
        PetPreviewScene,
        DirectionalLight {
            illuminance: 90_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-3.0, 6.0, -5.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(LAYER),
    ));
}

fn portrait_frame(minimum: Vec3, maximum: Vec3, fallback: Vec3) -> (Vec3, f32) {
    let (minimum, maximum) = if minimum.is_finite() && maximum.is_finite() {
        (minimum, maximum)
    } else {
        (
            Vec3::new(-fallback.x * 0.5, 0.0, -fallback.z * 0.5),
            Vec3::new(fallback.x * 0.5, fallback.y, fallback.z * 0.5),
        )
    };
    let center = (minimum + maximum) * 0.5;
    let rotation = Transform::from_translation(Vec3::new(3.0, 1.3, -5.0))
        .looking_at(Vec3::ZERO, Vec3::Y)
        .rotation
        .inverse();
    let mut half_extent = 0.0_f32;
    for x in [minimum.x, maximum.x] {
        for y in [minimum.y, maximum.y] {
            for z in [minimum.z, maximum.z] {
                let projected = rotation * (Vec3::new(x, y, z) - center);
                half_extent = half_extent.max(projected.x.abs()).max(projected.y.abs());
            }
        }
    }
    (center, (half_extent * 2.25).max(0.2))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portrait_frames_actual_leg_extents_instead_of_only_the_creature_body() {
        let minimum = Vec3::new(-2.3, -0.2, -2.0);
        let maximum = Vec3::new(2.3, 1.5, 2.0);
        let (center, height) = portrait_frame(minimum, maximum, Vec3::ONE);
        assert!(height > 4.0);
        let rotation = Transform::from_translation(Vec3::new(3.0, 1.3, -5.0))
            .looking_at(Vec3::ZERO, Vec3::Y)
            .rotation
            .inverse();
        for x in [minimum.x, maximum.x] {
            for y in [minimum.y, maximum.y] {
                for z in [minimum.z, maximum.z] {
                    let projected = rotation * (Vec3::new(x, y, z) - center);
                    assert!(projected.x.abs() < height * 0.5 && projected.y.abs() < height * 0.5);
                }
            }
        }
    }
}
