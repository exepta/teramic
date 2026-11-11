//! Live character editor renders the exact world rig, with no screenshot/readback delay.
use crate::players::{AvatarAssets, spawn_parts};
use bevy::{
    camera::{Exposure, Hdr, RenderTarget, visibility::RenderLayers},
    core_pipeline::{prepass::DepthPrepass, tonemapping::Tonemapping},
    prelude::*,
    render::render_resource::TextureFormat,
};
#[derive(Resource, Clone, Copy, PartialEq)]
pub struct RenderCharacterPreviewSelection {
    pub gender: terra_core::PlayerGender,
    pub appearance: terra_core::CharacterAppearance,
    pub rotation: f32,
}
#[derive(Resource, Default)]
pub struct RenderCharacterPreviewImage {
    pub image: Handle<Image>,
    pub ready: bool,
    pub hairstyles: Handle<Image>,
}

const LAYER: usize = 27;
const HAIR_LAYER: usize = 24;
#[derive(Component)]
struct EditorScene;
#[derive(Component)]
struct EditorAvatar;
#[derive(Resource, Default)]
struct SelectionState(
    Option<(terra_core::PlayerGender, terra_core::CharacterAppearance)>,
    u8,
);

pub(super) fn install(app: &mut App) {
    app.init_resource::<RenderCharacterPreviewImage>()
        .init_resource::<SelectionState>()
        .add_systems(Update, (update, animate_editor).chain());
}

#[allow(clippy::too_many_arguments)]
fn update(
    mut commands: Commands,
    selection: Option<Res<RenderCharacterPreviewSelection>>,
    mut image: ResMut<RenderCharacterPreviewImage>,
    mut state: ResMut<SelectionState>,
    mut assets: ResMut<AvatarAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    scene: Query<Entity, With<EditorScene>>,
    mut avatars: Query<&mut Transform, With<EditorAvatar>>,
) {
    let Some(selection) = selection else {
        if state.0.take().is_some() {
            for entity in &scene {
                commands.entity(entity).try_despawn();
            }
            image.ready = false;
            state.1 = 0;
        }
        return;
    };
    let selected = (selection.gender, selection.appearance);
    if state.0 == Some(selected) {
        for mut transform in &mut avatars {
            transform.rotation = Quat::from_rotation_y(selection.rotation);
        }
        state.1 = state.1.saturating_add(1);
        image.ready = state.1 >= 2;
        return;
    }
    for entity in &scene {
        commands.entity(entity).try_despawn();
    }
    state.0 = Some(selected);
    state.1 = 0;
    image.ready = false;
    if image.image == Handle::default() {
        image.image = images.add(Image::new_target_texture(
            640,
            896,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    }
    if image.hairstyles == Handle::default() {
        image.hairstyles = images.add(Image::new_target_texture(
            1000,
            600,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    }
    for style in 0..terra_core::CharacterAppearance::hairstyle_count(selection.gender) {
        let appearance = terra_core::CharacterAppearance {
            hairstyle: style,
            ..selection.appearance
        };
        if let Ok(head) = crate::players::model::head(
            selection.gender == terra_core::PlayerGender::Female,
            appearance,
        ) {
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
                head.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                continue;
            };
            let (minimum, maximum) = positions.iter().fold(
                (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                |(low, high), position| {
                    let point = Vec3::from_array(*position);
                    (low.min(point), high.max(point))
                },
            );
            // Fit each real hairstyle inside its atlas tile, including braids and buns.
            let scale = 0.9 / (maximum - minimum).x.max((maximum - minimum).y).max(0.001);
            let center = (minimum + maximum) * 0.5;
            let position = Vec3::new(2.0 - f32::from(style % 5), 1.0 - f32::from(style / 5), 0.0)
                - Vec3::new(center.x, center.y, 0.0) * scale;
            let mesh = meshes.add(head);
            commands.spawn((
                EditorScene,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(assets.material.clone()),
                Transform::from_translation(position).with_scale(Vec3::splat(scale)),
                RenderLayers::layer(HAIR_LAYER),
                bevy::light::NotShadowReceiver,
            ));
        }
    }
    commands.spawn((
        EditorScene,
        Camera3d::default(),
        Camera {
            order: -13,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        RenderTarget::Image(image.hairstyles.clone().into()),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                viewport_height: 3.0,
            },
            near: 0.01,
            far: 30.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 0.0, -5.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(HAIR_LAYER),
        AmbientLight {
            brightness: 950.0,
            color: Color::srgb(1.0, 0.96, 0.90),
            ..default()
        },
        Exposure { ev100: 13.7 },
        (Hdr, Tonemapping::AcesFitted, Msaa::Off),
    ));
    assets.prepare(&mut meshes, selection.gender, selection.appearance);
    let root = commands
        .spawn((
            EditorScene,
            EditorAvatar,
            Transform::from_rotation(Quat::from_rotation_y(selection.rotation)),
            Visibility::Visible,
        ))
        .id();
    spawn_parts(
        &mut commands,
        &assets,
        root,
        RenderLayers::layer(LAYER),
        selection.gender,
        selection.appearance,
    );
    commands.spawn((
        EditorScene,
        Camera3d::default(),
        Camera {
            order: -12,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        RenderTarget::Image(image.image.clone().into()),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                viewport_height: 2.15,
            },
            near: 0.01,
            far: 30.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 0.05, -5.0).looking_at(Vec3::new(0.0, 0.05, 0.0), Vec3::Y),
        RenderLayers::layer(LAYER),
        AmbientLight {
            brightness: 950.0,
            color: Color::srgb(1.0, 0.96, 0.90),
            ..default()
        },
        Exposure { ev100: 13.7 },
        (Hdr, Tonemapping::AcesFitted, Msaa::Off, DepthPrepass),
    ));
    for (position, illuminance, color) in [
        (
            Vec3::new(-3.0, 6.0, -5.0),
            100_000.0,
            Color::srgb(1.0, 0.94, 0.83),
        ),
        (
            Vec3::new(4.0, -2.0, -4.0),
            28_000.0,
            Color::srgb(0.90, 0.95, 1.0),
        ),
    ] {
        commands.spawn((
            EditorScene,
            DirectionalLight {
                illuminance,
                color,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(position).looking_at(Vec3::ZERO, Vec3::Y),
            RenderLayers::layer(LAYER).with(HAIR_LAYER),
        ));
    }
}

fn animate_editor(
    time: Res<Time>,
    roots: Query<(), With<EditorAvatar>>,
    mut joints: Query<(
        &crate::players::AvatarPart,
        &crate::players::AvatarJoint,
        &mut Transform,
    )>,
) {
    use crate::players::AvatarPart;
    let clock = time.elapsed_secs();
    for (part, joint, mut transform) in &mut joints {
        if !roots.contains(joint.root) {
            continue;
        }
        match part {
            AvatarPart::Head => {
                transform.rotation = Quat::from_euler(
                    EulerRot::YXZ,
                    (clock * 0.41).sin() * 0.10,
                    (clock * 1.7).sin() * 0.018,
                    (clock * 0.83).sin() * 0.008,
                )
            }
            AvatarPart::Body => {
                transform.scale =
                    joint.scale * Vec3::new(1.0, 1.0 + (clock * 1.8).sin() * 0.003, 1.0)
            }
            AvatarPart::Arm(side) => {
                transform.rotation = Quat::from_euler(
                    EulerRot::XYZ,
                    -0.06,
                    0.0,
                    f32::from(*side) * (0.045 + (clock * 1.7).sin() * 0.005),
                )
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_turns_the_real_rig_reuses_target_and_cleans_scene_when_closed() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<RenderCharacterPreviewImage>()
            .init_resource::<SelectionState>()
            .insert_resource(AvatarAssets {
                skin_eyelids: Default::default(),
                parts: [Vec::new(), Vec::new()],
                custom: default(),
                eyelids: None,
                material: Handle::default(),
            })
            .insert_resource(RenderCharacterPreviewSelection {
                gender: terra_core::PlayerGender::Male,
                appearance: default(),
                rotation: 0.0,
            })
            .add_systems(Update, (update, animate_editor).chain());
        app.update();
        let hairstyle_image = app
            .world()
            .resource::<RenderCharacterPreviewImage>()
            .hairstyles
            .clone();
        assert_eq!(
            app.world()
                .resource::<Assets<Image>>()
                .get(&hairstyle_image)
                .unwrap()
                .size(),
            UVec2::new(1000, 600)
        );
        let target = app
            .world()
            .resource::<RenderCharacterPreviewImage>()
            .image
            .clone();
        app.world_mut()
            .resource_mut::<RenderCharacterPreviewSelection>()
            .rotation = 1.2;
        app.update();
        let world = app.world_mut();
        let rotation = world
            .query_filtered::<&Transform, With<EditorAvatar>>()
            .single(world)
            .unwrap()
            .rotation;
        assert!(rotation.abs_diff_eq(Quat::from_rotation_y(1.2), 1e-5));
        assert_eq!(world.resource::<Assets<Image>>().len(), 2);
        world
            .resource_mut::<RenderCharacterPreviewSelection>()
            .appearance
            .hair_color = [200, 40, 20];
        app.update();
        assert_eq!(
            app.world().resource::<RenderCharacterPreviewImage>().image,
            target
        );
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 2);
        app.world_mut()
            .remove_resource::<RenderCharacterPreviewSelection>();
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<EditorScene>>()
                .iter(world)
                .count(),
            0
        );
        assert!(!world.resource::<RenderCharacterPreviewImage>().ready);
    }
}
