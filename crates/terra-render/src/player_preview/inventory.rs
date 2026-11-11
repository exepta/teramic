//! A live portrait camera exists only while the player inventory is visible.
use crate::players::{AvatarAssets, AvatarGender, spawn_parts};
use bevy::{
    camera::{Exposure, Hdr, RenderTarget, visibility::RenderLayers},
    core_pipeline::{prepass::DepthPrepass, tonemapping::Tonemapping},
    prelude::*,
    render::render_resource::TextureFormat,
};

#[derive(Resource, Default, Clone, Copy, PartialEq)]
pub struct RenderInventoryPortraitSelection {
    pub active: bool,
    pub rotation: f32,
}
#[derive(Resource, Default)]
pub struct RenderInventoryPortraitImage {
    pub image: Handle<Image>,
    pub ready: bool,
}
#[derive(Component)]
struct InventoryScene;
#[derive(Component)]
struct InventoryAvatar;
type Appearance = (
    terra_core::PlayerGender,
    terra_core::CharacterAppearance,
    crate::armor::AvatarEquipment,
);
#[derive(Resource, Default)]
struct SceneState {
    appearance: Option<Appearance>,
    frames: u8,
}
const LAYER: usize = 28;

pub(super) fn install(app: &mut App) {
    app.init_resource::<RenderInventoryPortraitSelection>()
        .init_resource::<RenderInventoryPortraitImage>()
        .init_resource::<SceneState>()
        .add_systems(Update, update);
}

#[allow(clippy::too_many_arguments)]
fn update(
    mut commands: Commands,
    selection: Res<RenderInventoryPortraitSelection>,
    profile: Option<Res<terra_core::PlayerProfile>>,
    session: Option<Res<terra_network::client::ClientSession>>,
    mut image: ResMut<RenderInventoryPortraitImage>,
    mut state: ResMut<SceneState>,
    mut assets: ResMut<AvatarAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    scene: Query<Entity, With<InventoryScene>>,
    mut avatars: Query<&mut Transform, With<InventoryAvatar>>,
) {
    if !selection.active {
        if state.appearance.take().is_some() {
            for entity in &scene {
                commands.entity(entity).try_despawn();
            }
        }
        state.frames = 0;
        image.ready = false;
        return;
    }
    let gender = profile
        .as_ref()
        .map_or_else(Default::default, |profile| profile.gender);
    let appearance = profile
        .as_ref()
        .map_or_else(Default::default, |profile| profile.appearance);
    let equipment = crate::armor::AvatarEquipment::from(
        session
            .as_ref()
            .map_or_else(Default::default, |session| session.stats.equipment),
    );
    let selected = (gender, appearance, equipment);
    let rotation = if selection.rotation.is_finite() {
        selection.rotation
    } else {
        0.0
    };
    if state.appearance == Some(selected) {
        for mut transform in &mut avatars {
            transform.rotation = Quat::from_rotation_y(rotation);
        }
        state.frames = state.frames.saturating_add(1);
        image.ready = state.frames >= 2;
        return;
    }
    for entity in &scene {
        commands.entity(entity).try_despawn();
    }
    state.appearance = Some(selected);
    state.frames = 0;
    image.ready = false;
    if image.image == Handle::default() {
        image.image = images.add(Image::new_target_texture(
            640,
            896,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    }
    assets.prepare(&mut meshes, gender, appearance);
    let root = commands
        .spawn((
            InventoryScene,
            InventoryAvatar,
            equipment,
            AvatarGender(gender),
            Transform::from_rotation(Quat::from_rotation_y(rotation)),
            Visibility::Visible,
        ))
        .id();
    spawn_parts(
        &mut commands,
        &assets,
        root,
        RenderLayers::layer(LAYER),
        gender,
        appearance,
    );
    commands.spawn((
        InventoryScene,
        Camera3d::default(),
        Camera {
            order: -14,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        RenderTarget::Image(image.image.clone().into()),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                viewport_height: 2.18,
            },
            near: 0.01,
            far: 30.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 0.02, -5.0).looking_at(Vec3::new(0.0, 0.02, 0.0), Vec3::Y),
        RenderLayers::layer(LAYER),
        AmbientLight {
            brightness: 900.0,
            color: Color::srgb(0.95, 0.98, 1.0),
            ..default()
        },
        Exposure { ev100: 13.7 },
        (Hdr, Tonemapping::AcesFitted, Msaa::Off, DepthPrepass),
    ));
    for (position, illuminance, color) in [
        (
            Vec3::new(-3.0, 6.0, -5.0),
            100_000.0,
            Color::srgb(1.0, 0.95, 0.85),
        ),
        (
            Vec3::new(4.0, -2.0, -4.0),
            28_000.0,
            Color::srgb(0.90, 0.95, 1.0),
        ),
    ] {
        commands.spawn((
            InventoryScene,
            DirectionalLight {
                illuminance,
                color,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(position).looking_at(Vec3::ZERO, Vec3::Y),
            RenderLayers::layer(LAYER),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotation_reuses_the_avatar_and_target_and_closing_removes_the_live_scene() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<Image>>()
            .insert_resource(AvatarAssets {
                skin_eyelids: default(),
                parts: [vec![], vec![]],
                custom: default(),
                eyelids: None,
                material: Handle::default(),
            });
        install(&mut app);
        app.world_mut()
            .resource_mut::<RenderInventoryPortraitSelection>()
            .active = true;
        app.update();
        let root = app
            .world_mut()
            .query_filtered::<Entity, With<InventoryAvatar>>()
            .single(app.world())
            .unwrap();
        let target = app
            .world()
            .resource::<RenderInventoryPortraitImage>()
            .image
            .clone();
        app.world_mut()
            .resource_mut::<RenderInventoryPortraitSelection>()
            .rotation = 1.5;
        app.update();
        app.update();
        assert_eq!(
            app.world().get::<Transform>(root).unwrap().rotation,
            Quat::from_rotation_y(1.5)
        );
        assert!(app.world().resource::<RenderInventoryPortraitImage>().ready);
        assert_eq!(
            app.world().resource::<RenderInventoryPortraitImage>().image,
            target
        );
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 1);
        app.world_mut()
            .resource_mut::<RenderInventoryPortraitSelection>()
            .active = false;
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&InventoryScene>()
                .iter(app.world())
                .count(),
            0
        );
        assert!(!app.world().resource::<RenderInventoryPortraitImage>().ready);
    }
}
