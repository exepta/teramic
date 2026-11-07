//! Native acceptance test: first-upload and re-entry of a generated section.
use super::*;
use std::time::{Duration, Instant};

#[derive(Resource)]
struct Fixture {
    started: Instant,
    root: Option<Entity>,
    child: Option<Entity>,
    original: Transform,
    mode: usize,
    saw_zero: bool,
    saw_middle: bool,
    saw_draw: bool,
    finished: bool,
    fresh_complete: bool,
    fresh_pixels: Option<usize>,
    background: Option<BackgroundProbe>,
}

struct BackgroundProbe {
    root: Entity,
    child: Entity,
    turned: Instant,
    saw_visible: bool,
}

const MODES: [ChunkAnimation; 4] = [
    ChunkAnimation::FadeSlow,
    ChunkAnimation::Bounce,
    ChunkAnimation::Rise,
    ChunkAnimation::Slide,
];

fn terrain_material(world: &World, entity: Entity) -> bool {
    world
        .get::<MeshMaterial3d<StandardMaterial>>(entity)
        .is_some()
        || world
            .get::<MeshMaterial3d<TerrainDepthMaterial>>(entity)
            .is_some()
        || world.get::<MeshMaterial3d<WetMaterial>>(entity).is_some()
}

pub(super) fn install(app: &mut App) {
    if std::env::args().any(|arg| arg == "--smoke-test-animations") {
        app.insert_resource(Fixture {
            started: Instant::now(),
            root: None,
            child: None,
            original: Transform::default(),
            mode: 0,
            saw_zero: false,
            saw_middle: false,
            saw_draw: false,
            finished: false,
            fresh_complete: false,
            fresh_pixels: None,
            background: None,
        })
        .add_systems(PreStartup, |mut config: ResMut<GameConfig>| {
            config.client.chunk_animation = ChunkAnimation::FadeSlow;
        })
        .add_systems(Last, run);
    }
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut fixture: Mut<Fixture>| {
        if fixture.finished {
            return;
        }
        match advance(world, &mut fixture) {
            Ok(true) => {
                fixture.finished = true;
                tracing::info!("Native fresh-upload and re-entry animation opacity, alignment and view-independent surrounding terrain readiness verified");
                world.write_message(bevy::app::AppExit::Success);
            }
            Err(error) => {
                fixture.finished = true;
                tracing::error!(%error, "Native chunk animation fixture failed");
                world.write_message(bevy::app::AppExit::error());
            }
            Ok(false) => {}
        }
    });
}

fn advance(world: &mut World, fixture: &mut Fixture) -> Result<bool, String> {
    if fixture.started.elapsed() > Duration::from_secs(100) {
        let status = world.resource::<ChunkAnimationStatus>();
        return Err(format!(
            "Animation timeout: {status:?}; draws={:?}",
            world.resource::<crate::DrawCallStatus>().snapshot()
        ));
    }
    if fixture.mode == MODES.len() {
        return background_readiness(world, fixture);
    }
    let Some(root) = fixture.root else {
        let Some((frustum, eye)) = world.query_filtered::<(&bevy::camera::primitives::Frustum, &GlobalTransform), With<terra_player::TerraCamera>>().iter(world).next().map(|(f, t)| (*f, t.translation())) else { return Ok(false); };
        let root = world
            .query_filtered::<(Entity, &Children, &Transform), With<crate::TerrainSection>>()
            .iter(world)
            .filter_map(|(root, children, transform)| {
                world.get::<EnteringChunk>(root)?;
                let child = children.iter().find(|child| {
                    terrain_material(world, *child)
                        && world
                            .get::<bevy::camera::primitives::Aabb>(*child)
                            .zip(world.get::<GlobalTransform>(*child))
                            .is_some_and(|(bounds, transform)| {
                                frustum.intersects_obb(bounds, &transform.affine(), true, true)
                            })
                })?;
                Some((root, child, transform))
            })
            .min_by(|a, b| {
                a.2.translation
                    .distance_squared(eye)
                    .total_cmp(&b.2.translation.distance_squared(eye))
            })
            .map(|(entity, child, _)| (entity, child));
        let Some((root, child)) = root else {
            return Ok(false);
        };
        fixture.root = Some(root);
        fixture.child = Some(child);
        tracing::info!(
            ?root,
            "Observing first upload of a freshly generated section before its first color draw"
        );
        fixture.original = *world
            .get::<Transform>(root)
            .ok_or("Missing section transform")?;
        if world.get::<QueuedChunk>(root).is_some() {
            if world.get::<Visibility>(root) != Some(&Visibility::Hidden) {
                return Err("New animation root was not queued invisible".into());
            }
        } else if world.get::<ChunkFade>(root).is_none_or(|fade| fade.started) {
            return Err(
                "Fresh section was already visible before the fixture observed its entry".into(),
            );
        }
        return Ok(false);
    };
    if let Some(fade) = world.get::<ChunkFade>(root) {
        let Some(material) = fade
            .materials
            .iter()
            .map(|material| &material.handle)
            .find(|material| Some(material.entity()) == fixture.child)
        else {
            return Err("Real terrain section has no faded terrain material".into());
        };
        let alpha = match material {
            FadedHandle::Standard(_, _, temporary) => world
                .resource::<Assets<StandardMaterial>>()
                .get(temporary)
                .map(FadeMaterial::alpha),
            FadedHandle::Depth(_, _, temporary) => world
                .resource::<Assets<TerrainDepthMaterial>>()
                .get(temporary)
                .map(FadeMaterial::alpha),
            FadedHandle::Wet(_, _, temporary) => world
                .resource::<Assets<WetMaterial>>()
                .get(temporary)
                .map(FadeMaterial::alpha),
            _ => None,
        }
        .ok_or("Missing terrain fade material")?;
        fixture.saw_zero |= alpha == 0.0;
        fixture.saw_draw |= fade.started;
        if !fixture.saw_middle && alpha > 0.2 && alpha < 0.6 {
            capture(
                world,
                if fixture.fresh_complete {
                    fixture.mode
                } else {
                    MODES.len()
                },
            )?;
            fixture.saw_middle = true;
        }
        return Ok(false);
    }
    if world.get::<QueuedChunk>(root).is_some() {
        return Ok(false);
    }
    if !(fixture.saw_zero && fixture.saw_middle && fixture.saw_draw) {
        return Err(format!(
            "Instant or unrendered transition: zero={} partial={} drawn={}",
            fixture.saw_zero, fixture.saw_middle, fixture.saw_draw
        ));
    }
    if world.get::<Transform>(root) != Some(&fixture.original) {
        return Err("Animation did not restore the original section alignment".into());
    }
    if !fixture.fresh_complete {
        let Some(pixels) = fixture.fresh_pixels else {
            return Ok(false);
        };
        if pixels < 64 {
            return Err(format!(
                "Fresh fade drew only background: {pixels} terrain-colored pixels"
            ));
        }
        tracing::info!(
            pixels,
            "Fresh partial-opacity terrain is visible over the horizon background"
        );
    }
    if fixture.fresh_complete {
        tracing::info!(mode = ?MODES[fixture.mode], "Native real-section re-entry animation completed");
        fixture.mode += 1;
    } else {
        fixture.fresh_complete = true;
        tracing::info!(
            "Native first upload of newly generated terrain faded from zero through partial opacity to its final position"
        );
    }
    if fixture.mode == MODES.len() {
        return background_readiness(world, fixture);
    }
    fixture.saw_zero = false;
    fixture.saw_middle = false;
    fixture.saw_draw = false;
    queue(world, root, MODES[fixture.mode]);
    Ok(false)
}

fn background_readiness(world: &mut World, fixture: &mut Fixture) -> Result<bool, String> {
    if let Some(probe) = &mut fixture.background {
        if world.get_entity(probe.root).is_err()
            || world.get::<QueuedChunk>(probe.root).is_some()
            || world.get::<ChunkFade>(probe.root).is_some()
            || world.get::<EnteringChunk>(probe.root).is_some()
        {
            return Err(
                "Turning toward prepared background terrain triggered a fresh entry".into(),
            );
        }
        probe.saw_visible |= world
            .get::<bevy::camera::visibility::ViewVisibility>(probe.child)
            .is_some_and(|visibility| visibility.get());
        if probe.turned.elapsed() < Duration::from_secs(2) {
            return Ok(false);
        }
        if !probe.saw_visible {
            return Err(
                "Prepared offscreen terrain never became visible after the camera turn".into(),
            );
        }
        tracing::info!(
            root = ?probe.root,
            "Offscreen terrain was ready before the camera turned and became visible without a new animation queue"
        );
        return Ok(true);
    }
    let focus = world.resource::<terra_world::streaming::ChunkFocus>().0;
    let center = [
        (focus[0].floor() as i64).div_euclid(16),
        (focus[2].floor() as i64).div_euclid(16),
    ];
    let columns = [
        [-2, 0],
        [2, 0],
        [0, -2],
        [0, 2],
        [-2, -2],
        [-2, 2],
        [2, -2],
        [2, 2],
    ]
    .map(|offset| [center[0] + offset[0], center[1] + offset[1]]);
    let generated = {
        let chunks = world.resource::<terra_world::streaming::ChunkStore>();
        columns.iter().all(|column| {
            chunks.loaded.iter().any(|(position, section)| {
                [position.x, position.z] == *column && !section.data.is_empty()
            })
        })
    };
    if !generated {
        return Ok(false);
    }
    let roots = world
        .query::<(Entity, &crate::TerrainSection)>()
        .iter(world)
        .map(|(entity, terrain)| (entity, terrain.position))
        .collect::<Vec<_>>();
    if !columns.iter().all(|column| {
        roots.iter().any(|(root, position)| {
            [position.x, position.z] == *column
                && mesh_entities(world, *root)
                    .iter()
                    .any(|child| terrain_material(world, *child))
        })
    }) {
        return Ok(false);
    }
    let views = camera_views(world);
    let Some((camera, eye)) = world
        .query_filtered::<(Entity, &Transform), With<terra_player::TerraCamera>>()
        .iter(world)
        .next()
        .map(|(entity, transform)| (entity, transform.translation))
    else {
        return Ok(false);
    };
    let candidate = roots.into_iter().find_map(|(root, position)| {
        if !columns.contains(&[position.x, position.z])
            || world.get::<QueuedChunk>(root).is_some()
            || world.get::<ChunkFade>(root).is_some()
            || world.get::<EnteringChunk>(root).is_some()
            || world.get::<Visibility>(root) == Some(&Visibility::Hidden)
            || !known_outside_view(world, root, &views)
        {
            return None;
        }
        mesh_entities(world, root).into_iter().find_map(|child| {
            if !terrain_material(world, child) {
                return None;
            }
            let bounds = world.get::<bevy::camera::primitives::Aabb>(child)?;
            let transform = world.get::<GlobalTransform>(child)?;
            let target = Vec3::from(transform.affine().transform_point3a(bounds.center));
            let horizontal = Vec2::new(target.x - eye.x, target.z - eye.z);
            (horizontal.length_squared() <= 48.0_f32.powi(2)).then_some((root, child, target))
        })
    });
    let Some((root, child, target)) = candidate else {
        return Ok(false);
    };
    let mut transform = world
        .get_mut::<Transform>(camera)
        .ok_or("Missing camera transform during background readiness check")?;
    *transform = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    fixture.background = Some(BackgroundProbe {
        root,
        child,
        turned: Instant::now(),
        saw_visible: false,
    });
    tracing::info!(
        ?columns,
        ?root,
        "Generated nonempty terrain and uploaded meshes surround the stationary focus in all eight directions; turning toward already prepared offscreen terrain"
    );
    Ok(false)
}

fn queue(world: &mut World, root: Entity, mode: ChunkAnimation) {
    world.resource_mut::<GameConfig>().client.chunk_animation = mode;
    enter(&mut world.commands(), root, mode);
    world.flush();
}

fn capture(world: &mut World, index: usize) -> Result<(), String> {
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-animations"));
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let mut capture = world.spawn(bevy::render::view::screenshot::Screenshot::primary_window());
    capture.observe(bevy::render::view::screenshot::save_to_disk(
        directory.join(format!("animation-{index}.png")),
    ));
    if index == MODES.len() {
        capture.observe(
            |event: On<bevy::render::view::screenshot::ScreenshotCaptured>,
             mut fixture: ResMut<Fixture>| {
                // This fixture looks at green/ochre terrain. Blue haze and the
                // white crosshair alone must not satisfy a visible-fade check.
                use bevy::render::render_resource::TextureFormat;
                let blue_first = matches!(
                    event.image.texture_descriptor.format,
                    TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb
                );
                fixture.fresh_pixels = Some(event.image.data.as_ref().map_or(0, |bytes| {
                    bytes
                        .chunks_exact(4)
                        .filter(|pixel| {
                            let (red, blue) = if blue_first {
                                (pixel[2], pixel[0])
                            } else {
                                (pixel[0], pixel[2])
                            };
                            red > blue.saturating_add(8) || pixel[1] > blue.saturating_add(8)
                        })
                        .count()
                }));
            },
        );
    }
    Ok(())
}
