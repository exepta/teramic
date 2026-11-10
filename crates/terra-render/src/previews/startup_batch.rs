//! Bounded parallel renders retain the complete startup icon catalog before the menu.
use super::*;
const LANES: usize = 16;
const SPACING: f32 = 64.0;
#[derive(Default)]
pub(super) struct State {
    pub jobs: Vec<Job>,
    light: Option<Entity>,
}
pub(super) struct Job {
    pub item: ItemKind,
    slot: usize,
    root: Entity,
    camera: Entity,
    image: Handle<Image>,
    frames: u16,
    started: Instant,
    capture: Option<(Entity, Instant)>,
    received: Option<bool>,
    retry_at: Option<Instant>,
}
fn origin(slot: usize) -> Vec3 {
    Vec3::X * (slot as f32 * SPACING)
}
pub(super) fn prepare(
    commands: &mut Commands,
    assets: &mut PreviewAssets,
    templates: &TemplateSet,
    options: &RenderOptions,
    queue: &mut PreviewQueue,
    previews: &mut RenderBlockPreviews,
) {
    if queue.preload.light.is_none() && !queue.items.is_empty() {
        queue.preload.light = Some(
            commands
                .spawn((
                    PreviewScene,
                    DirectionalLight {
                        color: Color::srgb(1., 0.96, 0.88),
                        illuminance: 100_000.,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(4., 8., 6.).looking_at(Vec3::ZERO, Vec3::Y),
                    RenderLayers::layer(PREVIEW_LAYER),
                ))
                .id(),
        );
    }
    let budget = Instant::now();
    for _ in 0..8 {
        if queue.preload.jobs.len() >= LANES || budget.elapsed() > Duration::from_millis(3) {
            break;
        }
        let Some(item) = queue.items.pop_front() else {
            break;
        };
        let batches = match compose_item(item, templates, options) {
            Ok(batches) => batches,
            Err(error) => {
                if queue.catalog_remaining.remove(&item) {
                    queue.catalog_failed += 1;
                }
                queue.failed.insert(item);
                previews.error = Some(error.clone());
                tracing::error!(%error,?item,"Cannot compose startup inventory preview");
                continue;
            }
        };
        let slot = (0..LANES)
            .find(|slot| !queue.preload.jobs.iter().any(|job| job.slot == *slot))
            .expect("bounded free preview lane");
        let offset = origin(slot);
        let root = spawn_item(
            commands,
            assets,
            templates,
            item,
            batches,
            offset,
            Visibility::Visible,
        );
        let mut image = Image::new_target_texture(
            PREVIEW_SIZE,
            PREVIEW_SIZE,
            TextureFormat::Rgba8UnormSrgb,
            None,
        );
        image.texture_descriptor.usage |= bevy::render::render_resource::TextureUsages::COPY_SRC;
        let image = assets.images.add(image);
        let camera = commands
            .spawn((
                PreviewScene,
                Camera3d::default(),
                Camera {
                    order: -10,
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                    ..default()
                },
                RenderTarget::Image(image.clone().into()),
                Projection::Orthographic(OrthographicProjection {
                    scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                        viewport_height: 1.95,
                    },
                    near: 0.01,
                    far: 30.,
                    ..OrthographicProjection::default_3d()
                }),
                Transform::from_translation(offset + Vec3::new(2.9, 2.5, 3.5))
                    .looking_at(offset + Vec3::splat(0.5), Vec3::Y),
                RenderLayers::layer(PREVIEW_LAYER),
                AmbientLight {
                    brightness: 700.,
                    color: Color::srgb(0.93, 0.96, 1.),
                    ..default()
                },
                Exposure { ev100: 13.7 },
                (Hdr, Tonemapping::AcesFitted, Msaa::Off, DepthPrepass),
                bevy::pbr::ScreenSpaceTransmission {
                    steps: 1,
                    ..default()
                },
            ))
            .id();
        queue.preload.jobs.push(Job {
            item,
            slot,
            root,
            camera,
            image,
            frames: 0,
            started: Instant::now(),
            capture: None,
            received: None,
            retry_at: None,
        });
    }
    previews.pending = queue.pending();
}
pub(super) fn advance(
    commands: &mut Commands,
    queue: &mut PreviewQueue,
    blocks: &mut RenderBlockPreviews,
    props: &mut RenderPropPreviews,
    items: &mut RenderItemPreviews,
) {
    let now = Instant::now();
    let mut pending = Vec::with_capacity(LANES);
    for mut job in std::mem::take(&mut queue.preload.jobs) {
        let result = job.received.take();
        if result.is_some() {
            job.capture = None;
        }
        let expired = job.capture.is_none() && now.duration_since(job.started) > PREVIEW_GRACE;
        let timed_out = job
            .capture
            .is_some_and(|(_, since)| now.duration_since(since) > Duration::from_secs(30));
        if result == Some(true) || expired || timed_out {
            let success = result == Some(true);
            if queue.catalog_remaining.remove(&job.item) && !success {
                queue.catalog_failed += 1;
            }
            if success {
                queue.captured_once = true;
                match job.item {
                    ItemKind::Block(id) => {
                        blocks.images.insert(id, job.image);
                    }
                    ItemKind::Prop(kind) => {
                        props.images.insert(kind, job.image);
                    }
                    item => items.publish(item, job.image),
                }
            } else {
                queue.failed.insert(job.item);
                let error = format!(
                    "{} inventory preview {}",
                    job.item.identifier(),
                    if timed_out {
                        "GPU readback timed out"
                    } else {
                        "remained blank after shader readiness grace"
                    }
                );
                tracing::error!(%error);
                blocks.error = Some(error);
            }
            commands.entity(job.root).try_despawn();
            commands.entity(job.camera).try_despawn();
            continue;
        }
        if result == Some(false) {
            job.retry_at = Some(now + RETRY_INTERVAL);
        }
        job.frames = job.frames.saturating_add(1);
        if job.capture.is_none()
            && job.frames >= if queue.captured_once { 2 } else { 8 }
            && job.retry_at.is_none_or(|at| now >= at)
        {
            let item = job.item;
            let capture = commands
                .spawn(Screenshot::image(job.image.clone()))
                .observe(
                    move |capture: On<ScreenshotCaptured>, mut queue: ResMut<PreviewQueue>| {
                        if let Some(job) = queue.preload.jobs.iter_mut().find(|job| {
                            job.item == item
                                && job
                                    .capture
                                    .is_some_and(|(entity, _)| entity == capture.entity)
                        }) {
                            job.received = Some(preview_has_pixels(&capture.image));
                        }
                    },
                )
                .id();
            job.capture = Some((capture, now));
            job.frames = 0;
            job.retry_at = None;
        }
        pending.push(job);
    }
    queue.preload.jobs = pending;
    blocks.pending = queue.pending();
    if queue.initialized && queue.pending() == 0 {
        if let Some(light) = queue.preload.light.take() {
            commands.entity(light).try_despawn();
        }
        queue.parallel = false;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concurrent_preview_lanes_cannot_enter_each_others_camera_far_plane() {
        for first in 0..LANES {
            for second in 0..LANES {
                if first != second {
                    assert!(origin(first).distance(origin(second)) > 30. * 2.);
                }
            }
        }
    }
}
