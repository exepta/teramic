//! One-shot inventory portrait of the selected shared world avatar.
use std::time::{Duration, Instant};
mod editor;
mod inventory;
pub use editor::{RenderCharacterPreviewImage, RenderCharacterPreviewSelection};
pub use inventory::{RenderInventoryPortraitImage, RenderInventoryPortraitSelection};

use bevy::{
    camera::{Exposure, Hdr, RenderTarget, visibility::RenderLayers},
    core_pipeline::{prepass::DepthPrepass, tonemapping::Tonemapping},
    prelude::*,
    render::{
        render_resource::{TextureFormat, TextureUsages},
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
};

use crate::players::{AvatarAssets, AvatarEyelidPose, spawn_parts};

const LAYER: usize = 30;
const GRACE: Duration = Duration::from_secs(12);
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Resource, Default)]
pub struct PlayerPreviewImage {
    pub image: Handle<Image>,
    pub closed_image: Handle<Image>,
    pub ready: bool,
    pub error: Option<String>,
}

#[derive(Component)]
struct PortraitScene;

#[derive(Resource, Default)]
struct PortraitCapture {
    started: Option<Instant>,
    frames: u16,
    capture: Option<(Entity, Instant)>,
    retry_at: Option<Instant>,
    received: Option<bool>,
    finished: bool,
    closed: bool,
    camera: Option<Entity>,
    eyelids: Option<Entity>,
    equipment: crate::armor::AvatarEquipment,
    gender: terra_core::PlayerGender,
    appearance: terra_core::CharacterAppearance,
}

pub(crate) fn install(app: &mut App) {
    editor::install(app);
    inventory::install(app);
    app.init_resource::<PlayerPreviewImage>()
        .init_resource::<PortraitCapture>()
        .add_systems(Update, (prepare, capture).chain());
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    mut commands: Commands,
    mut assets: ResMut<AvatarAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    scene: Query<Entity, With<PortraitScene>>,
    mut images: ResMut<Assets<Image>>,
    mut preview: ResMut<PlayerPreviewImage>,
    mut pending: ResMut<PortraitCapture>,
    profile: Option<Res<terra_core::PlayerProfile>>,
    session: Option<Res<terra_network::client::ClientSession>>,
) {
    let equipment = crate::armor::AvatarEquipment::from(
        session
            .as_ref()
            .map_or(terra_core::EquipmentSlots::default(), |session| {
                session.stats.equipment
            }),
    );
    let gender = profile
        .as_ref()
        .map_or(terra_core::PlayerGender::default(), |profile| {
            profile.gender
        });
    let appearance = profile
        .as_ref()
        .map_or(terra_core::CharacterAppearance::default(), |profile| {
            profile.appearance
        });
    if pending.started.is_some()
        && (pending.equipment != equipment
            || pending.gender != gender
            || pending.appearance != appearance)
    {
        for entity in &scene {
            commands.entity(entity).try_despawn();
        }
        if let Some((entity, _)) = pending.capture.take() {
            commands.entity(entity).try_despawn();
        }
        *pending = PortraitCapture::default();
        preview.ready = false;
        preview.error = None;
    }
    if pending.started.is_some() {
        return;
    }
    pending.equipment = equipment;
    pending.gender = gender;
    pending.appearance = appearance;
    pending.started = Some(Instant::now());
    if preview.image == Handle::default() || preview.closed_image == Handle::default() {
        let mut target = Image::new_target_texture(320, 512, TextureFormat::Rgba8UnormSrgb, None);
        target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        preview.image = images.add(target.clone());
        preview.closed_image = images.add(target);
    }
    let root = commands
        .spawn((
            PortraitScene,
            equipment,
            crate::players::AvatarGender(gender),
            Transform::default(),
            Visibility::Visible,
        ))
        .id();
    assets.prepare(&mut meshes, gender, appearance);
    let avatar = spawn_parts(
        &mut commands,
        &assets,
        root,
        RenderLayers::layer(LAYER),
        gender,
        appearance,
    );
    pending.eyelids = avatar.eyelids;
    if let Some(eyelids) = avatar.eyelids {
        commands.entity(eyelids).insert(AvatarEyelidPose(0.0));
    }
    pending.camera = Some(
        commands
            .spawn((
                PortraitScene,
                Camera3d::default(),
                Camera {
                    order: -11,
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                    ..default()
                },
                RenderTarget::Image(preview.image.clone().into()),
                Projection::Orthographic(OrthographicProjection {
                    scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                        viewport_height: 2.18,
                    },
                    near: 0.01,
                    far: 30.0,
                    ..OrthographicProjection::default_3d()
                }),
                Transform::from_xyz(2.2, 1.1, -5.0).looking_at(Vec3::new(0.0, 0.02, 0.0), Vec3::Y),
                RenderLayers::layer(LAYER),
                AmbientLight {
                    brightness: 900.0,
                    color: Color::srgb(0.95, 0.98, 1.0),
                    ..default()
                },
                Exposure { ev100: 13.7 },
                (Hdr, Tonemapping::AcesFitted, Msaa::Off, DepthPrepass),
            ))
            .id(),
    );
    commands.spawn((
        PortraitScene,
        DirectionalLight {
            illuminance: 100_000.0,
            color: Color::srgb(1.0, 0.95, 0.85),
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-3.0, 6.0, -5.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(LAYER),
    ));
    commands.spawn((
        PortraitScene,
        DirectionalLight {
            illuminance: 28_000.0,
            color: Color::srgb(0.90, 0.95, 1.0),
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(4.0, -2.0, -4.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(LAYER),
    ));
}

fn capture(
    mut commands: Commands,
    mut pending: ResMut<PortraitCapture>,
    mut preview: ResMut<PlayerPreviewImage>,
    scene: Query<Entity, With<PortraitScene>>,
) {
    let Some(started) = pending.started else {
        return;
    };
    if pending.finished {
        return;
    }
    let now = Instant::now();
    let rendered = pending.received.take();
    let timed_out = rendered.is_none()
        && pending
            .capture
            .is_some_and(|(_, since)| now.duration_since(since) >= CAPTURE_TIMEOUT);
    if rendered == Some(true) && !pending.closed {
        if let Some((entity, _)) = pending.capture.take() {
            commands.entity(entity).try_despawn();
        }
        pending.closed = true;
        pending.frames = 0;
        pending.retry_at = None;
        pending.started = Some(now);
        if let Some(eyelids) = pending.eyelids {
            commands.entity(eyelids).insert(AvatarEyelidPose(1.0));
        }
        if let Some(camera) = pending.camera {
            commands
                .entity(camera)
                .insert(RenderTarget::Image(preview.closed_image.clone().into()));
        }
        return;
    }
    if rendered == Some(true) || now.duration_since(started) >= GRACE || timed_out {
        if let Some((entity, _)) = pending.capture.take() {
            commands.entity(entity).try_despawn();
        }
        pending.finished = true;
        preview.ready = rendered == Some(true);
        if !preview.ready {
            let error = if timed_out {
                "Player preview GPU readback timed out"
            } else {
                "Player preview remained blank after shader readiness grace"
            };
            preview.error = Some(error.into());
            tracing::warn!(error);
        }
        for entity in &scene {
            commands.entity(entity).try_despawn();
        }
        return;
    }
    if rendered == Some(false) {
        pending.capture = None;
        pending.retry_at = Some(now + Duration::from_millis(200));
    }
    pending.frames = pending.frames.saturating_add(1);
    if pending.frames < 8
        || pending.capture.is_some()
        || pending.retry_at.is_some_and(|time| now < time)
    {
        return;
    }
    let entity = commands
        .spawn(Screenshot::image(if pending.closed {
            preview.closed_image.clone()
        } else {
            preview.image.clone()
        }))
        .observe(
            |capture: On<ScreenshotCaptured>, mut pending: ResMut<PortraitCapture>| {
                pending.received = Some(crate::previews::preview_has_pixels(&capture.image));
            },
        )
        .id();
    pending.capture = Some((entity, now));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> App {
        let mut app = App::new();
        app.init_resource::<PlayerPreviewImage>()
            .insert_resource(PortraitCapture {
                started: Some(Instant::now()),
                ..default()
            })
            .add_systems(Update, capture);
        app.world_mut().spawn(PortraitScene);
        app
    }

    #[test]
    fn portrait_is_published_only_after_real_nonblank_capture_and_scene_is_removed() {
        let mut app = fixture();
        app.world_mut().resource_mut::<PortraitCapture>().received = Some(false);
        app.update();
        assert!(!app.world().resource::<PlayerPreviewImage>().ready);
        app.world_mut().resource_mut::<PortraitCapture>().received = Some(true);
        app.update();
        assert!(!app.world().resource::<PlayerPreviewImage>().ready);
        assert!(app.world().resource::<PortraitCapture>().closed);
        app.world_mut().resource_mut::<PortraitCapture>().received = Some(true);
        app.update();
        assert!(app.world().resource::<PlayerPreviewImage>().ready);
        assert_eq!(
            app.world_mut()
                .query::<&PortraitScene>()
                .iter(app.world())
                .count(),
            0
        );
    }

    #[test]
    fn portrait_failure_cleans_up_without_publishing_empty_image() {
        let mut app = fixture();
        app.world_mut().resource_mut::<PortraitCapture>().started =
            Some(Instant::now() - GRACE - Duration::from_secs(1));
        app.update();
        let image = app.world().resource::<PlayerPreviewImage>();
        assert!(!image.ready);
        assert!(image.error.is_some());
        assert_eq!(
            app.world_mut()
                .query::<&PortraitScene>()
                .iter(app.world())
                .count(),
            0
        );
    }

    #[test]
    fn equipment_changes_recapture_portrait_without_allocating_new_targets() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<PlayerPreviewImage>()
            .init_resource::<PortraitCapture>()
            .init_resource::<terra_network::client::ClientSession>()
            .insert_resource(AvatarAssets {
                skin_eyelids: Default::default(),
                parts: [Vec::new(), Vec::new()],
                custom: default(),
                eyelids: None,
                material: Handle::default(),
            })
            .add_systems(Update, (prepare, capture).chain());
        app.update();
        let open = app.world().resource::<PlayerPreviewImage>().image.clone();
        let closed = app
            .world()
            .resource::<PlayerPreviewImage>()
            .closed_image
            .clone();
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 2);
        for material in terra_core::ArmorMaterial::ALL {
            for _ in 0..2 {
                app.world_mut().resource_mut::<PortraitCapture>().received = Some(true);
                app.update();
            }
            assert!(app.world().resource::<PlayerPreviewImage>().ready);
            let helmet = terra_core::ItemKind::Armor(terra_core::ArmorItem {
                material,
                piece: terra_core::ArmorPiece::Helmet,
            });
            app.world_mut()
                .resource_mut::<terra_network::client::ClientSession>()
                .stats
                .equipment
                .set(terra_core::EquipmentSlot::Helmet, Some(helmet));
            app.update();
            let preview = app.world().resource::<PlayerPreviewImage>();
            assert!(!preview.ready);
            assert_eq!(preview.image, open);
            assert_eq!(preview.closed_image, closed);
            assert_eq!(app.world().resource::<Assets<Image>>().len(), 2);
            assert_eq!(
                app.world().resource::<PortraitCapture>().equipment.0[0],
                Some(helmet)
            );
        }
    }
}
