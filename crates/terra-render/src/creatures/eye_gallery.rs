//! Opt-in four-frame inspection of real cached heads and the production blink system.
use super::{CreatureAnimationSet, Limb, Models, Motion, model::Part};
use bevy::{
    camera::{Exposure, Hdr, RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::{
        render_resource::{TextureFormat, TextureUsages},
        view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    },
};
use std::{path::PathBuf, time::Instant};
use terra_core::creatures::{CreatureBehavior as Behavior, CreatureKind as Kind};

mod pixels;

const LAYER: usize = 27;
const PHASES: [(f32, &str); 4] = [
    (1.0, "open"),
    (6.7275, "half-blink"),
    (6.77, "closed"),
    (6.96, "after-blink"),
];
#[derive(Component)]
struct GalleryHead;
#[derive(Resource)]
struct Gallery {
    image: Handle<Image>,
    folder: PathBuf,
    phase: usize,
    frames: u16,
    pending: bool,
    started: Instant,
}

pub(super) fn install(app: &mut App) {
    if std::env::var_os("TERRA_EYE_INSPECT").is_none()
        || !std::env::args().any(|a| a == "--smoke-test-creatures")
    {
        return;
    }
    app.add_systems(PostStartup, setup)
        .add_systems(Update, set_phase.before(CreatureAnimationSet))
        .add_systems(PostUpdate, capture);
}

fn setup(mut commands: Commands, models: Res<Models>, mut images: ResMut<Assets<Image>>) {
    let folder = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| "target/eye-gallery".into());
    if let Err(error) = std::fs::create_dir_all(&folder) {
        tracing::error!(%error, "Cannot create eye gallery directory");
        commands.write_message(AppExit::error());
        return;
    }
    let mut target = Image::new_target_texture(1920, 1080, TextureFormat::Rgba8UnormSrgb, None);
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let image = images.add(target);
    for (index, (kind, yaw)) in [
        (Kind::Sheep, 0.85),
        (Kind::Cow, -0.85),
        (Kind::Pig, 0.85),
        (Kind::Bear, 0.0),
        (Kind::Horse, std::f32::consts::FRAC_PI_2),
        (Kind::Horse, -std::f32::consts::FRAC_PI_2),
    ]
    .into_iter()
    .enumerate()
    {
        let Some(parts) = models.parts.get(&(kind, 0)) else {
            continue;
        };
        let Some((_, pivot, _)) = parts.iter().find(|(part, _, _)| matches!(part, Part::Head))
        else {
            continue;
        };
        let slot = Vec3::new(
            (index % 3) as f32 * 1.2 - 1.2,
            if index < 3 { 0.55 } else { -0.55 },
            0.0,
        );
        let rotation = Quat::from_rotation_y(yaw);
        let root = commands
            .spawn((
                GalleryHead,
                Motion {
                    kind,
                    behavior: Behavior::Rest,
                    phase: 0.0,
                    clock: 1.0,
                    speed: 0.0,
                    sleep: 0.0,
                    attack: 0.0,
                    worm_progress: 0.0,
                    worm_spit: None,
                    silk_length: 0.0,
                    youth: 0.0,
                    jump: Default::default(),
                },
                Transform::from_translation(slot - rotation * *pivot).with_rotation(rotation),
                Visibility::Inherited,
            ))
            .id();
        for (part, offset, mesh) in parts {
            if !matches!(part, Part::Head | Part::Eyelids) {
                continue;
            }
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(models.material.clone()),
                Limb(*part, *offset),
                Transform::from_translation(*offset),
                Visibility::Inherited,
                RenderLayers::layer(LAYER),
                ChildOf(root),
            ));
        }
    }
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -15,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.19, 0.23, 0.27)),
            ..default()
        },
        RenderTarget::Image(image.clone().into()),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                viewport_height: 2.3,
            },
            near: 0.01,
            far: 20.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 0.0, -6.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(LAYER),
        AmbientLight {
            brightness: 1000.0,
            color: Color::srgb(1.0, 0.96, 0.90),
            ..default()
        },
        Exposure { ev100: 13.7 },
        (Hdr, Tonemapping::AcesFitted, Msaa::Off),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 90_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-3.0, 5.0, -6.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(LAYER),
    ));
    commands.insert_resource(Gallery {
        image,
        folder,
        phase: 0,
        frames: 0,
        pending: false,
        started: Instant::now(),
    });
}

fn set_phase(gallery: Option<Res<Gallery>>, mut heads: Query<&mut Motion, With<GalleryHead>>) {
    let Some(gallery) = gallery else {
        return;
    };
    let Some(&(clock, _)) = PHASES.get(gallery.phase) else {
        return;
    };
    for mut motion in &mut heads {
        motion.clock = clock;
    }
}

fn capture(
    mut commands: Commands,
    gallery: Option<ResMut<Gallery>>,
    heads: Query<(), With<GalleryHead>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut gallery) = gallery else {
        return;
    };
    if gallery.phase >= PHASES.len() {
        tracing::info!(
            "Eye gallery passed: open, half-blink, closed, after-blink; five blinking species, both horse sides; production animation and cached meshes"
        );
        exit.write(AppExit::Success);
        return;
    }
    if gallery.started.elapsed().as_secs() > 90 || heads.iter().count() != 6 {
        tracing::error!("Eye gallery failed: timeout or missing model heads");
        exit.write(AppExit::error());
        return;
    }
    gallery.frames += 1;
    if gallery.pending || gallery.frames < 45 || gallery.started.elapsed().as_secs() < 4 {
        return;
    }
    gallery.pending = true;
    let phase = gallery.phase;
    commands
        .spawn(Screenshot::image(gallery.image.clone()))
        .observe(save_to_disk(
            gallery
                .folder
                .join(format!("eyes-{phase}-{}.png", PHASES[phase].1)),
        ))
        .observe(
            |event: On<ScreenshotCaptured>,
             mut gallery: ResMut<Gallery>,
             mut exit: MessageWriter<AppExit>| {
                let size = event.image.texture_descriptor.size;
                let nonblank = event.image.data.as_ref().is_some_and(|data| {
                    pixels::has_six_heads(data, size.width as usize, size.height as usize)
                });
                if !nonblank {
                    // Render pipelines compile asynchronously. Keep the same
                    // open-eye pose until every head is actually visible.
                    if gallery.phase == 0 {
                        tracing::warn!("Eye gallery waiting for all six rendered heads");
                        gallery.pending = false;
                        gallery.frames = 0;
                        return;
                    }
                    tracing::error!("Eye gallery screenshot is blank");
                    exit.write(AppExit::error());
                    return;
                }
                gallery.phase += 1;
                gallery.pending = false;
                gallery.frames = 0;
            },
        );
}
