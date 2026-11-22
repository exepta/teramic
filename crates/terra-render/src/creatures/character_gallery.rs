//! Opt-in native reference capture. It never edits worlds, profiles or gameplay entities.
use super::{Models, model};
use crate::players::{AvatarAssets, spawn_parts};
use bevy::{
    camera::{Exposure, Hdr, RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::{
        render_resource::{TextureFormat, TextureUsages},
        view::screenshot::{Screenshot, save_to_disk},
    },
};

const LAYER: usize = 26;
#[derive(Component)]
struct GalleryHand;

#[derive(Resource)]
struct Gallery {
    image: Handle<Image>,
    frames: u16,
    path: std::path::PathBuf,
}

pub(super) fn install(app: &mut App) {
    if !matches!(
        std::env::var("TERRA_CHARACTER_GALLERY").as_deref(),
        Ok("players" | "npcs")
    ) {
        return;
    }
    app.add_systems(PostStartup, setup)
        .add_systems(Update, capture)
        .add_systems(PostUpdate, layer_hands);
}
fn setup(
    mut commands: Commands,
    models: Res<Models>,
    mut avatars: ResMut<AvatarAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
) {
    let kind = std::env::var("TERRA_CHARACTER_GALLERY").unwrap_or_default();
    let directory = std::env::var_os("TERRA_CHARACTER_GALLERY_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("target/character-gallery"));
    if let Err(error) = std::fs::create_dir_all(&directory) {
        tracing::error!(%error,"Cannot create character gallery directory");
        return;
    }
    let mut target = Image::new_target_texture(4096, 2304, TextureFormat::Rgba8UnormSrgb, None);
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let image = images.add(target);
    let viewport_height = if kind == "players" {
        let columns =
            terra_core::CharacterAppearance::hairstyle_count(terra_core::PlayerGender::Female).max(
                terra_core::CharacterAppearance::hairstyle_count(terra_core::PlayerGender::Male),
            );
        ((f32::from(columns) * 0.98 + 1.2) / (4096.0 / 2304.0)).max(5.8)
    } else {
        5.8
    };
    if kind == "players" {
        for (row, gender) in [
            terra_core::PlayerGender::Male,
            terra_core::PlayerGender::Female,
        ]
        .into_iter()
        .enumerate()
        {
            let count = terra_core::CharacterAppearance::hairstyle_count(gender);
            for hairstyle in 0..count {
                let appearance = terra_core::CharacterAppearance {
                    hairstyle,
                    ..default()
                };
                avatars.prepare(&mut meshes, gender, appearance);
                let x = -(f32::from(hairstyle) - (f32::from(count) - 1.0) * 0.5) * 0.98;
                let root = commands
                    .spawn((
                        Transform::from_xyz(x, if row == 0 { 3.8 } else { 1.2 }, 0.0)
                            .with_rotation(Quat::from_rotation_y(-0.08)),
                        Visibility::Visible,
                    ))
                    .id();
                spawn_parts(
                    &mut commands,
                    &avatars,
                    root,
                    RenderLayers::layer(LAYER),
                    gender,
                    appearance,
                );
            }
        }
    } else {
        let roles = [
            13, 9, 14, 15, 1, 16, 5, 12, 10, 11, 8, 17, 18, 19, 20, 21, 22,
        ];
        for (index, role) in roles.into_iter().enumerate() {
            let row = index / 9;
            let column = index % 9;
            let count = if row == 0 { 9 } else { 8 };
            let root = commands
                .spawn((
                    Transform::from_xyz(
                        -(column as f32 - (count as f32 - 1.0) * 0.5) * 1.10,
                        if row == 0 { 3.0 } else { 0.25 },
                        0.0,
                    )
                    .with_rotation(Quat::from_rotation_y(-0.30)),
                    Visibility::Visible,
                ))
                .id();
            if let Some(parts) = models
                .parts
                .get(&(terra_core::creatures::CreatureKind::Villager, role))
            {
                for (part, offset, mesh) in parts {
                    let rotation = match part {
                        model::Part::VillagerArm(side)
                        | model::Part::VillagerSleeve(side)
                        | model::Part::MilitaryWeapon(side) => Quat::from_rotation_z(*side * 0.06),
                        _ => Quat::IDENTITY,
                    };
                    let mut child = commands.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(models.material.clone()),
                        Transform::from_translation(*offset).with_rotation(rotation),
                        RenderLayers::layer(LAYER),
                        bevy::light::NotShadowReceiver,
                        ChildOf(root),
                    ));
                    if matches!(part,model::Part::VillagerArm(side) if *side>0.0)
                        && let Some(hand) = super::VillagerHand::new(role)
                    {
                        child.insert((hand, GalleryHand));
                    }
                }
            }
        }
    }
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -14,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.91, 0.86, 0.78)),
            ..default()
        },
        RenderTarget::Image(image.clone().into()),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::FixedVertical { viewport_height },
            near: 0.01,
            far: 30.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 2.8, -12.0).looking_at(Vec3::new(0.0, 2.8, 0.0), Vec3::Y),
        RenderLayers::layer(LAYER),
        AmbientLight {
            brightness: 1000.0,
            color: Color::srgb(1.0, 0.96, 0.90),
            ..default()
        },
        Exposure { ev100: 13.7 },
        (Hdr, Tonemapping::AcesFitted, Msaa::Off),
    ));
    for (at, power, color) in [
        (
            Vec3::new(-4.0, 7.0, -5.0),
            100_000.0,
            Color::srgb(1.0, 0.94, 0.83),
        ),
        (
            Vec3::new(4.0, 2.0, -1.0),
            25_000.0,
            Color::srgb(0.90, 0.95, 1.0),
        ),
    ] {
        commands.spawn((
            DirectionalLight {
                illuminance: power,
                color,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(at).looking_at(Vec3::ZERO, Vec3::Y),
            RenderLayers::layer(LAYER),
        ));
    }
    commands.insert_resource(Gallery {
        image,
        frames: 0,
        path: directory.join(format!("{kind}.png")),
    });
}
fn capture(
    mut commands: Commands,
    gallery: Option<ResMut<Gallery>>,
    blocks: Res<crate::blocks::BlockAssets>,
) {
    let Some(mut gallery) = gallery else {
        return;
    };
    if blocks.templates.is_none() || gallery.frames >= 120 {
        return;
    }
    gallery.frames += 1;
    if gallery.frames == 120 {
        commands
            .spawn(Screenshot::image(gallery.image.clone()))
            .observe(save_to_disk(gallery.path.clone()));
        tracing::info!(path=%gallery.path.display(),"Character reference gallery capture requested");
    }
}

fn layer_hands(
    mut commands: Commands,
    hands: Query<(), With<GalleryHand>>,
    held: Query<(Entity, &super::VillagerHeldItem)>,
    children: Query<&Children>,
    mut layers: Query<&mut RenderLayers>,
) {
    for (root, item) in &held {
        if hands.get(item.arm).is_err() {
            continue;
        }
        for child in children.iter_descendants(root) {
            if let Ok(mut layer) = layers.get_mut(child) {
                *layer = RenderLayers::layer(LAYER);
                commands
                    .entity(child)
                    .insert(bevy::light::NotShadowReceiver);
            }
        }
    }
}
