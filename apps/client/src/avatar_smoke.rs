//! Renderer fixture, not a network or physics simulation: drives both real avatar paths.
use bevy::{
    app::AppExit,
    camera::{Exposure, Hdr},
    core_pipeline::tonemapping::Tonemapping,
    mesh::VertexAttributeValues,
    prelude::*,
    render::{
        gpu_readback::{Readback, ReadbackComplete},
        view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    },
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_network::{
    client::{RemoteTrack, ReplicatedPlayers},
    protocol::session::PlayerState,
};
use terra_player::{CameraPerspective, ControllerMode, PlayerBody, PlayerInputBlock, TerraCamera};

const REMOTE: terra_core::PlayerId = terra_core::PlayerId(777);
const CENTER: Vec3 = Vec3::new(0.0, 201.0, 0.0);

#[derive(Resource)]
struct AvatarSmoke {
    started: Instant,
    since: Instant,
    stage: usize,
    captured: usize,
    portrait_received: [bool; 2],
    portrait_hashes: [Option<u64>; 2],
    failed: bool,
    walking: bool,
    previous_pose: Vec<Quat>,
    blink_seen: bool,
}

pub(crate) fn install(app: &mut App) -> Result<(), terra_core::ConfigError> {
    if !std::env::args().any(|arg| arg == "--smoke-test-avatar") {
        return Ok(());
    }
    let path = terra_core::PlayerProfile::selected_path(&terra_core::menu_config_directory())?;
    app.insert_resource(terra_core::PlayerProfile::load_for_client(path)?);
    let now = Instant::now();
    app.insert_resource(AvatarSmoke {
        started: now,
        since: now,
        stage: 0,
        captured: 0,
        portrait_received: [false; 2],
        portrait_hashes: [None; 2],
        failed: false,
        walking: false,
        previous_pose: Vec::new(),
        blink_seen: false,
    })
    .insert_resource(CameraPerspective::ThirdPerson)
    .insert_resource(ControllerMode::Walk)
    .insert_resource(PlayerInputBlock(true))
    .init_resource::<ReplicatedPlayers>()
    .add_systems(PostStartup, setup)
    .add_systems(Update, drive.before(terra_player::PlayerSet::Movement))
    .add_systems(PostUpdate, inspect);
    Ok(())
}

fn setup(world: &mut World) {
    world
        .resource_mut::<terra_core::GameConfig>()
        .client
        .view_distance = 2;
    world.resource_mut::<terra_world::streaming::ChunkFocus>().0 = CENTER.to_array().map(f64::from);
    world
        .resource_mut::<bevy::gizmos::config::GizmoConfigStore>()
        .config_mut::<bevy::gizmos::config::DefaultGizmoConfigGroup>()
        .0
        .enabled = false;
    for (mut camera, mut transform) in world
        .query_filtered::<(&mut Camera, &mut Transform), With<TerraCamera>>()
        .iter_mut(world)
    {
        // Bevy's atmosphere environment still uses this view. Keep it alive
        // below the studio camera instead of leaving its probe without a view.
        camera.order = -1;
        *transform = Transform::from_translation(CENTER + Vec3::Y * 0.72);
    }
    // A transform-only body exercises the renderer without pretending to test physics.
    world.spawn((
        PlayerBody,
        Transform::from_translation(CENTER - Vec3::X * 0.65),
    ));
    world.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.055, 0.065, 0.05)),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                viewport_height: 2.5,
            },
            near: 0.01,
            far: 25.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(CENTER + Vec3::new(2.8, 1.2, -7.0)).looking_at(CENTER, Vec3::Y),
        AmbientLight {
            brightness: 900.0,
            ..default()
        },
        Exposure { ev100: 13.7 },
        Hdr,
        Tonemapping::AcesFitted,
        Msaa::Off,
    ));
    let mesh = world
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(12.0, 0.05, 12.0));
    let material = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(Color::srgb(0.14, 0.14, 0.14));
    world.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(CENTER - Vec3::Y * 0.925),
    ));
    world.spawn((
        DirectionalLight {
            illuminance: 90_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-3.0, 6.0, -5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn drive(
    state: Res<AvatarSmoke>,
    profile: Res<terra_core::PlayerProfile>,
    mut players: ResMut<ReplicatedPlayers>,
    mut bodies: Query<&mut Transform, (With<PlayerBody>, Without<TerraCamera>)>,
    mut cameras: Query<&mut Transform, (With<TerraCamera>, Without<PlayerBody>)>,
) {
    let yaw = match state.stage {
        6 => -std::f32::consts::FRAC_PI_2,
        7 => std::f32::consts::PI,
        8 => std::f32::consts::FRAC_PI_2,
        _ => 0.0,
    };
    for mut camera in &mut cameras {
        camera.rotation = Quat::from_rotation_y(yaw);
    }
    let displacement = if state.walking {
        (state.since.elapsed().as_secs_f32() * 8.0).sin() * 0.35
    } else {
        0.0
    };
    for mut body in &mut bodies {
        body.translation = CENTER + Vec3::new(-0.65, 0.0, displacement);
    }
    let pose = PlayerState {
        gender: if profile.gender == terra_core::PlayerGender::Female {
            terra_core::PlayerGender::Male
        } else {
            terra_core::PlayerGender::Female
        },
        player: REMOTE,
        look: [yaw, 0.0],
        position: (CENTER + Vec3::new(0.65, 0.0, displacement)).to_array(),
        ..default()
    };
    players.players.insert(
        REMOTE,
        RemoteTrack {
            previous: pose,
            current: pose,
            received: Instant::now(),
            interval: 0.05,
        },
    );
}

fn inspect(world: &mut World) {
    if world.resource::<AvatarSmoke>().stage >= 10 {
        return;
    }
    if world
        .query::<(&Name, &Transform, &ChildOf)>()
        .iter(world)
        .any(|(name, transform, parent)| {
            name.as_str() == "Avatar eyelids"
                && transform.scale.y > 0.8
                && world
                    .get::<ChildOf>(parent.parent())
                    .is_some_and(|head_parent| {
                        world
                            .get::<terra_render::RemoteAvatar>(head_parent.parent())
                            .is_some()
                    })
        })
    {
        world.resource_mut::<AvatarSmoke>().blink_seen = true;
    }
    let preview = world.resource::<terra_render::PlayerPreviewImage>();
    let failed = world.resource::<AvatarSmoke>().failed || preview.error.is_some();
    if failed || world.resource::<AvatarSmoke>().started.elapsed() > Duration::from_secs(60) {
        error!(?preview.error, preview_ready = preview.ready,
            stage = world.resource::<AvatarSmoke>().stage,
            mode = ?world.resource::<ControllerMode>(),
            "Native avatar validation failed");
        world.resource_mut::<AvatarSmoke>().stage = 5;
        world.write_message(AppExit::error());
        return;
    }
    if !preview.ready {
        return;
    }
    let stage = world.resource::<AvatarSmoke>().stage;
    let captured = world.resource::<AvatarSmoke>().captured;
    let elapsed = world.resource::<AvatarSmoke>().since.elapsed();
    if stage == 0 {
        let roots: Vec<_> = world
            .query::<(Entity, Option<&terra_render::RemoteAvatar>, Option<&Name>)>()
            .iter(world)
            .filter(|(_, remote, name)| {
                remote.is_some() || name.is_some_and(|name| name.as_str() == "Local player avatar")
            })
            .map(|(entity, _, _)| entity)
            .collect();
        if roots.len() != 2 {
            return;
        }
        let mut handles = Vec::new();
        let mut lid_handles = Vec::new();
        let mut vertices = 0;
        let mut triangles = 0;
        let mut geometry_bytes = 0;
        for root in roots {
            let start_vertices = vertices;
            let start_bytes = geometry_bytes;
            let mut meshes = Vec::new();
            let mut lids = Vec::new();
            let mut avatar_triangles = 0;
            for child in descendants(world, root) {
                let Some(mesh) = world.get::<Mesh3d>(child) else {
                    continue;
                };
                let Some(geometry) = world.resource::<Assets<Mesh>>().get(&mesh.0) else {
                    return;
                };
                if world
                    .get::<Name>(child)
                    .is_some_and(|name| name.as_str() == "Avatar eyelids")
                {
                    if geometry.count_vertices() != 48 {
                        world.resource_mut::<AvatarSmoke>().failed = true;
                        return;
                    }
                    lids.push(mesh.0.id());
                    continue;
                }
                let Some(VertexAttributeValues::Float32x3(points)) =
                    geometry.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    world.resource_mut::<AvatarSmoke>().failed = true;
                    return;
                };
                if points.iter().flatten().any(|value| !value.is_finite())
                    || geometry
                        .indices()
                        .is_none_or(|indices| indices.iter().any(|index| index >= points.len()))
                {
                    world.resource_mut::<AvatarSmoke>().failed = true;
                    return;
                }
                vertices += points.len();
                let index_count = geometry.indices().map_or(0, |indices| indices.len());
                avatar_triangles += index_count / 3;
                geometry_bytes += points.len() * 48 + index_count * 4;
                meshes.push(mesh.0.id());
            }
            if !(20_000..=40_000).contains(&avatar_triangles) {
                error!(avatar_triangles, "Avatar triangle budget out of range");
                world.resource_mut::<AvatarSmoke>().failed = true;
                return;
            }
            let selected_gender = world.resource::<terra_core::PlayerProfile>().gender;
            let gender = if world.get::<terra_render::RemoteAvatar>(root).is_none() {
                selected_gender
            } else if selected_gender == terra_core::PlayerGender::Male {
                terra_core::PlayerGender::Female
            } else {
                terra_core::PlayerGender::Male
            };
            info!(
                ?gender,
                triangles = avatar_triangles,
                vertices = vertices - start_vertices,
                vertex_index_bytes = geometry_bytes - start_bytes,
                "Reference avatar measured shared geometry"
            );
            triangles += avatar_triangles;
            meshes.sort_unstable();
            handles.push(meshes);
            lid_handles.push(lids);
        }
        if handles[0].len() != 10
            || handles[1].len() != 10
            || handles[0] == handles[1]
            || lid_handles[0].len() != 1
            || lid_handles[0] != lid_handles[1]
        {
            world.resource_mut::<AvatarSmoke>().failed = true;
            return;
        }
        info!(
            vertices,
            triangles,
            geometry_bytes,
            shared_meshes = 21,
            "Native male and female avatars use ten shared articulated meshes each plus one eyelid mesh"
        );
        capture(world, "portrait", true);
        capture(world, "portrait-closed", true);
        world.resource_mut::<AvatarSmoke>().stage = 1;
    } else if stage == 1 && captured >= 2 && elapsed > Duration::from_secs(2) {
        capture(world, "idle", false);
        world.resource_mut::<AvatarSmoke>().stage = 2;
    } else if stage == 2 && captured >= 3 {
        let mut state = world.resource_mut::<AvatarSmoke>();
        state.walking = true;
        state.since = Instant::now();
        state.stage = 3;
    } else if stage == 3 && elapsed > Duration::from_millis(420) {
        let pose = remote_pose(world);
        world.resource_mut::<AvatarSmoke>().previous_pose = pose;
        capture(world, "walk-a", false);
        world.resource_mut::<AvatarSmoke>().stage = 4;
    } else if stage == 4
        && captured >= 4
        && elapsed > Duration::from_millis(850)
        && world.resource::<AvatarSmoke>().blink_seen
    {
        let pose = remote_pose(world);
        let moved = pose
            .iter()
            .zip(&world.resource::<AvatarSmoke>().previous_pose)
            .filter(|(a, b)| a.angle_between(**b) > 0.02)
            .count();
        if moved < 4 {
            world.resource_mut::<AvatarSmoke>().failed = true;
            return;
        }
        capture(world, "walk-b", false);
        world.resource_mut::<AvatarSmoke>().stage = 5;
        info!(
            moved_limbs = moved,
            "Native reference avatar walk poses differ on all four limbs"
        );
    } else if stage == 5 && captured >= 5 {
        let mut state = world.resource_mut::<AvatarSmoke>();
        state.walking = false;
        state.stage = 6;
        state.since = Instant::now();
    } else if (6..=8).contains(&stage)
        && captured >= stage - 1
        && elapsed > Duration::from_millis(750)
    {
        capture(
            world,
            match stage {
                6 => "left",
                7 => "back",
                _ => "right",
            },
            false,
        );
        let mut state = world.resource_mut::<AvatarSmoke>();
        state.stage += 1;
        state.since = Instant::now();
    }
}

fn descendants(world: &World, root: Entity) -> Vec<Entity> {
    let mut pending = vec![root];
    let mut result = Vec::new();
    while let Some(entity) = pending.pop() {
        if let Some(children) = world.get::<Children>(entity) {
            for child in children.iter() {
                result.push(child);
                pending.push(child);
            }
        }
    }
    result
}

fn remote_pose(world: &mut World) -> Vec<Quat> {
    let root = world
        .query_filtered::<Entity, With<terra_render::RemoteAvatar>>()
        .iter(world)
        .next();
    root.map_or_else(Vec::new, |root| {
        descendants(world, root)
            .into_iter()
            .filter_map(|child| world.get::<Transform>(child).map(|pose| pose.rotation))
            .collect()
    })
}

fn capture(world: &mut World, name: &str, portrait: bool) {
    let base = std::env::var_os("TERRA_SCREENSHOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/terra-avatar.png"));
    let stem = base
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("avatar");
    let path = base.with_file_name(format!("{stem}-{name}.png"));
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        error!(%error, path = %path.display(), "Cannot create avatar capture directory");
        world.resource_mut::<AvatarSmoke>().failed = true;
        world.write_message(AppExit::error());
        return;
    }
    let closed = name == "portrait-closed";
    let preview = world.resource::<terra_render::PlayerPreviewImage>();
    let handle = if closed {
        preview.closed_image.clone()
    } else {
        preview.image.clone()
    };
    let mut entity = world.spawn_empty();
    if portrait {
        // Screenshot::image needs an active camera and overwrites the target.
        // The portrait camera has finished; read its retained GPU texture directly.
        entity.insert(Readback::texture(handle.clone())).observe(
            move |event: On<ReadbackComplete>,
                  images: Res<Assets<Image>>,
                  mut commands: Commands,
                  mut state: ResMut<AvatarSmoke>| {
                if state.portrait_received[usize::from(closed)] {
                    return;
                }
                state.portrait_received[usize::from(closed)] = true;
                let Some(source) = images.get(&handle) else {
                    state.failed = true;
                    return;
                };
                let width = source.texture_descriptor.size.width as usize * 4;
                let height = source.texture_descriptor.size.height as usize;
                let stride = width.div_ceil(256) * 256;
                if event.data.len() < stride * height {
                    state.failed = true;
                    return;
                }
                let mut image = source.clone();
                image.data = Some(
                    event
                        .data
                        .chunks_exact(stride)
                        .take(height)
                        .flat_map(|row| row[..width].iter().copied())
                        .collect(),
                );
                state.portrait_hashes[usize::from(closed)] = image.data.as_ref().map(|bytes| {
                    bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
                        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
                    })
                });
                if state.portrait_hashes[0].is_some()
                    && state.portrait_hashes[0] == state.portrait_hashes[1]
                {
                    state.failed = true;
                    error!("Open and closed avatar portraits must differ in real GPU pixels");
                    commands.entity(event.entity).try_despawn();
                    return;
                }
                commands.trigger(ScreenshotCaptured {
                    entity: event.entity,
                    image,
                });
                commands.entity(event.entity).try_despawn();
            },
        );
    } else {
        entity.insert(Screenshot::primary_window());
    }
    entity.observe(save_to_disk(path)).observe(
        move |capture: On<ScreenshotCaptured>,
              mut state: ResMut<AvatarSmoke>,
              mut exit: MessageWriter<AppExit>| {
            let (pixels, clipped) = match avatar_pixels(&capture.image, portrait) {
                Ok(result) => result,
                Err(error) => {
                    state.failed = true;
                    error!(%error, "Unsupported avatar capture pixel format");
                    exit.write(AppExit::error());
                    return;
                }
            };
            if pixels < 500 || clipped {
                state.failed = true;
                error!(
                    pixels,
                    clipped,
                    "Avatar capture is blank, cropped or lacks the warm reference avatar geometry"
                );
                exit.write(AppExit::error());
            } else {
                state.captured += 1;
                if state.captured == 8 {
                    state.stage = 10;
                    info!(
                        captures = state.captured,
                        blink_seen = state.blink_seen,
                        "Native avatar smoke completed"
                    );
                    exit.write(AppExit::Success);
                }
            }
        },
    );
}

fn avatar_pixels(image: &Image, portrait: bool) -> Result<(usize, bool), String> {
    // Window screenshots may be BGRA; portrait textures are RGBA. Use Bevy's
    // format-aware conversion before evaluating either camera's actual pixels.
    let rgba = image
        .clone()
        .try_into_dynamic()
        .map_err(|e| e.to_string())?
        .into_rgba8();
    let width = rgba.width() as usize;
    let height = rgba.height() as usize;
    let pixels = rgba
        .as_raw()
        .chunks_exact(4)
        .filter(|pixel| pixel[0] > 45 && pixel[0] > pixel[2].saturating_add(15) && pixel[3] > 8)
        .count();
    let clipped = portrait
        && rgba
            .as_raw()
            .chunks_exact(4)
            .enumerate()
            .any(|(index, pixel)| {
                let x = index % width;
                let y = index / width;
                pixel[3] > 8 && (x <= 1 || y <= 1 || x + 2 >= width || y + 2 >= height)
            });
    Ok((pixels, clipped))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::render_resource::TextureFormat;

    #[test]
    fn window_bgra_and_portrait_rgba_validate_the_same_visible_avatar_pixels() {
        for (format, color) in [
            (TextureFormat::Rgba8UnormSrgb, [180, 120, 60, 255]),
            (TextureFormat::Bgra8UnormSrgb, [60, 120, 180, 255]),
        ] {
            let mut image = Image::new_target_texture(16, 16, format, None);
            let mut data = vec![0; 16 * 16 * 4];
            for y in 4..12 {
                for x in 4..12 {
                    let offset = (y * 16 + x) * 4;
                    data[offset..offset + 4].copy_from_slice(&color);
                }
            }
            image.data = Some(data);
            assert_eq!(avatar_pixels(&image, true).unwrap(), (64, false));
            image.data.as_mut().unwrap()[..4].copy_from_slice(&color);
            assert_eq!(avatar_pixels(&image, true).unwrap(), (65, true));
            image.data = Some(vec![0; 16 * 16 * 4]);
            assert_eq!(avatar_pixels(&image, true).unwrap(), (0, false));
        }
    }
}
