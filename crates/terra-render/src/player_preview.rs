//! One-shot inventory portrait of the selected shared world avatar.
use std::time::{Duration, Instant};

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
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<PlayerPreviewImage>()
        .init_resource::<PortraitCapture>()
        .add_systems(Update, (prepare, capture).chain());
}

fn prepare(
    mut commands: Commands,
    assets: Res<AvatarAssets>,
    mut images: ResMut<Assets<Image>>,
    mut preview: ResMut<PlayerPreviewImage>,
    mut pending: ResMut<PortraitCapture>,
    profile: Option<Res<terra_core::PlayerProfile>>,
) {
    if pending.started.is_some() {
        return;
    }
    pending.started = Some(Instant::now());
    let mut target = Image::new_target_texture(320, 512, TextureFormat::Rgba8UnormSrgb, None);
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    preview.image = images.add(target.clone());
    preview.closed_image = images.add(target);
    let root = commands
        .spawn((PortraitScene, Transform::default(), Visibility::Visible))
        .id();
    let avatar = spawn_parts(
        &mut commands,
        &assets,
        root,
        RenderLayers::layer(LAYER),
        profile
            .as_ref()
            .map_or(terra_core::PlayerGender::default(), |p| p.gender),
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
        Transform::from_xyz(4.0, 2.0, -1.0).looking_at(Vec3::ZERO, Vec3::Y),
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
}
